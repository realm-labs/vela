use super::*;

#[test]
fn engine_installs_registered_native_functions_into_vm() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::add", NativeFunctionId::new(1))
                .param("lhs", TypeHint::Int)
                .param("rhs", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::pure())
                .access(FunctionAccess::public())
                .docs("Adds two integers."),
            |args| {
                let [OwnedValue::Int(lhs), OwnedValue::Int(rhs)] = args else {
                    return Ok(OwnedValue::Null);
                };
                Ok(OwnedValue::Int(lhs + rhs))
            },
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return game::add(2, 3);
}
"#,
    )
    .expect("program should compile");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(OwnedValue::Int(5))
    );
}

#[test]
fn engine_compiler_options_lower_named_registered_native_arguments() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::subtract", NativeFunctionId::new(27))
                .param("lhs", TypeHint::Int)
                .param("rhs", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::pure())
                .access(FunctionAccess::public()),
            |args| {
                let [OwnedValue::Int(lhs), OwnedValue::Int(rhs)] = args else {
                    return Ok(OwnedValue::Null);
                };
                Ok(OwnedValue::Int(lhs - rhs))
            },
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main() {
    return game::subtract(rhs = 3, lhs = 10);
}
"#,
        &engine.compiler_options(),
    )
    .expect("named registered native arguments should compile");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(OwnedValue::Int(7))
    );
}

#[test]
fn engine_compiler_options_lower_named_standard_native_arguments() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main() {
    return math::clamp(max = 10, value = 15, min = 1);
}
"#,
        &engine.compiler_options(),
    )
    .expect("named stdlib native arguments should compile");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(OwnedValue::Int(10))
    );
}

#[test]
fn engine_compiler_options_emit_standard_native_ids() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main() {
    return math::clamp(max = 10, value = 15, min = 1);
}
"#,
        &engine.compiler_options(),
    )
    .expect("standard native should compile");
    let main = program.function("main").expect("main should compile");

    let native = main
        .instructions
        .iter()
        .find_map(|instruction| match &instruction.kind {
            InstructionKind::CallNative { name, native, .. } if name == "math::clamp" => *native,
            _ => None,
        });

    assert_eq!(native, Some(crate::standard::MATH_CLAMP_FUNCTION_ID));
}

#[test]
fn engine_compiler_options_emit_standard_value_method_ids() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main() {
    return "gold".len();
}
"#,
        &engine.compiler_options(),
    )
    .expect("standard value method should compile");
    let main = program.function("main").expect("main should compile");

    let value_method = main
        .instructions
        .iter()
        .find_map(|instruction| match &instruction.kind {
            InstructionKind::CallMethod {
                method,
                value_method_id,
                ..
            } if method == "len" => *value_method_id,
            _ => None,
        });

    assert_eq!(value_method, Some(crate::standard::STRING_LEN_METHOD_ID));
}

#[test]
fn engine_compiler_options_emit_standard_range_method_ids() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main() {
    return (1..4).len();
}
"#,
        &engine.compiler_options(),
    )
    .expect("standard range method should compile");
    let main = program.function("main").expect("main should compile");

    let value_method = main
        .instructions
        .iter()
        .find_map(|instruction| match &instruction.kind {
            InstructionKind::CallMethod {
                method,
                value_method_id,
                ..
            } if method == "len" => *value_method_id,
            _ => None,
        });

    assert_eq!(value_method, Some(crate::standard::RANGE_LEN_METHOD_ID));
}

