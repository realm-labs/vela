use super::*;

#[test]
fn compiler_lowers_radix_ints_and_exponent_floats() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    return 0x10 + 0b10 + 3.5e+1;
}
"#,
        "main",
    )
    .expect("numeric literal source should compile");
    assert!(code.constants.contains(&Constant::Int(16)));
    assert!(code.constants.contains(&Constant::Int(2)));
    assert!(code.constants.contains(&Constant::Float(35.0)));
}
#[test]
fn compiler_rejects_uppercase_radix_prefixes_before_codegen() {
    let error = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    return 0X10 + 0B10;
}
"#,
        "main",
    )
    .expect_err("uppercase radix prefixes should be rejected by syntax validation");
    let CompileErrorKind::SyntaxDiagnostics(diagnostics) = error.kind else {
        panic!("expected syntax diagnostics");
    };
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code.as_deref() == Some("E_LEX_INT"))
    );
    assert_eq!(diagnostics.len(), 2);
}
#[test]
fn compiler_accepts_leading_shebang() {
    let code = compile_function_source(
        SourceId::new(1),
        "#!/usr/bin/env vela\nfn main() { return 7; }\n",
        "main",
    )
    .expect("shebang source should compile");
    assert!(code.constants.contains(&Constant::Int(7)));
}
#[test]
fn compiler_lowers_unicode_string_escapes() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"fn main() { return "\u{41}\u{7a}"; }"#,
        "main",
    )
    .expect("unicode escaped string source should compile");
    assert!(code.constants.contains(&Constant::String("Az".into())));
}
#[test]
fn compiler_lowers_script_value_method_calls() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let values = [1, 2, 3];
    let reward = Reward { item_id: "gold", count: 3 };
    return values.len() + reward.item_id.len();
}
"#,
        "main",
    )
    .expect("script value method call should compile");
    assert!(code.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        InstructionKind::CallMethod { method, .. } if method == "len"
    )));
}
#[test]
fn compiler_uses_hir_signatures_for_code_object_params() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main(player: game.Player, amount: int) -> int {
    return amount;
}
"#,
        "main",
    )
    .expect("typed params should compile through HIR signature metadata");
    assert_eq!(code.params, ["player", "amount"]);
}
#[test]
fn compiler_lowers_parameter_defaults_and_named_script_args() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn grant(base, amount = 10, bonus = amount + 1) {
    return base + amount + bonus;
}
fn main() {
    return grant(bonus = 5, base = 1);
}
"#,
    )
    .expect("named args and defaults should compile");
    let grant = program.function("grant").expect("grant function");
    let main = program.function("main").expect("main function");
    assert_eq!(grant.param_defaults, [false, true, true]);
    assert!(
        grant.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            InstructionKind::JumpIfNotMissing { .. }
        ))
    );
    assert!(main.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        InstructionKind::CallFunction { args, .. }
            if args.len() == 3 && matches!(args[1], CallArgument::Missing)
    )));
}

#[test]
fn compiler_lowers_named_native_args_from_compiler_options() {
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main() {
    return game.add(rhs = 3, lhs = 2);
}
"#,
        &CompilerOptions::new()
            .with_native_module_root("game")
            .with_native_function_params("game.add", ["lhs", "rhs"]),
    )
    .expect("named native args should compile with descriptor metadata");
    let main = program.function("main").expect("main function");

    assert!(main.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        InstructionKind::CallNative { name, args, .. } if name == "game.add" && args.len() == 2
    )));
}

#[test]
fn compiler_lowers_named_value_method_args_from_compiler_options() {
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main() {
    return {"gold": 4}.get_or(default = 0, key = "gold");
}
"#,
        &CompilerOptions::new().with_required_value_method_params("get_or", ["key", "default"]),
    )
    .expect("named value method args should compile with descriptor metadata");
    let main = program.function("main").expect("main function");

    assert!(main.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        InstructionKind::CallMethod { method, args, .. } if method == "get_or" && args.len() == 2
    )));
}

