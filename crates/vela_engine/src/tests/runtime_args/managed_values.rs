use super::*;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

fn script_record_field<'value>(
    value: &'value OwnedValue,
    field: &str,
) -> Option<&'value OwnedValue> {
    let OwnedValue::Record { fields, .. } = value else {
        return None;
    };
    fields.get(field)
}

#[test]
fn runtime_host_global_decl_reads_and_writes_persistent_host_object() {
    let engine = Engine::builder()
        .register_type(direct_player_type())
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
global state: Player;

fn main() {
    state.level += 2;
    return state.level;
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let global = runtime.insert_host_global("main::state", direct_player(9));

    let result = runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect("runtime call should run");

    assert_eq!(runtime.host_global_ref("main::state"), Some(global));
    assert_eq!(
        runtime.value_to_owned(&result),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(11)))
    );
}

#[test]
fn runtime_host_global_decl_uses_slotted_lookup_without_fallback_name_lookup() {
    let engine = Engine::builder()
        .register_type(direct_player_type())
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
global state: Player;

fn main() {
    return state.level;
}
"#,
        )
        .expect("program should compile");
    assert!(
        program.global_slot("main::state").is_some(),
        "declared global should have a hot-path slot"
    );
    let mut runtime = Runtime::new(engine, program);
    runtime.insert_host_global("main::state", direct_player(9));
    let mut fallback = CountingGlobalLookupAdapter::default();

    let result = runtime
        .call_with_adapter(
            "main",
            CallArgs::new(),
            CallOptions::unbounded(),
            &mut fallback,
        )
        .expect("runtime call should run");

    assert_eq!(
        runtime.value_to_owned(&result),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(9)))
    );
    assert_eq!(fallback.global_ref_by_slot_calls.get(), 0);
    assert_eq!(fallback.global_ref_calls.get(), 0);
}

#[test]
fn runtime_host_global_decl_requires_host_inserted_instance() {
    let engine = Engine::builder()
        .register_type(direct_player_type())
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
global state: Player;

fn main() {
    return state.level;
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);

    let error = runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect_err("missing global should fail");

    assert_eq!(
        error.kind(),
        VmErrorKind::Host(HostErrorKind::MissingGlobal {
            name: "main::state".to_owned()
        })
    );
}

#[test]
fn runtime_script_global_decl_persists_vm_owned_value_and_rust_updates() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
struct ServerState {
    level: i64,
    name: String,
}

global state: ServerState;

fn make_state() {
    return ServerState { level: 5, name: "boot" };
}

fn bump(amount) {
    state.level += amount;
    return state.level;
}

fn read_name() {
    return state.name;
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);

    let state = runtime
        .call("make_state", CallArgs::new(), CallOptions::unbounded())
        .expect("factory should run");
    runtime
        .insert_global("main::state", state)
        .expect("script global should insert");

    let first = runtime
        .call(
            "bump",
            CallArgs::from_positional([OwnedValue::Scalar(vela_common::ScalarValue::I64(2))]),
            CallOptions::unbounded(),
        )
        .expect("first bump should run");
    let second = runtime
        .call(
            "bump",
            CallArgs::from_positional([OwnedValue::Scalar(vela_common::ScalarValue::I64(3))]),
            CallOptions::unbounded(),
        )
        .expect("second bump should run");

    assert_eq!(
        runtime.value_to_owned(&first),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(7)))
    );
    assert_eq!(
        runtime.value_to_owned(&second),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(10)))
    );
    assert_eq!(
        script_record_field(
            &runtime
                .global("main::state")
                .expect("script global should materialize")
                .expect("script global should exist"),
            "level",
        ),
        Some(&OwnedValue::Scalar(vela_common::ScalarValue::I64(10)))
    );

    runtime
        .update_global("main::state", |value| {
            let OwnedValue::Record { fields, .. } = value else {
                panic!("state should remain a record");
            };
            fields
                .set_existing(
                    "level",
                    OwnedValue::Scalar(vela_common::ScalarValue::I64(40)),
                )
                .expect("level field should exist");
        })
        .expect("rust update should replace persistent global");

    let after_rust_update = runtime
        .call(
            "bump",
            CallArgs::from_positional([OwnedValue::Scalar(vela_common::ScalarValue::I64(1))]),
            CallOptions::unbounded(),
        )
        .expect("bump after rust update should run");
    let name = runtime
        .call("read_name", CallArgs::new(), CallOptions::unbounded())
        .expect("read name should run");

    assert_eq!(
        runtime.value_to_owned(&after_rust_update),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(41)))
    );
    assert_eq!(
        runtime.value_to_owned(&name),
        Ok(OwnedValue::String("boot".to_owned()))
    );
}

