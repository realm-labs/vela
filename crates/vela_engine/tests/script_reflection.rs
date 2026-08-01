use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use vela_engine::engine::Engine;
use vela_engine::permission::Capability;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_host::access::HostAccess;
use vela_host::mock::MockStateAdapter;
use vela_reflect::permissions::ReflectPolicy;
use vela_vm::owned_value::OwnedValue;

fn call_raw(
    runtime: &mut Runtime,
    entry: &str,
    args: &[OwnedValue],
    options: CallOptions,
    adapter: &mut MockStateAdapter,
    _access: &mut HostAccess,
) -> vela_vm::error::VmResult<OwnedValue> {
    let args = CallArgs::from_positional(args.iter().cloned()).with_fallback_adapter(adapter);
    let value = runtime.call(entry, args, options)?;
    runtime.value_to_owned(&value)
}

struct TestDir(PathBuf);

impl TestDir {
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn unique_test_dir(name: &str) -> TestDir {
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    let mut path = std::env::temp_dir();
    let sequence = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    path.push(format!(
        "vela_engine_{name}_{}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos(),
        sequence
    ));
    TestDir(path)
}

#[test]
fn runtime_reflection_includes_compiled_script_metadata() {
    let source = r#"
enum QuestProgress {
    Active { count }
    Finished { count }
}

fn main() {
    let quest_type = reflect::type_info("QuestProgress");
    let main_function = reflect::function("main");
    let quest = QuestProgress::Active { count: 2 };

    if reflect::kind(quest_type) == "script_enum"
        && reflect::name(main_function) == "main"
        && reflect::kind(main_function) == "function"
        && reflect::origin(main_function) == "script"
        && reflect::has_function("main")
        && reflect::has_variant(quest_type, "Active")
        && reflect::has_variant(quest_type, "Finished")
        && reflect::variant(quest) == "Active"
        && reflect::variant_is(quest, "Active") {
        return 1;
    }

    return 0;
}

"#;

    let engine = Engine::builder()
        .reflection_policy(ReflectPolicy::all())
        .build()
        .expect("build engine");
    let program = engine.compile_source(source).expect("compile script");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        call_raw(
            &mut runtime,
            "main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn runtime_reflection_reports_detached_targets_without_making_them_callable() {
    let source = r#"
async fn worker(value: Any) -> Any {
    let _started_at = time::now();
    return value;
}

fn schedule(value: Any) {
    task::spawn_scoped(worker(value));
}

fn inspect() {
    let target = reflect::function("worker");
    if !target.detached_target { return 10; }
    if target.detached_parameter_contracts[0] != "Any" { return 20; }
    if target.detached_parameter_modes[0] != "runtime_checked" { return 30; }
    if target.detached_result_contract.unwrap_or("") != "Any" { return 40; }
    if target.detached_result_mode.unwrap_or("") != "runtime_checked" { return 50; }
    if !target.detached_requires_service_generation { return 60; }
    if target.access.reflect_callable { return 70; }
    if !target.detached_effects.reads_time { return 80; }
    return 1;
}
"#;

    let engine = Engine::builder()
        .capability(Capability::TaskSpawn)
        .capability(Capability::Time)
        .with_time_clock(1_700_000_000, 42)
        .reflection_policy(ReflectPolicy::all())
        .build()
        .expect("build engine");
    let program = engine
        .compile_source(source)
        .expect("compile task metadata");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        call_raw(
            &mut runtime,
            "inspect",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        ),
        Ok(OwnedValue::i64(1))
    );
}

#[test]
fn runtime_reflection_includes_compiled_script_modules_and_exports() {
    let root = unique_test_dir("script_module_reflection");
    let game_dir = root.path().join("game");
    fs::create_dir_all(&game_dir).expect("create module dir");
    fs::write(
        game_dir.join("reward.vela"),
        r#"
#[doc("Grant reward.")]
#[event("reward")]
pub fn grant(player, amount: i64 = 1) -> bool {
    return true;
}
"#,
    )
    .expect("write reward module");
    fs::write(
        game_dir.join("main.vela"),
        r#"
use game::reward::grant

fn main() {
    let module = reflect::module("game::reward");
    let function = reflect::function("game::reward::grant");
    let exports = reflect::exports(module);

    if reflect::name(module) == "game::reward"
        && reflect::origin(module) == "script"
        && reflect::has_module("game::reward")
        && exports[0] == "game::reward::grant"
        && reflect::name(function) == "game::reward::grant"
        && reflect::origin(function) == "script"
        && reflect::docs(function).unwrap_or("") == "Grant reward."
        && reflect::attr(function, "event").unwrap_or("") == "reward"
        && reflect::returns(function).unwrap_or("") == "bool"
        && reflect::has_function("game::reward::grant") {
        return 1;
    }

    return 0;
}
"#,
    )
    .expect("write main module");

    let engine = Engine::builder()
        .reflection_policy(ReflectPolicy::all())
        .build()
        .expect("build engine");
    let program = engine.compile_dir(root.path()).expect("compile modules");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        call_raw(
            &mut runtime,
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}