#[test]
fn engine_compiler_options_emit_standard_option_result_method_ids() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main() {
    let some: Option = option::some(1);
    let err: Result = result::err("bad");
    return some.is_some() && err.is_err();
}
"#,
        &engine.compiler_options(),
    )
    .expect("standard option/result methods should compile");
    let main = program.function("main").expect("main should compile");

    let value_methods = main
        .instructions
        .iter()
        .filter_map(|instruction| match &instruction.kind {
            InstructionKind::CallMethod {
                method,
                value_method_id,
                ..
            } => Some((method.as_str(), *value_method_id)),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(value_methods.contains(&("is_some", Some(crate::standard::OPTION_IS_SOME_METHOD_ID))));
    assert!(value_methods.contains(&("is_err", Some(crate::standard::RESULT_IS_ERR_METHOD_ID))));
}

#[test]
fn engine_compiler_options_emit_standard_collection_method_ids() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main() {
    let names: array = ["gold", "xp"];
    let rewards: map = {"gold": 4};
    let tags: set = set::from_array(["daily"]);
    let other: set = set::from_array(["raid"]);
    names.push("bonus");
    names.pop();
    rewards.set("xp", 6);
    rewards.remove("xp");
    if names.contains("gold")
        && rewards.has("gold")
        && tags.has("daily")
        && tags.is_subset(tags)
        && tags.is_superset(tags)
        && tags.is_disjoint(other)
    {
        names.clear();
        rewards.clear();
        return names.len() + rewards.len() + tags.len();
    }
    return 0;
}
"#,
        &engine.compiler_options(),
    )
    .expect("standard collection methods should compile");
    let main = program.function("main").expect("main should compile");

    let value_methods = main
        .instructions
        .iter()
        .filter_map(|instruction| match &instruction.kind {
            InstructionKind::CallMethod {
                method,
                value_method_id,
                ..
            } => Some((method.as_str(), *value_method_id)),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(value_methods.contains(&("len", Some(crate::standard::ARRAY_LEN_METHOD_ID))));
    assert!(value_methods.contains(&("len", Some(crate::standard::MAP_LEN_METHOD_ID))));
    assert!(value_methods.contains(&("len", Some(crate::standard::SET_LEN_METHOD_ID))));
    assert!(value_methods.contains(&("contains", Some(crate::standard::ARRAY_CONTAINS_METHOD_ID))));
    assert!(value_methods.contains(&("push", Some(crate::standard::ARRAY_PUSH_METHOD_ID))));
    assert!(value_methods.contains(&("pop", Some(crate::standard::ARRAY_POP_METHOD_ID))));
    assert!(value_methods.contains(&("clear", Some(crate::standard::ARRAY_CLEAR_METHOD_ID))));
    assert!(value_methods.contains(&("has", Some(crate::standard::MAP_HAS_METHOD_ID))));
    assert!(value_methods.contains(&("set", Some(crate::standard::MAP_SET_METHOD_ID))));
    assert!(value_methods.contains(&("remove", Some(crate::standard::MAP_REMOVE_METHOD_ID))));
    assert!(value_methods.contains(&("clear", Some(crate::standard::MAP_CLEAR_METHOD_ID))));
    assert!(value_methods.contains(&("has", Some(crate::standard::SET_HAS_METHOD_ID))));
    assert!(value_methods.contains(&("is_subset", Some(crate::standard::SET_IS_SUBSET_METHOD_ID))));
    assert!(value_methods.contains(&(
        "is_superset",
        Some(crate::standard::SET_IS_SUPERSET_METHOD_ID)
    )));
    assert!(value_methods.contains(&(
        "is_disjoint",
        Some(crate::standard::SET_IS_DISJOINT_METHOD_ID)
    )));
}

#[test]
fn engine_compiler_options_lower_named_standard_value_method_arguments() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main() {
    let pair = "reward:gold".split_once(separator = ":").unwrap_or(["", ""]);
    return {"gold": 4}.get_or(default = 0, key = pair[1]);
}
"#,
        &engine.compiler_options(),
    )
    .expect("named stdlib value method arguments should compile");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(OwnedValue::Int(4))
    );
}

#[test]
fn engine_compiler_options_lower_receiver_specific_named_standard_value_method_arguments() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main() {
    return "reward:gold".contains(needle = ":") && ["gold"].contains(value = "gold");
}
"#,
        &engine.compiler_options(),
    )
    .expect("receiver-specific named stdlib value method arguments should compile");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(OwnedValue::Bool(true))
    );
}

#[test]
fn engine_compiler_options_lower_local_receiver_named_standard_value_method_arguments() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = compile_program_source_with_options(
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
        &engine.compiler_options(),
    )
    .expect("local receiver named stdlib value method arguments should compile");

    assert_eq!(
        engine.into_vm().run_program(
            &program,
            "main",
            &[OwnedValue::String("loot:xp".to_owned())]
        ),
        Ok(OwnedValue::Bool(true))
    );
}

#[test]
fn engine_compiler_options_reject_ambiguous_named_standard_value_method_arguments() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(value) {
    return value.contains(needle = ":");
}
"#,
        &engine.compiler_options(),
    )
    .expect_err("ambiguous stdlib value method names should not accept named args");
}

#[test]
fn engine_builder_installs_standard_natives_into_runtime() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let tags = set::from_array(["fire", "ice", "fire"]);
    let midpoint = math::floor(math::lerp(10, 20, 0.5));
    let range = math::round(math::distance3d(0, 0, 0, 2, 3, 6));
    let score = math::pow(2, 3);
    let root = math::round(math::sqrt(81));
    let direction = math::sign(-3);
    let approach = math::move_towards(0, 10, 4);
    return tags.len() + option::unwrap_or(option::some(midpoint), 0) + math::round(1.5) + range + score + root + direction + approach;
}
"#,
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    let result = runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx);
    assert_eq!(result, Ok(OwnedValue::Int(46)),);
}