#[test]
fn compiler_reports_named_value_method_arg_diagnostics_from_compiler_options() {
    let error = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main() {
    return {"gold": 4}.get_or(defalt = 0, key = "gold");
}
"#,
        &CompilerOptions::new().with_required_value_method_params("get_or", ["key", "default"]),
    )
    .expect_err("unknown named value method arg should fail");

    assert_eq!(
        semantic_diagnostic_codes(error),
        [
            "compiler::unknown_named_argument",
            "compiler::missing_required_argument"
        ]
    );
}

#[test]
fn compiler_lowers_named_value_method_args_by_receiver_type_from_compiler_options() {
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main() {
    return "reward:gold".contains(needle = ":") && ["gold"].contains(value = "gold");
}
"#,
        &CompilerOptions::new()
            .with_required_value_method_params_for_type("string", "contains", ["needle"])
            .with_required_value_method_params_for_type("array", "contains", ["value"]),
    )
    .expect("receiver-specific named value method args should compile");
    let main = program.function("main").expect("main function");

    assert_eq!(
        main.instructions
            .iter()
            .filter(|instruction| matches!(
                &instruction.kind,
                InstructionKind::CallMethod { method, args, .. }
                    if method == "contains" && args.len() == 1
            ))
            .count(),
        2
    );
}

#[test]
fn compiler_rejects_ambiguous_named_value_method_args_without_receiver_type() {
    compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(value) {
    return value.contains(needle = ":");
}
"#,
        &CompilerOptions::new()
            .with_required_value_method_params_for_type("string", "contains", ["needle"])
            .with_required_value_method_params_for_type("array", "contains", ["value"]),
    )
    .expect_err("ambiguous named method args should require receiver type evidence");
}

#[test]
fn compiler_reports_named_native_arg_diagnostics_from_compiler_options() {
    let error = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main() {
    return game.add(rsh = 3, lhs = 2);
}
"#,
        &CompilerOptions::new()
            .with_native_module_root("game")
            .with_native_function_params("game.add", ["lhs", "rhs"]),
    )
    .expect_err("unknown named native arg should fail");

    assert_eq!(
        semantic_diagnostic_codes(error),
        [
            "compiler::unknown_named_argument",
            "compiler::missing_required_argument"
        ]
    );
}

#[test]
fn compiler_reports_script_call_argument_diagnostics() {
    let unknown = compile_program_source(
        SourceId::new(1),
        r#"
fn grant(base, amount = 10) {
    return base + amount;
}
fn main() {
    return grant(amunt = 2, base = 1);
}
"#,
    )
    .expect_err("unknown named argument should fail");
    let duplicate = compile_program_source(
        SourceId::new(2),
        r#"
fn grant(base, amount = 10) {
    return base + amount;
}
fn main() {
    return grant(1, base = 2);
}
"#,
    )
    .expect_err("duplicate argument should fail");
    let positional_after_named = compile_program_source(
        SourceId::new(3),
        r#"
fn grant(base, amount = 10) {
    return base + amount;
}
fn main() {
    return grant(amount = 2, 1);
}
"#,
    )
    .expect_err("positional argument after named argument should fail");
    let too_many = compile_program_source(
        SourceId::new(4),
        r#"
fn grant(base) {
    return base;
}
fn main() {
    return grant(1, 2);
}
"#,
    )
    .expect_err("too many arguments should fail");
    let missing = compile_program_source(
        SourceId::new(5),
        r#"
fn grant(base, amount = 10) {
    return base + amount;
}
fn main() {
    return grant();
}
"#,
    )
    .expect_err("missing required argument should fail");
    assert_eq!(
        semantic_diagnostic_codes(unknown),
        ["compiler::unknown_named_argument"]
    );
    assert_eq!(
        semantic_diagnostic_codes(duplicate),
        ["compiler::duplicate_argument"]
    );
    assert_eq!(
        semantic_diagnostic_codes(positional_after_named),
        [
            "compiler::positional_after_named_argument",
            "compiler::missing_required_argument"
        ]
    );
    assert_eq!(
        semantic_diagnostic_codes(too_many),
        ["compiler::too_many_arguments"]
    );
    assert_eq!(
        semantic_diagnostic_codes(missing),
        ["compiler::missing_required_argument"]
    );
}
