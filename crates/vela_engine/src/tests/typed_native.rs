use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use vela_bytecode::UnlinkedProgram;
use vela_common::{HostObjectId, HostTypeId, SourceId};
use vela_def::{FieldId, TypeId};
use vela_host::error::{HostError, HostErrorKind, HostResult};
use vela_host::path::{HostPath, HostRef};
use vela_host::proxy::PathProxy;
use vela_reflect::registry::TypeKey;
use vela_vm::budget::ExecutionBudget;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::native::{NativeFunctionDesc, NativeFunctionId, TypeHint};

fn run_linked_program(
    engine: &Engine,
    program: &UnlinkedProgram,
    args: &[OwnedValue],
) -> VmResult<OwnedValue> {
    let linked = engine
        .link_program(program)
        .expect("engine typed native test program should link");
    let mut budget = ExecutionBudget::unbounded();
    engine
        .into_vm_for_program(program)
        .run_linked_program_with_budget(&linked, "main", args, &mut budget)
}

#[test]
fn engine_registers_typed_native_functions() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(i64, i64), _>(
            NativeFunctionDesc::new("game::add", NativeFunctionId::new(101))
                .param("lhs", TypeHint::Int)
                .param("rhs", TypeHint::Int)
                .returns(TypeHint::Int),
            |lhs: i64, rhs: i64| lhs + rhs,
        )
        .register_typed_native_fn::<(), _>(
            NativeFunctionDesc::new("game::label", NativeFunctionId::new(102))
                .returns(TypeHint::String),
            || "typed",
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    if game::label() == "typed" {
        return game::add(2, 3) + 5;
    }
    return 0;
}
"#,
        )
        .expect("program should compile");

    assert_eq!(
        run_linked_program(&engine, &program, &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(10))),
    );
}

#[test]
fn typed_native_functions_accept_string_values() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(String,), _>(
            NativeFunctionDesc::new("game::tag_len", NativeFunctionId::new(241))
                .param("tag", TypeHint::String)
                .returns(TypeHint::Int),
            |tag: String| i64::try_from(tag.len()).expect("tag length fits i64"),
        )
        .register_typed_native_fn::<(), _>(
            NativeFunctionDesc::new("game::default_tag", NativeFunctionId::new(242))
                .returns(TypeHint::String),
            || "quest".to_owned(),
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    return game::tag_len("dragon") + game::tag_len(game::default_tag());
}
"#,
        )
        .expect("program should compile");

    assert_eq!(
        run_linked_program(&engine, &program, &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(11))),
    );
}

#[test]
fn typed_native_functions_accept_host_refs() {
    let player_type = TypeHint::Host(TypeKey::new(TypeId::new(1), "Player"));
    let engine = Engine::builder()
        .register_typed_native_fn::<(HostRef,), _>(
            NativeFunctionDesc::new("game::host_generation", NativeFunctionId::new(243))
                .param("player", player_type.clone())
                .returns(TypeHint::Int),
            |player: HostRef| i64::from(player.generation),
        )
        .register_typed_native_fn::<(HostRef,), _>(
            NativeFunctionDesc::new("game::host_object_id", NativeFunctionId::new(244))
                .param("player", player_type)
                .returns(TypeHint::Int),
            |player: HostRef| {
                i64::try_from(player.object_id.get()).expect("host object id fits i64")
            },
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player) {
    return game::host_generation(player) + game::host_object_id(player);
}
"#,
        )
        .expect("program should compile");
    let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 7);

    assert_eq!(
        run_linked_program(&engine, &program, &[OwnedValue::HostRef(player)]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(49))),
    );
}

#[test]
fn typed_native_functions_accept_path_proxies() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(PathProxy,), _>(
            NativeFunctionDesc::new("game::path_depth", NativeFunctionId::new(247))
                .param("path", TypeHint::PathProxy)
                .returns(TypeHint::Int),
            |path: PathProxy| {
                i64::try_from(path.to_diagnostic_path().segments.len())
                    .expect("path depth fits i64")
            },
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(path) {
    return game::path_depth(path);
}
"#,
        )
        .expect("program should compile");
    let registry = engine.registry();
    let reflected = registry
        .function_by_name("game::path_depth")
        .expect("typed native should register metadata");
    assert_eq!(reflected.params[0].type_hint.as_deref(), Some("path_proxy"));
    let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 7);
    let path =
        PathProxy::from_diagnostic_path(HostPath::new(player).field(FieldId::new(3)).index(2));

    assert_eq!(
        run_linked_program(&engine, &program, &[OwnedValue::PathProxy(path)]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2))),
    );
}

