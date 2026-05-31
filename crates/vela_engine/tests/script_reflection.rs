use std::fs;
use std::path::PathBuf;

use vela_engine::{CallOptions, Engine, ReflectPolicy, Runtime, Value};
use vela_host::{MockStateAdapter, PatchTx};

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
    let root = unique_test_dir("script_reflection");
    fs::create_dir_all(&root).expect("create temp dir");
    let script = root.join("script_reflection.lang");
    fs::write(
        &script,
        r#"
enum QuestProgress {
    Active { count }
    Finished { count }
}

fn main() {
    let quest_type = reflect.type_info("QuestProgress");
    let main_function = reflect.function("main");
    let functions = reflect.functions();
    let quest = QuestProgress.Active { count: 2 };
    let variants = reflect.variants(quest);
    let active = reflect.variant_info(quest, "Active");

    if reflect.kind(quest_type) == "script_enum"
        && quest_type.variant_count == 2
        && reflect.name(main_function) == "main"
        && reflect.kind(main_function) == "function"
        && reflect.origin(main_function) == "script"
        && reflect.has_function("main")
        && functions.any(|function| reflect.name(function) == "main"
            && reflect.origin(function) == "script")
        && variants.len() == 2
        && reflect.has_variant(quest_type, "Active")
        && reflect.variant(quest) == "Active"
        && reflect.variant_is(quest, "Active")
        && active.owner == "QuestProgress"
        && active.fields[0].name == "count" {
        return active.fields.len();
    }

    return 0;
}
"#,
    )
    .expect("write script");

    let engine = Engine::builder()
        .reflection_policy(ReflectPolicy::all())
        .build()
        .expect("build engine");
    let program = engine.compile_file(&script).expect("compile script");
    let mut runtime = Runtime::new(engine, program);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx,),
        Ok(Value::Int(1))
    );

    fs::remove_dir_all(root).expect("clean temp dir");
}

#[test]
fn runtime_reflection_includes_compiled_script_modules_and_exports() {
    let root = unique_test_dir("script_module_reflection");
    let game_dir = root.join("game");
    fs::create_dir_all(&game_dir).expect("create module dir");
    fs::write(
        game_dir.join("reward.lang"),
        r#"
#[doc("Grant reward.")]
#[event("reward")]
pub fn grant(player, amount: int = 1) -> bool {
    return true;
}
"#,
    )
    .expect("write reward module");
    fs::write(
        game_dir.join("main.lang"),
        r#"
use game.reward.grant

fn main() {
    let module = reflect.module("game.reward");
    let function = reflect.function("game.reward.grant");
    let params = reflect.params(function);
    let exports = reflect.exports(module);
    let modules = reflect.modules();
    let functions = reflect.functions();

    if reflect.name(module) == "game.reward"
        && reflect.origin(module) == "script"
        && reflect.has_module("game.reward")
        && module.exports.len() == 1
        && exports.len() == 1
        && exports[0] == "game.reward.grant"
        && modules.any(|candidate| reflect.name(candidate) == "game.reward"
            && reflect.origin(candidate) == "script")
        && reflect.name(function) == "game.reward.grant"
        && reflect.origin(function) == "script"
        && reflect.docs(function) == "Grant reward."
        && reflect.attr(function, "event") == "reward"
        && reflect.returns(function) == "bool"
        && params.len() == 2
        && params[0].name == "player"
        && params[1].name == "amount"
        && params[1].type == "int"
        && params[1].defaulted
        && reflect.has_function("game.reward.grant")
        && functions.any(|candidate| reflect.name(candidate) == "game.reward.grant"
            && reflect.origin(candidate) == "script") {
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
    let mut runtime = Runtime::new(engine, program);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game.main.main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        ),
        Ok(Value::Int(1))
    );

    fs::remove_dir_all(root).expect("clean temp dir");
}
