use std::fs;
use std::path::PathBuf;

use vela_engine::engine::Engine;
use vela_engine::runtime::{CallOptions, Runtime};
use vela_host::access::HostAccess;
use vela_host::mock::MockStateAdapter;
use vela_reflect::permissions::ReflectPolicy;
use vela_vm::owned_value::OwnedValue;

fn unique_test_dir(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "vela_engine_{name}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos()
    ));
    path
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
    engine
        .link_program(&program)
        .expect("reflection script should link");
    let mut runtime = Runtime::new(engine, program);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx,),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn runtime_reflection_includes_compiled_script_modules_and_exports() {
    let root = unique_test_dir("script_module_reflection");
    let game_dir = root.join("game");
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
        && reflect::docs(function) == "Grant reward."
        && reflect::attr(function, "event") == "reward"
        && reflect::returns(function) == "bool"
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
    let program = engine.compile_dir(&root).expect("compile modules");
    engine
        .link_program(&program)
        .expect("reflection module script should link");
    let mut runtime = Runtime::new(engine, program);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    fs::remove_dir_all(root).expect("clean temp dir");
}