#[test]
fn runtime_script_global_nested_record_program_links() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
struct ServerStats {
    handled_ticks: i64,
}

struct ServerState {
    level: i64,
    name: String,
    total_gold: i64,
    stats: ServerStats,
}

global state: ServerState;

fn handle_tick(level_gain, gold_gain) {
    state.level += level_gain;
    state.total_gold += gold_gain;
    state.stats.handled_ticks += 1;
    return state.level + state.total_gold + state.stats.handled_ticks;
}

fn projected_score(snapshot: ServerState, bonus) {
    return snapshot.level + snapshot.total_gold + snapshot.stats.handled_ticks + bonus;
}
"#,
        )
        .expect("program should compile");

    engine
        .link_program(&program)
        .expect("nested script global program should link");
}

#[test]
fn shared_runtime_image_keeps_script_globals_isolated() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
struct ServerState {
    level: i64,
    name: String,
}

global state: ServerState;

fn make_state(level, name) {
    return ServerState { level: level, name: name };
}

fn bump(amount) {
    state.level += amount;
    return state.level;
}

fn read_name() {
    return state.name;
}
"#,
        )
        .expect("program should compile");
    let shared_image = RuntimeImage::new(engine, program).into_shared();
    let mut first = SharedRuntime::from_shared_image(shared_image.clone());
    let mut second = SharedRuntime::from_shared_image(shared_image);

    let first_state = first
        .call(
            "make_state",
            CallArgs::from_positional([
                OwnedValue::Scalar(vela_common::ScalarValue::I64(5)),
                OwnedValue::String("first".into()),
            ]),
            CallOptions::unbounded(),
        )
        .expect("first factory should run");
    let second_state = second
        .call(
            "make_state",
            CallArgs::from_positional([
                OwnedValue::Scalar(vela_common::ScalarValue::I64(40)),
                OwnedValue::String("second".into()),
            ]),
            CallOptions::unbounded(),
        )
        .expect("second factory should run");

    first
        .insert_global("main::state", first_state)
        .expect("first script global should insert");
    second
        .insert_global("main::state", second_state)
        .expect("second script global should insert");

    let first_bumped = first
        .call(
            "bump",
            CallArgs::from_positional([OwnedValue::Scalar(vela_common::ScalarValue::I64(2))]),
            CallOptions::unbounded(),
        )
        .expect("first bump should run");
    let second_bumped = second
        .call(
            "bump",
            CallArgs::from_positional([OwnedValue::Scalar(vela_common::ScalarValue::I64(3))]),
            CallOptions::unbounded(),
        )
        .expect("second bump should run");
    let first_name = first
        .call("read_name", CallArgs::new(), CallOptions::unbounded())
        .expect("first name should read");
    let second_name = second
        .call("read_name", CallArgs::new(), CallOptions::unbounded())
        .expect("second name should read");

    assert_eq!(
        first.value_to_owned(&first_bumped),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(7)))
    );
    assert_eq!(
        second.value_to_owned(&second_bumped),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(43)))
    );
    assert_eq!(
        first.value_to_owned(&first_name),
        Ok(OwnedValue::String("first".to_owned()))
    );
    assert_eq!(
        second.value_to_owned(&second_name),
        Ok(OwnedValue::String("second".to_owned()))
    );
}

#[test]
fn runtime_insert_global_rejects_type_contract_mismatch() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
global amount: i64;

fn read_amount() {
    return amount;
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);

    let error = runtime
        .insert_global("main::amount", OwnedValue::String("wrong".to_owned()))
        .expect_err("typed global insertion should reject mismatched value");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeContractViolation {
            expected: "i64".to_owned(),
            actual: "String".to_owned(),
            debug_name: "main::amount".to_owned(),
        }
    );
    assert_eq!(
        runtime.global("main::amount"),
        Ok(None),
        "rejected value must not enter the script global store"
    );
}

