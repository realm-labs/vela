use super::*;

fn value_method_registry(specs: &[(&str, &str, &[&str])]) -> vela_registry::DefinitionRegistry {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let mut types = std::collections::BTreeMap::new();
    for (type_name, method, params) in specs {
        let owner = *types.entry(*type_name).or_insert_with(|| {
            let path = vela_def::DefPath::ty("std", std::iter::empty::<&str>(), *type_name);
            let def = if let Some(primitive) = primitive_for_std_type_name(type_name) {
                vela_registry::TypeDef::primitive(path, primitive)
            } else {
                vela_registry::TypeDef::new(path)
            };
            registry
                .register_type(def)
                .expect("test value method type should register")
        });
        registry
            .register_method(vela_registry::MethodDef::new(
                vela_def::DefPath::method("std", std::iter::empty::<&str>(), *type_name, *method),
                owner,
                vela_registry::FunctionSignature::new(
                    params
                        .iter()
                        .map(|param| vela_registry::ParamDef::new(*param, None::<String>)),
                    None,
                ),
            ))
            .expect("test value method should register");
    }
    registry
}

fn primitive_for_std_type_name(type_name: &str) -> Option<vela_common::PrimitiveTag> {
    match type_name {
        "Null" => Some(vela_common::PrimitiveTag::Null),
        "Bool" => Some(vela_common::PrimitiveTag::Bool),
        "I8" => Some(vela_common::PrimitiveTag::I8),
        "I16" => Some(vela_common::PrimitiveTag::I16),
        "I32" => Some(vela_common::PrimitiveTag::I32),
        "I64" => Some(vela_common::PrimitiveTag::I64),
        "U8" => Some(vela_common::PrimitiveTag::U8),
        "U16" => Some(vela_common::PrimitiveTag::U16),
        "U32" => Some(vela_common::PrimitiveTag::U32),
        "U64" => Some(vela_common::PrimitiveTag::U64),
        "F32" => Some(vela_common::PrimitiveTag::F32),
        "F64" => Some(vela_common::PrimitiveTag::F64),
        "String" => Some(vela_common::PrimitiveTag::String),
        "Bytes" => Some(vela_common::PrimitiveTag::Bytes),
        _ => None,
    }
}

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
    assert!(
        code.constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::I64(16)))
    );
    assert!(
        code.constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::I64(2)))
    );
    assert!(
        code.constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::F64(35.0)))
    );
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
fn compiler_lowers_suffixed_numeric_literals_to_scalar_constants() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let i8_value = 12i8;
    let i16_value = 12i16;
    let i32_value = 12i32;
    let i64_value = 12i64;
    let u8_value = 12u8;
    let u16_value = 12u16;
    let u32_value = 12u32;
    let u64_value = 12u64;
    let f32_value = 12.5f32;
    let f64_value = 12.5f64;
    return f64_value;
}
"#,
        "main",
    )
    .expect("suffixed numeric literals should compile");

    for expected in [
        vela_common::ScalarValue::I8(12),
        vela_common::ScalarValue::I16(12),
        vela_common::ScalarValue::I32(12),
        vela_common::ScalarValue::I64(12),
        vela_common::ScalarValue::U8(12),
        vela_common::ScalarValue::U16(12),
        vela_common::ScalarValue::U32(12),
        vela_common::ScalarValue::U64(12),
        vela_common::ScalarValue::F32(12.5),
        vela_common::ScalarValue::F64(12.5),
    ] {
        assert!(
            code.constants.contains(&Constant::Scalar(expected)),
            "missing scalar constant {expected}"
        );
    }
}

#[test]
fn compiler_accepts_signed_min_suffixed_literals() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let min_i8 = -128i8;
    let min_i16 = -32768i16;
    let min_i32 = -2147483648i32;
    let min_i64 = -9223372036854775808i64;
    return min_i64;
}
"#,
        "main",
    )
    .expect("signed minimum literals should compile through unary-aware lowering");

    for expected in [
        vela_common::ScalarValue::I8(i8::MIN),
        vela_common::ScalarValue::I16(i16::MIN),
        vela_common::ScalarValue::I32(i32::MIN),
        vela_common::ScalarValue::I64(i64::MIN),
    ] {
        assert!(
            code.constants.contains(&Constant::Scalar(expected)),
            "missing signed minimum constant {expected}"
        );
    }
}

#[test]
fn compiler_rejects_out_of_range_suffixed_integer_literals() {
    let error = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    return 128i8;
}
"#,
        "main",
    )
    .expect_err("out-of-range suffixed literal should fail");

    let CompileErrorKind::InvalidIntLiteral { literal, error } = error.kind else {
        panic!("expected invalid integer literal");
    };
    assert_eq!(literal, "128i8");
    assert!(error.contains("out of range"), "{error}");
}

