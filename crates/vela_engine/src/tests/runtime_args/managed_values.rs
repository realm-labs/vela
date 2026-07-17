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
fn runtime_extern_state_reads_and_writes_persistent_host_object() {
    let engine = Engine::builder()
        .register_type(direct_player_type())
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
extern state state: Player;

fn main() {
    state.level += 2;
    return state.level;
}
"#,
        )
        .expect("program should compile");
    let mut builder = Runtime::builder(engine, program).expect("runtime image should link");
    let binding = builder
        .bind_extern_state("main::state", direct_player(9))
        .expect("extern state should bind");
    let mut runtime = builder.build().expect("runtime should initialize");

    let result = runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect("runtime call should run");

    assert_eq!(runtime.extern_state_ref("main::state"), Some(binding));
    assert_eq!(
        runtime.value_to_owned(&result),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(11)))
    );

    let replacement = runtime
        .replace_extern_state("main::state", direct_player(5))
        .expect("extern state should replace at the runtime boundary");
    let replaced = runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect("replacement should be visible");
    assert_eq!(runtime.extern_state_ref("main::state"), Some(replacement));
    assert_eq!(
        runtime.value_to_owned(&replaced),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(7)))
    );
}

#[test]
fn runtime_builder_rejects_mismatched_extern_state_type() {
    let engine = Engine::builder()
        .register_type(direct_player_type())
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source("extern state state: Player; fn main() { return state.level; }")
        .expect("program should compile");
    let mut builder = Runtime::builder(engine, program).expect("runtime image should link");
    let error = builder
        .bind_extern_state("main::state", WrongHostType)
        .expect_err("mismatched host type must be rejected");

    assert_eq!(
        error.kind,
        HostErrorKind::TypeMismatch {
            expected: HostTypeId::new(1),
            actual: HostTypeId::new(99),
        }
    );
}

#[test]
fn runtime_extern_state_uses_id_lookup_without_fallback_lookup() {
    let engine = Engine::builder()
        .register_type(direct_player_type())
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
extern state state: Player;

fn main() {
    return state.level;
}
"#,
        )
        .expect("program should compile");
    assert!(
        program.state_slot("main::state").is_some(),
        "declared extern state should have a generation slot"
    );
    let mut builder = Runtime::builder(engine, program).expect("runtime image should link");
    builder
        .bind_extern_state("main::state", direct_player(9))
        .expect("extern state should bind");
    let mut runtime = builder.build().expect("runtime should initialize");
    let mut fallback = CountingExternStateLookupAdapter::default();

    let result = runtime
        .call(
            "main",
            CallArgs::new().with_fallback_adapter(&mut fallback),
            CallOptions::unbounded(),
        )
        .expect("runtime call should run");

    assert_eq!(
        runtime.value_to_owned(&result),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(9)))
    );
    assert_eq!(fallback.extern_state_ref_calls.get(), 0);
}

#[test]
fn runtime_extern_state_requires_host_binding() {
    let engine = Engine::builder()
        .register_type(direct_player_type())
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
extern state state: Player;

fn main() {
    return state.level;
}
"#,
        )
        .expect("program should compile");
    let error = match Runtime::new(engine, program) {
        Ok(_) => panic!("missing extern state should reject runtime construction"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        RuntimeBuildError::MissingExternState { state, .. } if state == "main::state"
    ));
}

#[test]
fn reload_requires_and_transactionally_publishes_added_extern_state_binding() {
    let engine = Engine::builder()
        .register_type(direct_player_type())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(SourceId::new(21), "fn main() { return 1; }")
        .expect("initial generation");
    let update_source = r#"
extern state player: Player;
fn main() { return 1; }
fn player_level() { return player.level; }
"#;
    let missing_update = engine
        .compile_hot_reload_update_with_id(&initial, SourceId::new(22), update_source)
        .expect("state addition compiles");
    let mut runtime = Runtime::from_hot_reload_version(engine.clone(), initial.clone())
        .expect("runtime initializes");

    let rejected = runtime
        .apply_hot_update(missing_update)
        .expect("missing binding is a reload rejection");
    assert!(!rejected.accepted);
    assert_eq!(
        rejected.errors[0].code,
        "reload.state.extern_binding_missing"
    );
    assert!(runtime.extern_state_ref("main::player").is_none());

    let update = engine
        .compile_hot_reload_update_with_id(&initial, SourceId::new(23), update_source)
        .expect("retry update compiles");
    let binding = runtime
        .stage_extern_state("main::player", direct_player(12))
        .expect("binding stages");
    let accepted = runtime
        .apply_hot_update(update)
        .expect("apply bound update");

    assert!(accepted.accepted);
    assert_eq!(runtime.extern_state_ref("main::player"), Some(binding));
    let value = runtime
        .call("player_level", CallArgs::new(), CallOptions::unbounded())
        .expect("new extern state reads through HostAccess");
    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::from(12_i64)));
}

