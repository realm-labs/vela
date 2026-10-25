use std::sync::Arc;

use vela_def::FunctionId;
use vela_engine::engine::Engine;
use vela_engine::native::NativeFunctionDesc;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_hot_reload::version::ProgramVersion;
use vela_reflect::permissions::ReflectPolicy;
use vela_vm::owned_value::OwnedValue;

fn call_raw(
    runtime: &mut Runtime,
    entry: &str,
    args: &[OwnedValue],
    options: CallOptions,
) -> vela_vm::error::VmResult<OwnedValue> {
    let args = CallArgs::from_positional(args.iter().cloned());
    let value = runtime.call(entry, args, options)?;
    runtime.value_to_owned(&value)
}

#[test]
fn runtime_hot_reload_update_waits_for_explicit_reload_safe_point() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(
            "fn main() -> i64 { let total = 0; for value in 0..2 { total += value + 1 - 1; } return total; }",
        )
        .expect("initial hot reload compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    let initial = runtime
        .hot_reload_version()
        .expect("runtime should expose active hot reload version");
    let initial_version = initial.id;
    let initial_artifact = Arc::clone(initial.linked_artifact());
    assert!(has_selected_scalar_loop(&initial));

    assert_eq!(
        call_raw(&mut runtime, "main", &[], CallOptions::unbounded()),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    runtime
        .stage_reload(
            "fn main() -> i64 { let total = 0; for value in 0..2 { total += value + 1 - 1; } return total + 1; }",
        )
        .expect("compatible update should stage");

    let staged_active = runtime
        .hot_reload_version()
        .expect("runtime should keep active version until apply");
    assert_eq!(staged_active.id, initial_version);
    assert!(Arc::ptr_eq(
        staged_active.linked_artifact(),
        &initial_artifact
    ));
    assert_eq!(
        call_raw(&mut runtime, "main", &[], CallOptions::unbounded()),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .activate_reload()
        .expect("runtime should activate update at safe point")
        .expect("staged update should produce a report");

    assert!(report.accepted);
    let activated = runtime
        .hot_reload_version()
        .expect("accepted reload should publish a new version");
    assert!(has_selected_scalar_loop(&activated));
    assert_ne!(
        activated.executable_generation_id(),
        initial_artifact.generation()
    );
    assert!(!Arc::ptr_eq(activated.linked_artifact(), &initial_artifact));
    assert!(initial_artifact.program().functions().any(|(_, code)| {
        code.scalar_blocks
            .iter()
            .any(|plan| plan.range_loop.is_some())
    }));
    assert_eq!(
        call_raw(&mut runtime, "main", &[], CallOptions::unbounded()),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn active_sync_call_keeps_old_generation_until_completion_safe_point() {
    let staging = Arc::new(std::sync::Mutex::new(
        None::<vela_engine::runtime::HotReloadStagingHandle>,
    ));
    let pending = Arc::new(std::sync::Mutex::new(None));
    let callback_staging = Arc::clone(&staging);
    let callback_pending = Arc::clone(&pending);
    let engine = Engine::builder()
        .register_context_host_native_fn(
            NativeFunctionDesc::new("test::stage", FunctionId::new(0xA5D0)),
            move |_, context| {
                if let Some(update) = callback_pending.lock().expect("update lock").take() {
                    let staging = callback_staging.lock().expect("staging lock");
                    let staging = staging.as_ref().expect("staging installed");
                    assert!(staging.stage_reload_update(update).is_none());
                    assert!(staging.has_pending_update());
                }
                let _ = context.call("nested", CallArgs::new())?;
                Ok(OwnedValue::Unit)
            },
        )
        .build()
        .expect("engine");
    let initial = engine.compile_hot_reload_initial(
        "state value: i64 = 1; fn nested() { value += 1; } fn helper() { return 10; } fn main() { test::stage(); return helper() + value; }",
    ).expect("initial generation");
    let update = engine.compile_hot_reload_update(&initial,
        "state extra: i64 = 100; state value: i64 = 999; fn nested() { value += 10; } fn helper() { return 20; } fn main() { test::stage(); return helper() + value + extra; }",
    ).expect("compatible update");
    let old_generation = initial.executable_generation_id();
    let new_generation = update.linked_artifact().generation();
    let mut runtime = Runtime::from_hot_reload_version(engine, initial).expect("runtime");
    *staging.lock().expect("staging lock") = runtime.hot_reload_staging_handle();
    *pending.lock().expect("update lock") = Some(update);

    let old = runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::new(10_000, 1 << 20, 32),
        )
        .expect("active old call");
    assert_eq!(runtime.value_to_owned(&old), Ok(OwnedValue::i64(12)));
    assert_eq!(
        runtime
            .hot_reload_version()
            .expect("owner")
            .executable_generation_id(),
        old_generation
    );
    assert!(
        runtime
            .activate_reload()
            .expect("safe point")
            .expect("staged update")
            .accepted
    );
    assert_eq!(
        runtime
            .hot_reload_version()
            .expect("owner")
            .executable_generation_id(),
        new_generation
    );
    let new = runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::new(10_000, 1 << 20, 32),
        )
        .expect("new call");
    assert_eq!(runtime.value_to_owned(&new), Ok(OwnedValue::i64(132)));
}

#[test]
fn retained_closure_pins_old_generation_across_handle_layout_reload() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(
            r#"
fn helper(value: i64) -> i64 { let total = 0; for item in 0..2 { total += item + 1 - 1; } return value + 1 + total; }
fn make() { return |value: i64| helper(value) + 10; }
fn invoke(callback, value: i64) -> i64 { return callback(value); }
"#,
        )
        .expect("initial closure generation should compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    let old_artifact = runtime
        .hot_reload_version()
        .map(|version| Arc::clone(version.linked_artifact()))
        .expect("old closure version");
    assert!(old_artifact.program().functions().any(|(_, code)| {
        code.scalar_blocks
            .iter()
            .any(|plan| plan.range_loop.is_some())
    }));
    let old_closure = runtime
        .call("make", CallArgs::new(), CallOptions::unbounded())
        .expect("old closure should be retained");

    runtime
        .stage_reload(
            r#"
fn alpha_private(value: i64) -> i64 { return value * 1000; }
fn helper(value: i64) -> i64 { let total = 0; for item in 0..4 { total += item + 1 - 1; } return value + 100 + total; }
fn make() { return |value: i64| helper(value) + 20; }
fn invoke(callback, value: i64) -> i64 { return callback(value); }
"#,
        )
        .expect("closure reload should stage");
    runtime
        .activate_reload()
        .expect("closure reload should activate")
        .expect("staged closure reload should produce a report");
    let new_version = runtime
        .hot_reload_version()
        .expect("new closure version should be active");
    assert!(has_selected_scalar_loop(&new_version));
    assert_ne!(
        new_version.executable_generation_id(),
        old_artifact.generation()
    );

    let mut old_args = CallArgs::from_values([old_closure.clone()]);
    old_args.push(5_i64);
    let old_result = runtime
        .call("invoke", old_args, CallOptions::unbounded())
        .expect("old closure should execute against its creation generation");
    let new_closure = runtime
        .call("make", CallArgs::new(), CallOptions::unbounded())
        .expect("new closure should use the active generation");
    let mut new_args = CallArgs::from_values([new_closure]);
    new_args.push(5_i64);
    let new_result = runtime
        .call("invoke", new_args, CallOptions::unbounded())
        .expect("new closure should execute active code");

    assert_eq!(runtime.value_to_owned(&old_result), Ok(OwnedValue::i64(17)));
    assert_eq!(
        runtime.value_to_owned(&new_result),
        Ok(OwnedValue::i64(131))
    );

    let mut budgeted_args = CallArgs::from_values([old_closure]);
    budgeted_args.push(5_i64);
    let error = runtime
        .call(
            "invoke",
            budgeted_args,
            CallOptions::new(1, usize::MAX, usize::MAX),
        )
        .expect_err("nested old-generation call needs two semantic call units");
    assert!(matches!(
        error.kind(),
        vela_vm::error::VmErrorKind::BudgetExceeded {
            budget: vela_vm::budget::ExecutionBudgetKind::ExecutionUnits,
            limit: 1
        }
    ));
}

fn has_selected_scalar_loop(version: &ProgramVersion) -> bool {
    version.linked_program().functions().any(|(_, code)| {
        code.scalar_blocks
            .iter()
            .any(|plan| plan.range_loop.is_some())
    })
}

#[test]
fn retained_old_closure_reads_state_removed_from_active_generation() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(
            r#"
state retired: i64 = 5;
fn make() { return || retired; }
fn invoke(callback) -> i64 { return callback(); }
"#,
        )
        .expect("initial state generation should compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    let old_closure = runtime
        .call("make", CallArgs::new(), CallOptions::unbounded())
        .expect("old closure should be retained");
    runtime
        .stage_reload(
            r#"
fn make() { return || 0; }
fn invoke(callback) -> i64 { return callback(); }
"#,
        )
        .expect("removed-state update should stage");

    let report = runtime
        .activate_reload()
        .expect("reload activates")
        .expect("staged update should produce a report");

    assert_eq!(report.removed_states, ["main::retired"]);
    assert_eq!(runtime.state("main::retired"), Ok(None));
    let old_result = runtime
        .call(
            "invoke",
            CallArgs::from_values([old_closure]),
            CallOptions::unbounded(),
        )
        .expect("old closure should resolve its generation state slot");
    assert_eq!(runtime.value_to_owned(&old_result), Ok(OwnedValue::i64(5)));
}

#[test]
fn retained_old_closure_keeps_native_dispatch_and_rejected_reload_owner() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(
            r#"
fn make() { return |value: String| value.starts_with("old"); }
fn invoke(callback, value: String) -> bool { return callback(value); }
fn scalar_sum(limit: i64) -> i64 { let total = 0; for item in 0..limit { total += item + 1 - 1; } return total; }
"#,
        )
        .expect("initial native closure generation should compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    let old_generation = runtime
        .hot_reload_version()
        .expect("hot reload version")
        .executable_generation_id();
    let old_artifact = runtime
        .hot_reload_version()
        .map(|version| Arc::clone(version.linked_artifact()))
        .expect("old generation artifact");
    assert!(old_artifact.program().functions().any(|(_, code)| {
        code.scalar_blocks
            .iter()
            .any(|plan| plan.range_loop.is_some())
    }));
    let old_closure = runtime
        .call("make", CallArgs::new(), CallOptions::unbounded())
        .expect("old closure should be retained");

    runtime
        .stage_reload(
            r#"
fn make(extra) { return |value: String| value.ends_with("new"); }
fn invoke(callback, value: String) -> bool { return callback(value); }
fn scalar_sum(limit: i64) -> i64 { let total = 0; for item in 0..limit { total += item + 1 - 1; } return total; }
"#,
        )
        .expect("ABI rejection should stage for a safe-point report");
    let rejected = runtime
        .activate_reload()
        .expect("rejected reload should activate as a report")
        .expect("staged rejection should produce a report");
    assert!(!rejected.accepted);
    assert_eq!(
        runtime
            .hot_reload_version()
            .expect("rejected reload keeps version")
            .executable_generation_id(),
        old_generation
    );
    assert!(Arc::ptr_eq(
        runtime
            .hot_reload_version()
            .expect("rejected reload keeps artifact")
            .linked_artifact(),
        &old_artifact
    ));

    let mut args = CallArgs::from_values([old_closure]);
    args.push("old-generation".to_owned());
    let result = runtime
        .call("invoke", args, CallOptions::unbounded())
        .expect("old closure should retain native dispatch after rejection");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
}

#[test]
fn hot_reload_runtime_reflection_tracks_script_metadata_after_reload() {
    let engine = Engine::builder()
        .reflection_policy(ReflectPolicy::all())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(
            r#"
enum QuestProgress {
    Active { count }
}

fn main() {
    let quest_type = reflect::type_info("QuestProgress");
    let quest = QuestProgress::Active { count: 1 };

    if reflect::kind(quest_type) == "script_enum"
        && reflect::has_function("main")
        && reflect::has_variant(quest_type, "Active")
        && reflect::variant_is(quest, "Active") {
        return 1;
    }

    return 0;
}
"#,
        )
        .expect("initial hot reload compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");

    assert_eq!(
        call_raw(&mut runtime, "main", &[], CallOptions::unbounded()),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    runtime
        .stage_reload(
            r#"
enum QuestProgress {
    Active { count }
    Finished { count }
}

fn main() {
    let quest_type = reflect::type_info("QuestProgress");
    let quest = QuestProgress::Finished { count: 2 };

    if reflect::kind(quest_type) == "script_enum"
        && reflect::has_function("main")
        && reflect::has_variant(quest_type, "Finished")
        && reflect::variant_is(quest, "Finished") {
        return 2;
    }

    return 0;
}
"#,
        )
        .expect("compatible update should stage");

    assert_eq!(
        call_raw(&mut runtime, "main", &[], CallOptions::unbounded()),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .activate_reload()
        .expect("runtime should activate update at safe point")
        .expect("staged update should produce a report");

    assert!(report.accepted);
    assert_eq!(
        call_raw(&mut runtime, "main", &[], CallOptions::unbounded()),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn hot_reload_runtime_preserves_script_method_dispatch_tables() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(
            r#"
trait BonusSource {
    fn bonus(self, amount) -> i64;
}

struct Player {
    level: i64
}

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount;
    }
}

fn main() {
    return Player { level: 7 }.bonus(5);
}
"#,
        )
        .expect("initial hot reload compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");

    assert_eq!(
        call_raw(&mut runtime, "main", &[], CallOptions::unbounded()),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );

    runtime
        .stage_reload(
            r#"
trait BonusSource {
    fn bonus(self, amount) -> i64;
}

struct Player {
    level: i64
}

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount * 2;
    }
}

fn main() {
    return Player { level: 7 }.bonus(5);
}
"#,
        )
        .expect("compatible update should stage");

    assert_eq!(
        call_raw(&mut runtime, "main", &[], CallOptions::unbounded()),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );

    let report = runtime
        .activate_reload()
        .expect("runtime should activate update at safe point")
        .expect("staged update should produce a report");

    assert!(report.accepted);
    assert_eq!(
        call_raw(&mut runtime, "main", &[], CallOptions::unbounded()),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(17)))
    );
}
