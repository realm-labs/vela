use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use vela_bytecode::compiler::compile_program_source;
use vela_common::SourceId;
use vela_vm::{VmError, VmErrorKind};

use crate::{Engine, NativeFunctionDesc, NativeFunctionId, TypeHint, Value};

#[test]
fn engine_registers_typed_native_functions() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(i64, i64), _>(
            NativeFunctionDesc::new("game.add", NativeFunctionId::new(101))
                .param("lhs", TypeHint::Int)
                .param("rhs", TypeHint::Int)
                .returns(TypeHint::Int),
            |lhs: i64, rhs: i64| lhs + rhs,
        )
        .register_typed_native_fn::<(), _>(
            NativeFunctionDesc::new("game.label", NativeFunctionId::new(102))
                .returns(TypeHint::String),
            || "typed",
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return game.add(2, 3) + game.label().len();
}
"#,
    )
    .expect("program should compile");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(10)),
    );
}

#[test]
fn typed_native_functions_accept_four_script_args() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(i64, i64, i64, i64), _>(
            NativeFunctionDesc::new("game.sum4", NativeFunctionId::new(221))
                .param("a", TypeHint::Int)
                .param("b", TypeHint::Int)
                .param("c", TypeHint::Int)
                .param("d", TypeHint::Int)
                .returns(TypeHint::Int),
            |a: i64, b: i64, c: i64, d: i64| a + b + c + d,
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return game.sum4(1, 2, 3, 4);
}
"#,
    )
    .expect("program should compile");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(10)),
    );
}

#[test]
fn typed_native_functions_accept_five_script_args() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(i64, i64, i64, i64, i64), _>(
            NativeFunctionDesc::new("game.sum5", NativeFunctionId::new(229))
                .param("a", TypeHint::Int)
                .param("b", TypeHint::Int)
                .param("c", TypeHint::Int)
                .param("d", TypeHint::Int)
                .param("e", TypeHint::Int)
                .returns(TypeHint::Int),
            |a: i64, b: i64, c: i64, d: i64, e: i64| a + b + c + d + e,
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return game.sum5(1, 2, 3, 4, 5);
}
"#,
    )
    .expect("program should compile");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(15)),
    );
}

#[test]
fn typed_native_functions_accept_six_script_args() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(i64, i64, i64, i64, i64, i64), _>(
            NativeFunctionDesc::new("game.sum6", NativeFunctionId::new(237))
                .param("a", TypeHint::Int)
                .param("b", TypeHint::Int)
                .param("c", TypeHint::Int)
                .param("d", TypeHint::Int)
                .param("e", TypeHint::Int)
                .param("f", TypeHint::Int)
                .returns(TypeHint::Int),
            |a: i64, b: i64, c: i64, d: i64, e: i64, f: i64| a + b + c + d + e + f,
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return game.sum6(1, 2, 3, 4, 5, 6);
}
"#,
    )
    .expect("program should compile");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(21)),
    );
}

#[test]
fn typed_native_functions_accept_optional_values() {
    let engine = Engine::builder()
        .with_standard_natives()
        .register_typed_native_fn::<(Option<i64>,), _>(
            NativeFunctionDesc::new("game.option_bonus", NativeFunctionId::new(108))
                .param("bonus", TypeHint::Any)
                .returns(TypeHint::Int),
            |bonus: Option<i64>| bonus.unwrap_or(7),
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return game.option_bonus(null)
        + game.option_bonus(5)
        + game.option_bonus(option.none())
        + game.option_bonus(option.some(9));
}
"#,
    )
    .expect("program should compile");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(28)),
    );
}

#[test]
fn typed_native_functions_accept_f32_values() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(f32,), _>(
            NativeFunctionDesc::new("game.scale_weight", NativeFunctionId::new(228))
                .param("weight", TypeHint::Float)
                .returns(TypeHint::Float),
            |weight: f32| weight * 2.0,
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return game.scale_weight(1.5);
}
"#,
    )
    .expect("program should compile");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Float(3.0)),
    );
}

#[test]
fn typed_native_functions_accept_set_values() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(BTreeSet<String>,), _>(
            NativeFunctionDesc::new("game.count_tags", NativeFunctionId::new(224))
                .param("tags", TypeHint::Set)
                .returns(TypeHint::Int),
            |tags: BTreeSet<String>| i64::try_from(tags.len()).expect("set length fits i64"),
        )
        .register_typed_native_fn::<(), _>(
            NativeFunctionDesc::new("game.reward_tags", NativeFunctionId::new(225))
                .returns(TypeHint::Set),
            || BTreeSet::from(["daily".to_owned(), "quest".to_owned()]),
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(tags) {
    return game.count_tags(tags) + game.reward_tags().len();
}
"#,
    )
    .expect("program should compile");

    assert_eq!(
        engine.into_vm().run_program(
            &program,
            "main",
            &[Value::Set(vec![
                Value::String("fire".to_owned()),
                Value::String("ice".to_owned()),
                Value::String("fire".to_owned()),
            ])],
        ),
        Ok(Value::Int(4)),
    );
}