#[test]
fn reload_rejects_mismatched_staged_extern_state_binding() {
    let engine = Engine::builder()
        .register_type(direct_player_type())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(SourceId::new(24), "fn main() { return 1; }")
        .expect("initial generation");
    let update = engine
        .compile_hot_reload_update_with_id(
            &initial,
            SourceId::new(25),
            "extern state player: Player; fn main() { return 1; }",
        )
        .expect("state addition compiles");
    let initial_id = initial.id;
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime initializes");
    runtime
        .stage_extern_state("main::player", WrongHostType)
        .expect("unresolved future binding can stage");

    let report = runtime
        .apply_hot_update(update)
        .expect("reload reports rejection");

    assert!(!report.accepted);
    assert_eq!(report.errors[0].code, "reload.state.extern_binding_invalid");
    assert_eq!(
        runtime.hot_reload_version().expect("active version").id,
        initial_id
    );
}

#[test]
fn runtime_state_decl_persists_vm_owned_value_and_rust_updates() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
struct ServerState {
    level: i64,
    name: String,
}

state state: ServerState = ServerState { level: 0, name: "" };

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
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

    let state = runtime
        .call("make_state", CallArgs::new(), CallOptions::unbounded())
        .expect("factory should run");
    runtime
        .set_state("main::state", state)
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
                .state("main::state")
                .expect("script global should materialize")
                .expect("script global should exist"),
            "level",
        ),
        Some(&OwnedValue::Scalar(vela_common::ScalarValue::I64(10)))
    );

    runtime
        .update_state("main::state", |value| {
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
fn runtime_state_nested_record_program_links() {
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

state state: ServerState = ServerState {
    level: 0,
    name: "",
    total_gold: 0,
    stats: ServerStats { handled_ticks: 0 },
};

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
        .link_test_program(&program)
        .expect("nested script global program should link");
}

#[test]
fn shared_runtime_image_keeps_vm_states_isolated() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
struct ServerState {
    level: i64,
    name: String,
}

state state: ServerState = ServerState { level: 0, name: "" };

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
    let shared_image = RuntimeImage::new_compiled(engine, program).into_shared();
    let mut first =
        SharedRuntime::from_shared_image(shared_image.clone()).expect("runtime should initialize");
    let mut second =
        SharedRuntime::from_shared_image(shared_image).expect("runtime should initialize");

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
        .set_state("main::state", first_state)
        .expect("first script global should insert");
    second
        .set_state("main::state", second_state)
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
fn runtime_set_state_rejects_type_contract_mismatch() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
state amount: i64 = 0;

fn read_amount() {
    return amount;
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

    let error = runtime
        .set_state("main::amount", OwnedValue::String("wrong".to_owned()))
        .expect_err("typed state update should reject mismatched value");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeContractViolation {
            expected: "i64".to_owned(),
            actual: "String".to_owned(),
            debug_name: "main::amount".to_owned(),
        }
    );
    assert_eq!(
        runtime.state("main::amount"),
        Ok(Some(OwnedValue::from(0_i64))),
        "rejected value must not replace initialized state"
    );
}

#[test]
fn runtime_set_state_validates_linked_recursive_contract_before_replacement() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
state values: Array<i64> = [1, 2];
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

    runtime
        .set_state("main::values", OwnedValue::array([3_i64, 4_i64]))
        .expect("matching recursive contract should replace state");
    let malformed = OwnedValue::array([
        OwnedValue::from(5_i64),
        OwnedValue::String("wrong".to_owned()),
    ]);
    let error = runtime
        .set_state("main::values", malformed)
        .expect_err("nested mismatch should reject replacement");

    assert!(matches!(
        error.kind(),
        VmErrorKind::TypeContractViolation { expected, .. } if expected == "Array<i64>"
    ));
    assert_eq!(
        runtime.state("main::values"),
        Ok(Some(OwnedValue::array([3_i64, 4_i64]))),
        "rejected recursive value must not replace the current state"
    );
    assert_eq!(
        runtime
            .set_state("main::missing", OwnedValue::from(1_i64))
            .expect_err("missing linked descriptor must not bypass validation")
            .kind(),
        VmErrorKind::MissingVmState {
            name: "main::missing".to_owned(),
        }
    );
}

#[test]
fn runtime_set_state_accepts_and_rejects_recursive_contract_matrix() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
struct Player {
    level: i64,
}

enum Status {
    Ready,
    Waiting { code: i64 },
}

state array: Array<i64> = [1];
state map: Map<String, i64> = { "score": 1 };
state tuple: (i64, String) = (1, "ready");
state option: Option<i64> = Option::Some(1);
state result: Result<i64, String> = Result::Ok(1);
state player: Player = Player { level: 1 };
state status: Status = Status::Ready {};

fn player_level() {
    return player.level;
}