#[test]
fn compiler_accepts_leading_shebang() {
    let code = compile_function_source(
        SourceId::new(1),
        "#!/usr/bin/env vela\nfn main() { return 7; }\n",
        "main",
    )
    .expect("shebang source should compile");
    assert!(
        code.constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::I64(7)))
    );
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
        UnlinkedInstructionKind::CallMethod { method, .. } if method == "len"
    )));
}
#[test]
fn compiler_uses_hir_signatures_for_code_object_params() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main(player: game::Player, amount: int) -> int {
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
    assert!(grant.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::JumpIfNotMissing { .. }
    )));
    assert!(main.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        UnlinkedInstructionKind::CallFunction { args, .. }
            if args.len() == 3 && matches!(args[1], CallArgument::Missing)
    )));
}

#[test]
fn compiler_lowers_named_native_args_from_registry() {
    let native_id = vela_def::FunctionId::new(77);
    let mut registry = vela_registry::DefinitionRegistry::new();
    registry
        .register_function(
            vela_registry::FunctionDef::new(
                vela_def::DefPath::function("host", ["game"], "add"),
                vela_registry::FunctionSignature::new(
                    [
                        vela_registry::ParamDef::new("lhs", Some("int")),
                        vela_registry::ParamDef::new("rhs", Some("int")),
                    ],
                    Some("int".to_owned()),
                ),
            )
            .with_id(native_id),
        )
        .expect("test native function should register");
    let program = compile_program_source_with_options_and_registry(
        SourceId::new(1),
        r#"
fn main() {
    return game::add(rhs = 3, lhs = 2);
}
"#,
        &CompilerOptions::new(),
        registry.compile_view(),
    )
    .expect("named native args should compile from registry metadata");
    let main = program.function("main").expect("main function");

    assert!(main.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        UnlinkedInstructionKind::CallNative { name, native, args, .. }
            if name == "game::add" && *native == native_id && args.len() == 2
    )));
}

#[test]
fn compiler_reports_unresolved_native_from_registry() {
    let registry = vela_registry::DefinitionRegistry::new();
    let error = compile_program_source_with_options_and_registry(
        SourceId::new(1),
        r#"
fn main() {
    return game::missing(1);
}
"#,
        &CompilerOptions::new(),
        registry.compile_view(),
    )
    .expect_err("missing registry native should fail before bytecode emission");

    assert_eq!(
        semantic_diagnostic_codes(error),
        ["compiler::unresolved_native_function"]
    );
}

#[test]
fn compiler_lowers_named_value_method_args_from_registry() {
    let registry = value_method_registry(&[("Map", "get_or", &["key", "default"])]);
    let program = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    return {"gold": 4}.get_or(default = 0, key = "gold");
}
"#,
        registry.compile_view(),
    )
    .expect("named value method args should compile with registry metadata");
    let main = program.function("main").expect("main function");

    assert!(main.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        UnlinkedInstructionKind::CallMethodId { method, args, .. } if method == "get_or" && args.len() == 2
    )));
}

#[test]
fn compiler_reports_named_value_method_arg_diagnostics_from_registry() {
    let registry = value_method_registry(&[("Map", "get_or", &["key", "default"])]);
    let error = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    return {"gold": 4}.get_or(defalt = 0, key = "gold");
}
"#,
        registry.compile_view(),
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
fn compiler_lowers_named_value_method_args_by_receiver_type_from_registry() {
    let registry = value_method_registry(&[
        ("String", "contains", &["needle"]),
        ("Array", "contains", &["value"]),
    ]);
    let program = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    return "reward:gold".contains(needle = ":") && ["gold"].contains(value = "gold");
}
"#,
        registry.compile_view(),
    )
    .expect("receiver-specific named value method args should compile");
    let main = program.function("main").expect("main function");

    assert_eq!(
        main.instructions
            .iter()
            .filter(|instruction| matches!(
                &instruction.kind,
                UnlinkedInstructionKind::CallMethodId { method, args, .. }
                    if method == "contains" && args.len() == 1
            ))
            .count(),
        2
    );
}