#[test]
fn runtime_update_global_rejects_type_contract_mismatch_without_replacing_value() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
struct ServerState {
    level: i64,
}

global state: ServerState;
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    runtime
        .insert_global(
            "main::state",
            OwnedValue::record("ServerState", [("level", OwnedValue::i64(3))]),
        )
        .expect("matching global should insert");

    let error = runtime
        .update_global("main::state", |value| {
            *value = OwnedValue::String("wrong".to_owned());
        })
        .expect_err("typed global update should reject mismatched replacement");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeContractViolation {
            expected: "ServerState".to_owned(),
            actual: "String".to_owned(),
            debug_name: "main::state".to_owned(),
        }
    );
    assert_eq!(
        script_record_field(
            &runtime
                .global("main::state")
                .expect("global should materialize")
                .expect("original global should remain"),
            "level",
        ),
        Some(&OwnedValue::i64(3))
    );
}

#[cfg(feature = "serde")]
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
struct SerdeServerState {
    level: i64,
    name: String,
}

#[cfg(feature = "serde")]
#[test]
fn runtime_insert_global_accepts_serde_struct_with_single_api() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
struct SerdeServerState {
    level: i64,
    name: String,
}

global state: SerdeServerState;

fn bump(amount) {
    state.level += amount;
    return state.level;
}

fn read_name() {
    return state.name;
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let state = SerdeServerState {
        level: 5,
        name: "serde".to_owned(),
    };

    runtime
        .insert_global("main::state", &state)
        .expect("serde global should insert through unified API");

    let level_value = runtime
        .call(
            "bump",
            CallArgs::from_positional([OwnedValue::Scalar(vela_common::ScalarValue::I64(4))]),
            CallOptions::unbounded(),
        )
        .expect("bump should run");
    let name_value = runtime
        .call("read_name", CallArgs::new(), CallOptions::unbounded())
        .expect("read name should run");
    let level: i64 = runtime
        .from_value(&level_value)
        .expect("level value should deserialize directly");
    let name: String = runtime
        .from_value(&name_value)
        .expect("name value should deserialize directly");
    let global: SerdeServerState = runtime
        .global_as("main::state")
        .expect("script global should deserialize directly")
        .expect("script global should exist");

    assert_eq!(state.level, 5);
    assert_eq!(level, 9);
    assert_eq!(name, "serde");
    assert_eq!(
        global,
        SerdeServerState {
            level: 9,
            name: "serde".to_owned()
        }
    );
}

#[cfg(feature = "serde")]
#[test]
fn runtime_from_value_rejects_non_string_map_keys_without_loss() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
fn make_scores() {
    return [
        MapEntry { key: 1, value: 10 },
        MapEntry { key: 2, value: 20 },
    ].iter().collect_map();
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);

    let scores = runtime
        .call("make_scores", CallArgs::new(), CallOptions::unbounded())
        .expect("scores should be returned as runtime value");
    let error = runtime
        .from_value::<BTreeMap<String, i64>>(&scores)
        .expect_err("runtime serde object maps require string keys");

    assert!(matches!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "serde owned value conversion",
        }
    ));
    assert_eq!(
        runtime
            .value_to_owned(&scores)
            .expect("runtime value can materialize without key loss"),
        OwnedValue::map([(1_i64, 10_i64), (2_i64, 20_i64)])
    );
}

#[cfg(feature = "serde")]
#[test]
fn runtime_call_accepts_serde_non_string_map_keys() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
fn lookup_score(scores: Map<i64, i64>) -> i64 {
    return scores[2];
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let scores = BTreeMap::from([(1_i64, 10_i64), (2_i64, 20_i64)]);

    let value = runtime
        .call(
            "lookup_score",
            CallArgs::new()
                .with_serde(&scores)
                .expect("scores should serialize as an owned value"),
            CallOptions::unbounded(),
        )
        .expect("numeric-key map should cross the serde call boundary");

    assert_eq!(
        runtime.from_value::<i64>(&value),
        Ok(20),
        "script lookup should use the numeric ValueKey, not string coercion"
    );
}

#[test]
fn runtime_insert_global_accepts_runtime_managed_value_with_single_api() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
struct ServerState {
    level: i64,
    name: String,
}

global state: ServerState;

fn make_state() {
    return ServerState { level: 11, name: "runtime" };
}