fn status_code() {
    return match status {
        Status::Ready {} => 1,
        Status::Waiting { code } => code,
    };
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

    let cases = [
        (
            "main::array",
            OwnedValue::array([2_i64]),
            OwnedValue::array([OwnedValue::String("wrong".to_owned())]),
        ),
        (
            "main::map",
            OwnedValue::map([("score", 2_i64)]),
            OwnedValue::map([("score", OwnedValue::String("wrong".to_owned()))]),
        ),
        (
            "main::tuple",
            OwnedValue::tuple([OwnedValue::from(2_i64), OwnedValue::from("ready")]),
            OwnedValue::tuple([OwnedValue::from(2_i64), OwnedValue::from(3_i64)]),
        ),
        (
            "main::option",
            OwnedValue::enum_variant("Option", "Some", [("0", OwnedValue::from(2_i64))]),
            OwnedValue::enum_variant("Option", "Some", [("0", OwnedValue::from("wrong"))]),
        ),
        (
            "main::result",
            OwnedValue::enum_variant("Result", "Ok", [("0", OwnedValue::from(2_i64))]),
            OwnedValue::enum_variant("Result", "Ok", [("0", OwnedValue::from("wrong"))]),
        ),
        (
            "main::player",
            OwnedValue::record("Player", [("level", 2_i64)]),
            OwnedValue::record("Player", [("level", "wrong")]),
        ),
        (
            "main::status",
            OwnedValue::enum_variant("Status", "Waiting", [("code", 2_i64)]),
            OwnedValue::enum_variant("Status", "Missing", Vec::<(&str, OwnedValue)>::new()),
        ),
    ];

    for (name, valid, invalid) in cases {
        runtime
            .set_state(name, valid.clone())
            .unwrap_or_else(|error| panic!("{name} should accept valid value: {error}"));
        runtime
            .set_state(name, invalid)
            .expect_err("malformed recursive value should fail");
        assert_eq!(runtime.state(name), Ok(Some(valid)));
    }

    runtime
        .set_state(
            "main::status",
            OwnedValue::enum_variant("Status", "Waiting", [("code", "wrong")]),
        )
        .expect_err("malformed enum payload should fail before replacement");

    runtime
        .update_state("main::player", |_| {})
        .expect("no-op record update should retain linked identity");
    runtime
        .update_state("main::status", |_| {})
        .expect("no-op enum update should retain linked identity");
    let level = runtime
        .call("player_level", CallArgs::new(), CallOptions::unbounded())
        .expect("canonical record should pass linked guards and field access");
    let status = runtime
        .call("status_code", CallArgs::new(), CallOptions::unbounded())
        .expect("canonical enum should retain pattern-match identity");
    assert_eq!(runtime.value_to_owned(&level), Ok(OwnedValue::from(2_i64)));
    assert_eq!(runtime.value_to_owned(&status), Ok(OwnedValue::from(2_i64)));
}

#[test]
fn runtime_update_state_rejects_type_contract_mismatch_without_replacing_value() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
struct ServerState {
    level: i64,
}

state state: ServerState = ServerState { level: 0 };
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    runtime
        .set_state(
            "main::state",
            OwnedValue::record("ServerState", [("level", OwnedValue::i64(3))]),
        )
        .expect("matching global should insert");

    let error = runtime
        .update_state("main::state", |value| {
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
                .state("main::state")
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
fn runtime_set_state_accepts_serde_struct_with_single_api() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
struct SerdeServerState {
    level: i64,
    name: String,
}

state state: SerdeServerState = SerdeServerState { level: 0, name: "" };

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
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let state = SerdeServerState {
        level: 5,
        name: "serde".to_owned(),
    };

    runtime
        .set_state("main::state", &state)
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
        .state_as("main::state")
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
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

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
fn runtime_from_value_accepts_string_keyed_script_maps() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
fn make_scores() {
    return [
        MapEntry { key: "xp", value: 20 },
        MapEntry { key: "gold", value: 10 },
    ].iter().collect_map();
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

    let scores = runtime
        .call("make_scores", CallArgs::new(), CallOptions::unbounded())
        .expect("scores should be returned as runtime value");

    assert_eq!(
        runtime.from_value::<BTreeMap<String, i64>>(&scores),
        Ok(BTreeMap::from([
            ("gold".to_owned(), 10_i64),
            ("xp".to_owned(), 20_i64),
        ]))
    );
    assert_eq!(
        runtime
            .value_to_owned(&scores)
            .expect("runtime value can materialize with string keys"),
        OwnedValue::map([("gold", 10_i64), ("xp", 20_i64)])
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
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
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
fn runtime_set_state_accepts_runtime_managed_value_with_single_api() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
struct ServerState {
    level: i64,
    name: String,
}

state state: ServerState = ServerState { level: 0, name: "" };

fn make_state() {
    return ServerState { level: 11, name: "runtime" };
}

fn read_level() {
    return state.level;
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

    let state = runtime
        .call("make_state", CallArgs::new(), CallOptions::unbounded())
        .expect("factory should return runtime-managed value");
    runtime
        .set_state("main::state", state)
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
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

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
fn retained_runtime_value_survives_state_collection() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
struct Reward {
    gold: i64,
    label: String,
}

state scratch: Reward = Reward { gold: 0, label: "" };

fn make_reward(gold, label) {
    return Reward { gold: gold, label: label };
}

fn reward_score(reward: Reward) {
    return reward.gold;
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

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
        .set_state("main::scratch", scratch)
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
    let mut runtime_a = Runtime::new(engine.clone(), program_a).expect("runtime should initialize");
    let mut runtime_b = Runtime::new(engine, program_b).expect("runtime should initialize");

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