#[test]
fn compiler_lowers_named_value_method_args_and_id_from_registry() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let string_type = vela_registry::TypeDef::new(vela_def::DefPath::ty(
        "std",
        std::iter::empty::<&str>(),
        "String",
    ));
    let string_type_id = registry
        .register_type(string_type)
        .expect("String type should register");
    let method_def = vela_registry::MethodDef::new(
        vela_def::DefPath::method("std", std::iter::empty::<&str>(), "String", "contains"),
        string_type_id,
        vela_registry::FunctionSignature::new(
            [vela_registry::ParamDef::new("needle", Some("string"))],
            Some("bool".to_owned()),
        ),
    );
    let method = registry
        .register_method(method_def)
        .expect("String::contains method should register");
    let program = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    return "reward:gold".contains(needle = ":");
}
"#,
        registry.compile_view(),
    )
    .expect("registry value method args should compile");
    let main = program.function("main").expect("main function");

    let lowered = main.instructions.iter().find_map(|instruction| {
        let UnlinkedInstructionKind::CallMethodId {
            method: name,
            method_id,
            args,
            ..
        } = &instruction.kind
        else {
            return None;
        };
        (name == "contains").then_some((*method_id, args.len()))
    });

    assert_eq!(lowered, Some((method, 1)));
}

#[test]
fn compiler_lowers_named_value_method_args_from_local_value_type_flow() {
    let registry = value_method_registry(&[
        ("String", "contains", &["needle"]),
        ("Array", "contains", &["value"]),
    ]);
    let program = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main(text: string) {
    let parts = ["gold"];
    let reward = "reward:gold";
    return text.contains(needle = ":")
        && reward.contains(needle = ":")
        && parts.contains(value = "gold");
}
"#,
        registry.compile_view(),
    )
    .expect("local value method receiver facts should compile");
    let main = program.function("main").expect("main function");

    assert_eq!(
        main.instructions
            .iter()
            .filter(|instruction| matches!(
                &instruction.kind,
                UnlinkedInstructionKind::CallMethodId { method, args, .. }
                    if method == "contains" && args.len() == 1
            ))
            .count(),
        3
    );
}

#[test]
fn compiler_tracks_exact_primitive_value_type_flow() {
    let registry = value_method_registry(&[
        ("I8", "touch", &["arg"]),
        ("I64", "touch", &["arg"]),
        ("U32", "touch", &["arg"]),
        ("F32", "touch", &["arg"]),
        ("F64", "touch", &["arg"]),
        ("String", "touch", &["arg"]),
        ("Bytes", "touch", &["arg"]),
    ]);
    let program = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main(i64_arg: i64, text: string, blob: bytes) {
    let i8_value = 1i8;
    let u32_value = 1u32;
    let f32_value = 1.0f32;
    let f64_value = 1.0f64;
    return i8_value.touch(arg = 0)
        && i64_arg.touch(arg = 0)
        && u32_value.touch(arg = 0)
        && f32_value.touch(arg = 0)
        && f64_value.touch(arg = 0)
        && text.touch(arg = "")
        && blob.touch(arg = b"");
}
"#,
        registry.compile_view(),
    )
    .expect("exact primitive value receiver facts should compile");
    let main = program.function("main").expect("main function");

    assert_eq!(
        main.instructions
            .iter()
            .filter(|instruction| matches!(
                &instruction.kind,
                UnlinkedInstructionKind::CallMethodId { method, args, .. }
                    if method == "touch" && args.len() == 1
            ))
            .count(),
        7
    );
}

#[test]
fn compiler_lowers_named_value_method_args_from_captured_value_type_flow() {
    let registry = value_method_registry(&[("String", "contains", &["needle"])]);
    let program = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    let reward = "reward:gold";
    let has_separator = |needle| reward.contains(needle = needle);
    return has_separator(":");
}
"#,
        registry.compile_view(),
    )
    .expect("captured value method receiver facts should compile");
    let main = program.function("main").expect("main function");
    let lambda = main
        .instructions
        .iter()
        .find_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::MakeClosure { function, .. } => {
                main.nested_function(*function)
            }
            _ => None,
        })
        .expect("lambda code object");

    assert!(lambda.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        UnlinkedInstructionKind::CallMethodId { method, args, .. } if method == "contains" && args.len() == 1
    )));
}

#[test]
fn compiler_lowers_value_method_ids_in_collection_callback_params() {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let program = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    let names = ["Quest", "Raid"];
    let rewards = {"gold": 5, "gem": 6};
    let tags = set::from_array(["daily", "raid"]);
    let matched = names.filter(|name| name.starts_with("Q"));
    let valuable = rewards.filter(|key, value| key.len() >= 3 && value >= 5);
    let found = tags.find(|tag| tag.starts_with("r"));
    return matched.len() + valuable.len() + found.unwrap_or("").len();
}
"#,
        registry.compile_view(),
    )
    .expect("collection callback parameter value methods should compile");
    let main = program.function("main").expect("main function");
    let methods = nested_method_id_names(main);

    assert!(methods.iter().any(|method| method == "starts_with"));
    assert!(methods.iter().any(|method| method == "len"));
}