#[test]
fn typed_native_functions_accept_four_script_args() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(i64, i64, i64, i64), _>(
            NativeFunctionDesc::new("game::sum4", NativeFunctionId::new(221))
                .param("a", TypeHint::Int)
                .param("b", TypeHint::Int)
                .param("c", TypeHint::Int)
                .param("d", TypeHint::Int)
                .returns(TypeHint::Int),
            |a: i64, b: i64, c: i64, d: i64| a + b + c + d,
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    return game::sum4(1, 2, 3, 4);
}
"#,
        )
        .expect("program should compile");

    assert_eq!(
        run_linked_program(&engine, &program, &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(10))),
    );
}

#[test]
fn typed_native_functions_accept_five_script_args() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(i64, i64, i64, i64, i64), _>(
            NativeFunctionDesc::new("game::sum5", NativeFunctionId::new(229))
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
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    return game::sum5(1, 2, 3, 4, 5);
}
"#,
        )
        .expect("program should compile");

    assert_eq!(
        run_linked_program(&engine, &program, &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(15))),
    );
}

#[test]
fn typed_native_functions_accept_six_script_args() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(i64, i64, i64, i64, i64, i64), _>(
            NativeFunctionDesc::new("game::sum6", NativeFunctionId::new(237))
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
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    return game::sum6(1, 2, 3, 4, 5, 6);
}
"#,
        )
        .expect("program should compile");

    assert_eq!(
        run_linked_program(&engine, &program, &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(21))),
    );
}

#[test]
fn typed_native_functions_accept_optional_values() {
    let engine = Engine::builder()
        .with_standard_natives()
        .register_typed_native_fn::<(Option<i64>,), _>(
            NativeFunctionDesc::new("game::option_bonus", NativeFunctionId::new(108))
                .param("bonus", TypeHint::Any)
                .returns(TypeHint::Int),
            |bonus: Option<i64>| bonus.unwrap_or(7),
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    return game::option_bonus(null)
        + game::option_bonus(5)
        + game::option_bonus(option::none())
        + game::option_bonus(option::some(9));
}
"#,
        )
        .expect("program should compile");

    assert_eq!(
        run_linked_program(&engine, &program, &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(28))),
    );
}

#[test]
fn typed_native_functions_accept_f32_values() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(f32,), _>(
            NativeFunctionDesc::new("game::scale_weight", NativeFunctionId::new(228))
                .param("weight", TypeHint::Float)
                .returns(TypeHint::Float),
            |weight: f32| weight * 2.0,
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    return game::scale_weight(1.5);
}
"#,
        )
        .expect("program should compile");

    assert_eq!(
        run_linked_program(&engine, &program, &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::F64(3.0))),
    );
}

#[test]
fn typed_native_functions_accept_set_values() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(BTreeSet<String>,), _>(
            NativeFunctionDesc::new("game::count_tags", NativeFunctionId::new(224))
                .param("tags", TypeHint::Set)
                .returns(TypeHint::Int),
            |tags: BTreeSet<String>| i64::try_from(tags.len()).expect("set length fits i64"),
        )
        .register_typed_native_fn::<(), _>(
            NativeFunctionDesc::new("game::reward_tags", NativeFunctionId::new(225))
                .returns(TypeHint::Set),
            || BTreeSet::from(["daily".to_owned(), "quest".to_owned()]),
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(tags) {
    return game::count_tags(tags) + game::count_tags(game::reward_tags());
}
"#,
        )
        .expect("program should compile");

    assert_eq!(
        run_linked_program(
            &engine,
            &program,
            &[OwnedValue::Set(vec![
                OwnedValue::String("fire".to_owned()),
                OwnedValue::String("ice".to_owned()),
                OwnedValue::String("fire".to_owned()),
            ])],
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(4))),
    );
}

#[test]
fn typed_native_functions_accept_hash_set_values() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(HashSet<String>,), _>(
            NativeFunctionDesc::new("game::count_unordered_tags", NativeFunctionId::new(235))
                .param("tags", TypeHint::Set)
                .returns(TypeHint::Int),
            |tags: HashSet<String>| i64::try_from(tags.len()).expect("set length fits i64"),
        )
        .register_typed_native_fn::<(), _>(
            NativeFunctionDesc::new("game::unordered_reward_tags", NativeFunctionId::new(236))
                .returns(TypeHint::Set),
            || HashSet::from(["daily".to_owned(), "quest".to_owned()]),
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(tags) {
    return game::count_unordered_tags(tags) + game::count_unordered_tags(game::unordered_reward_tags());
}
"#,
        )
        .expect("program should compile");

    assert_eq!(
        run_linked_program(
            &engine,
            &program,
            &[OwnedValue::Set(vec![
                OwnedValue::String("fire".to_owned()),
                OwnedValue::String("ice".to_owned()),
                OwnedValue::String("fire".to_owned()),
            ])],
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(4))),
    );
}

