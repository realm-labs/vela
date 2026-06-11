use super::*;

#[test]
fn compiler_reports_named_native_arg_diagnostics_from_registry() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    registry
        .register_function(vela_registry::FunctionDef::new(
            vela_def::DefPath::function("host", ["game"], "add"),
            vela_registry::FunctionSignature::new(
                [
                    vela_registry::ParamDef::new("lhs", Some("i64")),
                    vela_registry::ParamDef::new("rhs", Some("i64")),
                ],
                Some("i64".to_owned()),
            ),
        ))
        .expect("test native function should register");
    let error = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    return game::add(rsh = 3, lhs = 2);
}
"#,
        registry.compile_view(),
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