#[test]
fn compiler_lowers_value_method_ids_after_array_extrema_methods() {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let program = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    let values = [4, 1, 3, 1];
    return values.min().unwrap_or(0) + values.max().unwrap_or(0);
}
"#,
        registry.compile_view(),
    )
    .expect("array extrema option methods should compile");
    let main = program.function("main").expect("main function");
    let methods = nested_method_id_names(main);

    assert!(methods.iter().any(|method| method == "min"));
    assert!(methods.iter().any(|method| method == "max"));
    assert_eq!(
        methods
            .iter()
            .filter(|method| method.as_str() == "unwrap_or")
            .count(),
        2
    );
}

#[test]
fn compiler_lowers_value_method_ids_after_set_values_method() {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let program = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    let numbers = set::from_array([1, 2, 3]);
    let tags = set::from_array(["raid", "daily"]);
    return numbers.values().sum() + tags.values().sort_by(|tag| tag).join(",").len();
}
"#,
        registry.compile_view(),
    )
    .expect("set values array methods should compile");
    let main = program.function("main").expect("main function");
    let methods = nested_method_id_names(main);

    assert!(methods.iter().any(|method| method == "values"));
    assert!(methods.iter().any(|method| method == "sum"));
    assert!(methods.iter().any(|method| method == "sort_by"));
    assert!(methods.iter().any(|method| method == "join"));
    assert!(methods.iter().any(|method| method == "len"));
}

#[test]
fn compiler_lowers_set_method_ids_after_mixed_string_shapes() {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let program = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    let reward_pair = "reward:gold".split_once(":").unwrap_or(["reward", ""]);
    let item = reward_pair[1];
    let tags = set::from_array(["reward", item, "daily", item]);
    return tags.has("gold");
}
"#,
        registry.compile_view(),
    )
    .expect("mixed literal and indexed string set methods should compile");
    let main = program.function("main").expect("main function");
    let methods = nested_method_id_names(main);

    assert!(methods.iter().any(|method| method == "split_once"));
    assert!(methods.iter().any(|method| method == "unwrap_or"));
    assert!(methods.iter().any(|method| method == "has"));
}

#[test]
fn compiler_lowers_value_method_ids_after_string_char_at_method() {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let program = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    return "level.up".char_at(5).unwrap_or("");
}
"#,
        registry.compile_view(),
    )
    .expect("string char_at option method should compile");
    let main = program.function("main").expect("main function");
    let methods = nested_method_id_names(main);

    assert!(methods.iter().any(|method| method == "char_at"));
    assert!(methods.iter().any(|method| method == "unwrap_or"));
}

#[test]
fn compiler_lowers_value_method_ids_after_reflection_metadata_collections() {
    let mut registry = vela_stdlib::standard_registry().expect("standard registry should build");
    for (name, params) in [
        ("type_info", &["name"][..]),
        ("function", &["name"]),
        ("functions", &[]),
        ("effects", &["target"]),
        ("fields", &["target"]),
        ("params", &["target"]),
        ("methods", &["target"]),
        ("method", &["target", "name"]),
        ("variants", &["target"]),
    ] {
        registry
            .register_function(vela_registry::FunctionDef::new(
                vela_def::DefPath::function("host", ["reflect"], name),
                vela_registry::FunctionSignature::new(
                    params
                        .iter()
                        .map(|param| vela_registry::ParamDef::new(*param, None::<String>)),
                    None::<String>,
                ),
            ))
            .expect("test reflection native should register");
    }
    let program = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    let target = reflect::type_info("Context");
    let option_type = reflect::type_info("Option");
    let fields = reflect::fields(target);
    let methods = reflect::methods(target);
    let functions = reflect::functions();
    let variants = reflect::variants(option_type);
    let emit = reflect::method(target, "emit");
    let random = reflect::function("math::random");
    let random_params = reflect::params(random);
    let effects = reflect::effects(random);
    return fields.len() > 0
        && methods.len() > 0
        && fields[0].name.len() > 0
        && fields[0].access.reflect_readable
        && functions[0].name.len() > 0
        && random.public
        && random.access.reflect_visible
        && random.access.required_permissions.len() == 0
        && random_params.len() == 0
        && emit.owner.len() > 0
        && emit.access.reflect_callable
        && emit.params[0].name.len() > 0
        && emit.params[0].defaulted == false
        && variants[0].name.len() > 0
        && variants[0].fields[0].name.len() > 0
        && effects.uses_random
        && !effects.reads_host;
}
"#,
        registry.compile_view(),
    )
    .expect("reflection metadata collection value methods should compile");
    let main = program.function("main").expect("main function");
    let methods = nested_method_id_names(main);
    let record_fields = main
        .instructions
        .iter()
        .filter_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::GetRecordSlot { field, .. } => Some(field.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(methods.iter().any(|method| method == "len"));
    assert!(record_fields.contains(&"name"));
    assert!(record_fields.contains(&"owner"));
    assert!(record_fields.contains(&"public"));
    assert!(record_fields.contains(&"reflect_callable"));
    assert!(record_fields.contains(&"reflect_readable"));
    assert!(record_fields.contains(&"reflect_visible"));
    assert!(record_fields.contains(&"required_permissions"));
    assert!(record_fields.contains(&"defaulted"));
    assert!(record_fields.contains(&"uses_random"));
    assert!(record_fields.contains(&"reads_host"));
}