#[test]
fn typed_native_functions_accept_fixed_array_values() {
    let engine = Engine::builder()
        .register_typed_native_fn::<([i64; 3],), _>(
            NativeFunctionDesc::new("game::sum_weights", NativeFunctionId::new(237))
                .param("weights", TypeHint::Array)
                .returns(TypeHint::Int),
            |weights: [i64; 3]| weights.iter().sum::<i64>(),
        )
        .register_typed_native_fn::<(), _>(
            NativeFunctionDesc::new("game::default_weights", NativeFunctionId::new(238))
                .returns(TypeHint::Array),
            || [2_i64, 4, 6],
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(weights) {
    return game::sum_weights(weights) + game::sum_weights(game::default_weights());
}
"#,
        )
        .expect("program should compile");

    assert_eq!(
        run_linked_program(
            &engine,
            &program,
            &[OwnedValue::Array(vec![
                OwnedValue::Scalar(vela_common::ScalarValue::I64(3)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(5)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(7)),
            ])],
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(27))),
    );
}

#[test]
fn typed_native_functions_accept_vec_values() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(Vec<i64>,), _>(
            NativeFunctionDesc::new("game::sum_rewards", NativeFunctionId::new(239))
                .param("rewards", TypeHint::Array)
                .returns(TypeHint::Int),
            |rewards: Vec<i64>| rewards.iter().sum::<i64>(),
        )
        .register_typed_native_fn::<(), _>(
            NativeFunctionDesc::new("game::default_rewards", NativeFunctionId::new(240))
                .returns(TypeHint::Array),
            || vec![2_i64, 4, 6],
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(rewards) {
    return game::sum_rewards(rewards) + game::sum_rewards(game::default_rewards());
}
"#,
        )
        .expect("program should compile");

    assert_eq!(
        run_linked_program(
            &engine,
            &program,
            &[OwnedValue::Array(vec![
                OwnedValue::Scalar(vela_common::ScalarValue::I64(3)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(5)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(7)),
            ])],
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(27))),
    );
}

#[test]
fn typed_native_functions_accept_hash_map_values() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(HashMap<String, i64>,), _>(
            NativeFunctionDesc::new("game::sum_scores", NativeFunctionId::new(226))
                .param("scores", TypeHint::Map)
                .returns(TypeHint::Int),
            |scores: HashMap<String, i64>| scores.values().sum::<i64>(),
        )
        .register_typed_native_fn::<(), _>(
            NativeFunctionDesc::new("game::default_scores", NativeFunctionId::new(227))
                .returns(TypeHint::Map),
            || HashMap::from([("quest".to_owned(), 4_i64), ("raid".to_owned(), 6_i64)]),
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(scores) {
    let defaults = game::default_scores();
    return game::sum_scores(scores) + game::sum_scores(defaults);
}
"#,
        )
        .expect("program should compile");

    assert_eq!(
        run_linked_program(
            &engine,
            &program,
            &[OwnedValue::Map(
                [
                    (
                        "daily".to_owned(),
                        OwnedValue::Scalar(vela_common::ScalarValue::I64(2))
                    ),
                    (
                        "weekly".to_owned(),
                        OwnedValue::Scalar(vela_common::ScalarValue::I64(5))
                    ),
                ]
                .into(),
            )],
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(17))),
    );
}

#[test]
fn typed_native_functions_accept_btree_map_values() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(BTreeMap<String, i64>,), _>(
            NativeFunctionDesc::new("game::sum_ordered_scores", NativeFunctionId::new(233))
                .param("scores", TypeHint::Map)
                .returns(TypeHint::Int),
            |scores: BTreeMap<String, i64>| scores.values().sum::<i64>(),
        )
        .register_typed_native_fn::<(), _>(
            NativeFunctionDesc::new("game::default_ordered_scores", NativeFunctionId::new(234))
                .returns(TypeHint::Map),
            || BTreeMap::from([("quest".to_owned(), 4_i64), ("raid".to_owned(), 6_i64)]),
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(scores) {
    let defaults = game::default_ordered_scores();
    return game::sum_ordered_scores(scores) + game::sum_ordered_scores(defaults);
}
"#,
        )
        .expect("program should compile");

    assert_eq!(
        run_linked_program(
            &engine,
            &program,
            &[OwnedValue::Map(
                [
                    (
                        "daily".to_owned(),
                        OwnedValue::Scalar(vela_common::ScalarValue::I64(2))
                    ),
                    (
                        "weekly".to_owned(),
                        OwnedValue::Scalar(vela_common::ScalarValue::I64(5))
                    ),
                ]
                .into(),
            )],
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(17))),
    );
}