#[test]
fn typed_native_functions_accept_hash_set_values() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(HashSet<String>,), _>(
            NativeFunctionDesc::new("game.count_unordered_tags", NativeFunctionId::new(235))
                .param("tags", TypeHint::Set)
                .returns(TypeHint::Int),
            |tags: HashSet<String>| i64::try_from(tags.len()).expect("set length fits i64"),
        )
        .register_typed_native_fn::<(), _>(
            NativeFunctionDesc::new("game.unordered_reward_tags", NativeFunctionId::new(236))
                .returns(TypeHint::Set),
            || HashSet::from(["daily".to_owned(), "quest".to_owned()]),
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(tags) {
    return game.count_unordered_tags(tags) + game.unordered_reward_tags().len();
}
"#,
    )
    .expect("program should compile");

    assert_eq!(
        engine.into_vm().run_program(
            &program,
            "main",
            &[Value::Set(vec![
                Value::String("fire".to_owned()),
                Value::String("ice".to_owned()),
                Value::String("fire".to_owned()),
            ])],
        ),
        Ok(Value::Int(4)),
    );
}

#[test]
fn typed_native_functions_accept_fixed_array_values() {
    let engine = Engine::builder()
        .register_typed_native_fn::<([i64; 3],), _>(
            NativeFunctionDesc::new("game.sum_weights", NativeFunctionId::new(237))
                .param("weights", TypeHint::Array)
                .returns(TypeHint::Int),
            |weights: [i64; 3]| weights.iter().sum::<i64>(),
        )
        .register_typed_native_fn::<(), _>(
            NativeFunctionDesc::new("game.default_weights", NativeFunctionId::new(238))
                .returns(TypeHint::Array),
            || [2_i64, 4, 6],
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(weights) {
    return game.sum_weights(weights) + game.default_weights().sum();
}
"#,
    )
    .expect("program should compile");

    assert_eq!(
        engine.into_vm().run_program(
            &program,
            "main",
            &[Value::Array(vec![
                Value::Int(3),
                Value::Int(5),
                Value::Int(7),
            ])],
        ),
        Ok(Value::Int(27)),
    );
}

#[test]
fn typed_native_functions_accept_hash_map_values() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(HashMap<String, i64>,), _>(
            NativeFunctionDesc::new("game.sum_scores", NativeFunctionId::new(226))
                .param("scores", TypeHint::Map)
                .returns(TypeHint::Int),
            |scores: HashMap<String, i64>| scores.values().sum::<i64>(),
        )
        .register_typed_native_fn::<(), _>(
            NativeFunctionDesc::new("game.default_scores", NativeFunctionId::new(227))
                .returns(TypeHint::Map),
            || HashMap::from([("quest".to_owned(), 4_i64), ("raid".to_owned(), 6_i64)]),
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(scores) {
    let defaults = game.default_scores();
    return game.sum_scores(scores) + defaults.get_or("quest", 0);
}
"#,
    )
    .expect("program should compile");

    assert_eq!(
        engine.into_vm().run_program(
            &program,
            "main",
            &[Value::Map(
                [
                    ("daily".to_owned(), Value::Int(2)),
                    ("weekly".to_owned(), Value::Int(5)),
                ]
                .into(),
            )],
        ),
        Ok(Value::Int(11)),
    );
}

#[test]
fn typed_native_functions_accept_btree_map_values() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(BTreeMap<String, i64>,), _>(
            NativeFunctionDesc::new("game.sum_ordered_scores", NativeFunctionId::new(233))
                .param("scores", TypeHint::Map)
                .returns(TypeHint::Int),
            |scores: BTreeMap<String, i64>| scores.values().sum::<i64>(),
        )
        .register_typed_native_fn::<(), _>(
            NativeFunctionDesc::new("game.default_ordered_scores", NativeFunctionId::new(234))
                .returns(TypeHint::Map),
            || BTreeMap::from([("quest".to_owned(), 4_i64), ("raid".to_owned(), 6_i64)]),
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(scores) {
    let defaults = game.default_ordered_scores();
    return game.sum_ordered_scores(scores) + defaults.get_or("raid", 0);
}
"#,
    )
    .expect("program should compile");

    assert_eq!(
        engine.into_vm().run_program(
            &program,
            "main",
            &[Value::Map(
                [
                    ("daily".to_owned(), Value::Int(2)),
                    ("weekly".to_owned(), Value::Int(5)),
                ]
                .into(),
            )],
        ),
        Ok(Value::Int(13)),
    );
}

#[test]
fn typed_native_functions_return_dynamic_result_values() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(bool,), _>(
            NativeFunctionDesc::new("game.checked_bonus", NativeFunctionId::new(109))
                .param("ok", TypeHint::Bool)
                .returns(TypeHint::Any),
            |ok: bool| -> std::result::Result<i64, String> {
                if ok { Ok(11) } else { Err("denied".to_owned()) }
            },
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return game.checked_bonus(false);
}
"#,
    )
    .expect("program should compile");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Enum {
            enum_name: "Result".to_owned(),
            variant: "Err".to_owned(),
            fields: [("0".to_owned(), Value::String("denied".to_owned()))].into(),
        }),
    );
}

#[test]
fn typed_native_functions_report_arity_and_type_errors() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(i64, i64), _>(
            NativeFunctionDesc::new("game.add", NativeFunctionId::new(103)),
            |lhs: i64, rhs: i64| lhs + rhs,
        )
        .build()
        .expect("engine should build");
    let function = engine
        .native_function_by_name("game.add")
        .expect("typed native should be registered");

    assert!(matches!(
        (function.function)(&[Value::Int(1)]),
        Err(VmError {
            kind: VmErrorKind::ArityMismatch {
                expected: 2,
                actual: 1,
                ..
            },
            ..
        })
    ));
    assert!(matches!(
        (function.function)(&[Value::String("x".to_owned()), Value::Int(1)]),
        Err(VmError {
            kind: VmErrorKind::TypeMismatch { operation: "int" },
            ..
        })
    ));
}