#[test]
fn compiler_lowers_value_method_ids_in_option_result_callback_params() {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let program = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    let option_chain = option::some("quest")
        .map(|value| value.to_upper())
        .filter(|value| value.starts_with("Q"));
    let result_chain = result::ok(["gold", "xp"])
        .map(|values| values.join("+"))
        .and_then(|text| result::ok(text.replace("+", ".")));
    let mapped_err = result::err(["bad", "level"]).map_err(|errors| errors.join("."));
    return option_chain.unwrap_or("")
        + result_chain.unwrap_or("")
        + mapped_err.to_error_option().unwrap_or("");
}
"#,
        registry.compile_view(),
    )
    .expect("Option/Result callback parameter value methods should compile");
    let main = program.function("main").expect("main function");
    let methods = nested_method_id_names(main);

    assert!(methods.iter().any(|method| method == "to_upper"));
    assert!(methods.iter().any(|method| method == "starts_with"));
    assert!(methods.iter().any(|method| method == "join"));
    assert!(methods.iter().any(|method| method == "replace"));
}

#[test]
fn compiler_does_not_leak_named_value_method_receiver_facts_from_for_body() {
    let registry = value_method_registry(&[
        ("String", "contains", &["needle"]),
        ("Array", "contains", &["value"]),
    ]);
    let err = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    let value = 1;
    for item in [] {
        value = "reward:gold";
    }
    return value.contains(needle = ":");
}
"#,
        registry.compile_view(),
    )
    .expect_err("for body value receiver facts must not leak after loop scope");

    assert_eq!(
        err.kind,
        CompileErrorKind::UnsupportedSyntax("script method call")
    );
}

fn nested_method_id_names(code: &UnlinkedCodeObject) -> Vec<String> {
    let mut methods = Vec::new();
    collect_nested_method_id_names(code, &mut methods);
    methods
}

fn collect_nested_method_id_names(code: &UnlinkedCodeObject, methods: &mut Vec<String>) {
    for instruction in &code.instructions {
        if let UnlinkedInstructionKind::CallMethodId { method, .. } = &instruction.kind {
            methods.push(method.clone());
        }
    }
    for nested in &code.nested_functions {
        collect_nested_method_id_names(nested, methods);
    }
}

#[test]
fn compiler_does_not_leak_named_value_method_receiver_facts_from_match_arm() {
    let registry = value_method_registry(&[
        ("String", "contains", &["needle"]),
        ("Array", "contains", &["value"]),
    ]);
    let err = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    let value = 1;
    match value {
        1 => {
            value = "reward:gold";
        }
        _ => {}
    }
    return value.contains(needle = ":");
}
"#,
        registry.compile_view(),
    )
    .expect_err("match arm value receiver facts must not leak after match scope");

    assert_eq!(
        err.kind,
        CompileErrorKind::UnsupportedSyntax("script method call")
    );
}

#[test]
fn compiler_rejects_ambiguous_named_value_method_args_without_receiver_type() {
    let registry = value_method_registry(&[
        ("String", "contains", &["needle"]),
        ("Array", "contains", &["value"]),
    ]);
    compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main(value) {
    return value.contains(needle = ":");
}
"#,
        registry.compile_view(),
    )
    .expect_err("ambiguous named method args should require receiver type evidence");
}

#[test]
fn compiler_reports_named_native_arg_diagnostics_from_registry() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    registry
        .register_function(vela_registry::FunctionDef::new(
            vela_def::DefPath::function("host", ["game"], "add"),
            vela_registry::FunctionSignature::new(
                [
                    vela_registry::ParamDef::new("lhs", Some("int")),
                    vela_registry::ParamDef::new("rhs", Some("int")),
                ],
                Some("int".to_owned()),
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