#[test]
fn typed_native_functions_return_dynamic_result_values() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(bool,), _>(
            NativeFunctionDesc::new("game::checked_bonus", NativeFunctionId::new(109))
                .param("ok", TypeHint::Bool)
                .returns(TypeHint::Any),
            |ok: bool| -> std::result::Result<i64, String> {
                if ok { Ok(11) } else { Err("denied".to_owned()) }
            },
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    return game::checked_bonus(false);
}
"#,
        )
        .expect("program should compile");

    assert_eq!(
        run_linked_program(&engine, &program, &[]),
        Ok(OwnedValue::Enum {
            enum_name: "Result".to_owned(),
            variant: "Err".to_owned(),
            fields: [("0".to_owned(), OwnedValue::String("denied".to_owned()))].into(),
        }),
    );
}

#[test]
fn typed_native_functions_propagate_vm_result_errors() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(bool,), _>(
            NativeFunctionDesc::new("game::require_admin", NativeFunctionId::new(245))
                .param("allowed", TypeHint::Bool)
                .returns(TypeHint::Int),
            |allowed: bool| -> vela_vm::error::VmResult<i64> {
                if allowed {
                    Ok(17)
                } else {
                    Err(VmError::new(VmErrorKind::PermissionDenied {
                        native: "game::require_admin".to_owned(),
                        capability: "admin".to_owned(),
                    }))
                }
            },
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(allowed) {
    return game::require_admin(allowed);
}
"#,
        )
        .expect("program should compile");

    assert_eq!(
        run_linked_program(&engine, &program, &[OwnedValue::Bool(false)])
            .map_err(|error| error.kind()),
        Err(VmErrorKind::PermissionDenied {
            native: "game::require_admin".to_owned(),
            capability: "admin".to_owned(),
        }),
    );
}

#[test]
fn typed_native_functions_map_host_result_errors() {
    let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 7);
    let denied_path = HostPath::new(player);
    let expected_path = denied_path.clone();
    let engine = Engine::builder()
        .register_typed_native_fn::<(bool,), _>(
            NativeFunctionDesc::new("game::write_score", NativeFunctionId::new(246))
                .param("allowed", TypeHint::Bool)
                .returns(TypeHint::Int),
            move |allowed: bool| -> HostResult<i64> {
                if allowed {
                    Ok(21)
                } else {
                    Err(HostError {
                        kind: HostErrorKind::PermissionDenied {
                            path: denied_path.clone(),
                            action: "write",
                        },
                        source_span: None,
                    })
                }
            },
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(allowed) {
    return game::write_score(allowed);
}
"#,
        )
        .expect("program should compile");

    assert_eq!(
        run_linked_program(&engine, &program, &[OwnedValue::Bool(false)])
            .map_err(|error| error.kind()),
        Err(VmErrorKind::Host(HostErrorKind::PermissionDenied {
            path: expected_path,
            action: "write",
        })),
    );
}

#[test]
fn typed_native_functions_report_arity_and_type_errors() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(i64, i64), _>(
            NativeFunctionDesc::new("game::add", NativeFunctionId::new(103)),
            |lhs: i64, rhs: i64| lhs + rhs,
        )
        .build()
        .expect("engine should build");
    let function = engine
        .native_function_by_name("game::add")
        .expect("typed native should be registered");

    assert!(matches!(
        (function.function)(&[OwnedValue::Scalar(vela_common::ScalarValue::I64(1))]),
        Err(error) if matches!(error.kind(), VmErrorKind::ArityMismatch {
                expected: 2,
                actual: 1,
                ..
            })
    ));
    assert!(matches!(
        (function.function)(&[OwnedValue::String("x".to_owned()), OwnedValue::Scalar(vela_common::ScalarValue::I64(1))]),
        Err(error) if matches!(error.kind(), VmErrorKind::TypeMismatch { operation: "int" })
    ));
}