fn read_level() {
    return state.level;
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);

    let state = runtime
        .call("make_state", CallArgs::new(), CallOptions::unbounded())
        .expect("factory should return runtime-managed value");
    runtime
        .insert_global("main::state", state)
        .expect("runtime value should insert through unified API");

    let level = runtime
        .call("read_level", CallArgs::new(), CallOptions::unbounded())
        .expect("read level should run");
    assert_eq!(
        runtime.value_to_owned(&level),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(11)))
    );
}

#[test]
fn runtime_call_returns_runtime_managed_value_that_can_be_passed_back() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
struct Reward {
    gold: i64,
    xp: i64,
}

fn make_reward() {
    return Reward { gold: 7, xp: 3 };
}

fn reward_score(reward: Reward, bonus) {
    return reward.gold + reward.xp + bonus;
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);

    let reward = runtime
        .call("make_reward", CallArgs::new(), CallOptions::unbounded())
        .expect("reward should be returned as runtime value");
    let score = runtime
        .call(
            "reward_score",
            CallArgs::new()
                .with_vela_value(reward.clone())
                .with(OwnedValue::Scalar(vela_common::ScalarValue::I64(5))),
            CallOptions::unbounded(),
        )
        .expect("runtime value should pass back without owned materialization");

    assert_eq!(
        runtime.value_to_owned(&score),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(15)))
    );
    assert_eq!(
        script_record_field(
            &runtime
                .value_to_owned(&reward)
                .expect("runtime value can materialize on demand"),
            "gold",
        ),
        Some(&OwnedValue::Scalar(vela_common::ScalarValue::I64(7)))
    );
}

#[test]
fn retained_runtime_value_survives_script_global_collection() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
struct Reward {
    gold: i64,
    label: String,
}

global scratch: Reward;

fn make_reward(gold, label) {
    return Reward { gold: gold, label: label };
}

fn reward_score(reward: Reward) {
    return reward.gold;
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);

    let retained = runtime
        .call(
            "make_reward",
            CallArgs::from_positional([
                OwnedValue::Scalar(vela_common::ScalarValue::I64(7)),
                OwnedValue::String("retained".to_owned()),
            ]),
            CallOptions::unbounded(),
        )
        .expect("retained reward should be returned as runtime value");
    let scratch = runtime
        .call(
            "make_reward",
            CallArgs::from_positional([
                OwnedValue::Scalar(vela_common::ScalarValue::I64(99)),
                OwnedValue::String("scratch".to_owned()),
            ]),
            CallOptions::unbounded(),
        )
        .expect("scratch reward should be returned as runtime value");

    runtime
        .insert_global("main::scratch", scratch)
        .expect("inserting a script global should trigger persistent heap collection");

    let score = runtime
        .call(
            "reward_score",
            CallArgs::new().with_vela_value(retained.clone()),
            CallOptions::unbounded(),
        )
        .expect("retained runtime value should remain rooted after collection");

    assert_eq!(
        runtime.value_to_owned(&score),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(7)))
    );
    assert_eq!(
        script_record_field(
            &runtime
                .value_to_owned(&retained)
                .expect("retained runtime value can still materialize"),
            "label",
        ),
        Some(&OwnedValue::String("retained".to_owned()))
    );
}

#[test]
fn runtime_call_rejects_values_from_another_runtime() {
    let engine = Engine::builder().build().expect("engine should build");
    let source = r#"
struct Reward {
    gold: i64,
}

fn make_reward() {
    return Reward { gold: 7 };
}

fn read_reward(reward: Reward) {
    return reward.gold;
}
"#;
    let program_a = engine
        .compile_source_with_id(SourceId::new(1), source)
        .expect("program should compile");
    let program_b = engine
        .compile_source_with_id(SourceId::new(2), source)
        .expect("program should compile");
    let mut runtime_a = Runtime::new(engine.clone(), program_a);
    let mut runtime_b = Runtime::new(engine, program_b);

    let reward = runtime_a
        .call("make_reward", CallArgs::new(), CallOptions::unbounded())
        .expect("runtime value should be created");
    let error = runtime_b
        .call(
            "read_reward",
            CallArgs::new().with_vela_value(reward),
            CallOptions::unbounded(),
        )
        .expect_err("runtime values must not cross runtime heaps");

    assert!(matches!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "VelaValue belongs to another Runtime",
        }
    ));
}
