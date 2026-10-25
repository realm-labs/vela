use super::*;

use vela_bytecode::UnlinkedProgram;
use vela_vm::error::VmResult;

fn run_linked_program(
    engine: &Engine,
    program: &UnlinkedProgram,
    entry: &str,
    args: &[OwnedValue],
) -> VmResult<OwnedValue> {
    let linked = engine
        .link_test_program(program)
        .expect("source reload directory test program should link");
    engine
        .into_vm_for_program(program)
        .run_linked_program(&linked, entry, args)
}

#[test]
fn engine_compile_dir_loads_vela_modules_deterministically() {
    let root = unique_test_dir("compile_dir");
    let game_dir = root.join("game");
    std::fs::create_dir_all(&game_dir).expect("create module dir");
    std::fs::write(
        game_dir.join("main.vela"),
        r#"
use game::reward::grant

fn main() {
    return grant() + game::config::BONUS;
}
"#,
    )
    .expect("write main module");
    std::fs::write(
        game_dir.join("reward.vela"),
        r#"
pub fn grant() {
    return 4;
}
"#,
    )
    .expect("write reward module");
    std::fs::write(
        game_dir.join("config.vela"),
        r#"
pub const BONUS: i64 = 6;
"#,
    )
    .expect("write config module");
    std::fs::write(root.join("ignored.txt"), "fn main() { return 99; }")
        .expect("write ignored file");
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");

    let program = engine.compile_dir(&root).expect("compile dir");

    assert_eq!(
        run_linked_program(&engine, &program, "game::main::main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(10)))
    );
    assert!(program.function("ignored.main").is_none());
}

#[test]
fn engine_compile_hot_reload_dir_loads_module_updates() {
    let root = unique_test_dir("hot_reload_dir");
    let game_dir = root.join("game");
    std::fs::create_dir_all(&game_dir).expect("create module dir");
    std::fs::write(
        game_dir.join("main.vela"),
        r#"
use game::reward::grant

fn main() {
    return grant() + 1;
}
"#,
    )
    .expect("write main module");
    std::fs::write(
        game_dir.join("reward.vela"),
        r#"
pub fn grant() {
    return 4;
}
"#,
    )
    .expect("write reward module");
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");

    assert_eq!(
        runtime.call_raw("game::main::main", &[], CallOptions::unbounded()),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(5)))
    );

    std::fs::write(
        game_dir.join("reward.vela"),
        r#"
pub fn grant() {
    return 7;
}
"#,
    )
    .expect("write updated reward module");
    let current = runtime
        .hot_reload_version()
        .expect("current hot reload version");
    let update = runtime
        .engine()
        .compile_hot_reload_update_dir(&current, &root)
        .expect("compatible hot reload dir update");
    let report = runtime
        .apply_reload_update_for_test(update)
        .expect("apply update");

    assert!(report.accepted);
    assert_eq!(
        report.changed_functions,
        vec!["game::reward::grant".to_owned()]
    );
    assert_eq!(report.changed_modules, vec!["game::reward".to_owned()]);
    assert_eq!(
        report.impacted_modules,
        vec!["game::main".to_owned(), "game::reward".to_owned()]
    );
    assert_eq!(
        runtime.call_raw("game::main::main", &[], CallOptions::unbounded()),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(8)))
    );
}

#[test]
fn runtime_stages_hot_reload_dir_until_check_reload_safe_point() {
    let root = unique_test_dir("runtime_stage_hot_reload_dir");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");

    assert_eq!(
        runtime.call_raw("game::main::main", &[], CallOptions::unbounded()),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    write_reward_module(&reward_file, 6);
    runtime
        .stage_reload(crate::runtime::ReloadSource::directory(&root))
        .expect("dir update should stage");
    assert!(
        runtime
            .has_pending_reload()
            .expect("dir update should be pending")
    );
    assert_eq!(
        runtime.call_raw("game::main::main", &[], CallOptions::unbounded()),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    let report = runtime
        .activate_reload()
        .expect("check reload at safe point")
        .expect("staged dir report");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["game::reward::grant"]);
    assert_eq!(report.changed_modules, vec!["game::reward"]);
    assert_eq!(
        report.impacted_modules,
        vec!["game::main".to_owned(), "game::reward".to_owned()]
    );
    assert!(
        !runtime
            .has_pending_reload()
            .expect("safe point should consume dir update")
    );
    assert_eq!(
        runtime.call_raw("game::main::main", &[], CallOptions::unbounded()),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(6)))
    );
}

#[test]
fn runtime_stages_dir_hot_reload_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_dir_rejection");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .hot_reload_policy(HotReloadPolicy::locked_down())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");

    write_reward_module_with_helper(&reward_file, 6);
    runtime
        .stage_reload(crate::runtime::ReloadSource::directory(&root))
        .expect("hot reload rejection should be staged");
    assert_eq!(
        runtime.call_raw("game::main::main", &[], CallOptions::unbounded()),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    let report = runtime
        .activate_reload()
        .expect("check reload at safe point")
        .expect("staged rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert!(matches!(
        report.errors[0].error.kind,
        HotReloadErrorKind::NewFunctionDenied { ref function }
            if function == "game::reward::helper"
    ));
    assert_eq!(
        runtime.call_raw("game::main::main", &[], CallOptions::unbounded()),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn runtime_stages_dir_private_helper_addition_until_safe_point() {
    private_helper_addition_report(
        "runtime_stage_dir_private_helper_addition",
        ScriptFunctionReloadWorkflow::Directory,
    );
}

#[test]
fn runtime_stages_dir_public_function_addition_until_safe_point() {
    public_function_addition_report(
        "runtime_stage_dir_public_function_addition",
        ScriptFunctionReloadWorkflow::Directory,
    );
}

#[test]
fn runtime_stages_dir_return_abi_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_dir_return_abi");
    let reward_file = write_typed_reward_modules(&root, "return grant();", "i64", "2");
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");

    assert_eq!(
        runtime.call_raw("game::main::main", &[], CallOptions::unbounded()),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    write_typed_reward_module(&reward_file, "f64", "6.0");
    runtime
        .stage_reload(crate::runtime::ReloadSource::directory(&root))
        .expect("dir return ABI rejection should be staged");
    assert_eq!(
        runtime.call_raw("game::main::main", &[], CallOptions::unbounded()),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    let report = runtime
        .activate_reload()
        .expect("check reload at safe point")
        .expect("staged dir return ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.return_abi_changed");
    assert_function_return_repair_hint(&report);
    assert_rendered_repair_hint(
        &report,
        "preserve the previous return type hint or restart with an explicit migration",
    );
    let HotReloadErrorKind::ChangedFunctionReturnAbi {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed function return ABI");
    };
    assert_eq!(function, "game::reward::grant");
    assert_eq!(old.as_deref(), Some("i64"));
    assert_eq!(new.as_deref(), Some("f64"));
    assert!(source_span.is_some());
    assert_eq!(
        runtime.call_raw("game::main::main", &[], CallOptions::unbounded()),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}
