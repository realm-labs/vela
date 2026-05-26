use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn run_demo(script: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_vela_cli"))
        .arg(script_path(script))
        .output()
        .expect("run vela_cli demo script");

    assert!(
        output.status.success(),
        "demo script failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    String::from_utf8(output.stdout).expect("demo stdout should be utf8")
}

fn run_hot_reload_demo(initial: &str, updated: &str) -> String {
    let initial = script_path(initial);
    let updated = script_path(updated);
    let output = run_hot_reload_paths(&initial, &updated);

    assert!(
        output.status.success(),
        "hot reload demo failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    String::from_utf8(output.stdout).expect("hot reload stdout should be utf8")
}

fn run_hot_reload_paths(initial: &Path, updated: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_vela_cli"))
        .arg("--hot-reload")
        .arg(initial)
        .arg(updated)
        .output()
        .expect("run vela_cli hot reload demo")
}

fn script_path(script: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/game_server_demo/scripts")
        .join(script)
}

fn unique_test_dir(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "vela_cli_{name}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos()
    ));
    path
}

#[test]
fn level_up_demo_runs_through_cli() {
    assert_eq!(
        run_demo("level_up.lang"),
        "result=Int(10) level=Int(10) patches=1\n"
    );
}

#[test]
fn context_event_demo_runs_through_cli() {
    assert_eq!(
        run_demo("context_event.lang"),
        "result=Int(1700000042) level=Int(9) ctx_now=Int(1700000000) \
         ctx_tick=Int(42) emits=1 logs=1 patches=2\n"
    );
}

#[test]
fn monster_kill_reward_demo_runs_through_cli() {
    assert_eq!(
        run_demo("monster_kill_reward.lang"),
        "result=Int(2) level=Int(2) exp=Int(0) quest_count=Int(2) \
         quest_done=Bool(false) inventory_gold=Int(3) reward_calls=0 emits=2 patches=6\n"
    );
}

#[test]
fn quest_progress_demo_runs_through_cli() {
    assert_eq!(
        run_demo("quest_progress.lang"),
        "result=Int(3) level=Int(1) exp=Int(90) quest_count=Int(3) \
         quest_done=Bool(true) inventory_gold=Int(0) reward_calls=0 emits=1 patches=3\n"
    );
}

#[test]
fn reflect_debug_demo_runs_through_cli() {
    assert_eq!(
        run_demo("reflect_debug.lang"),
        "result=Int(19) level=Int(12) ctx_now=Int(1700000000) \
         ctx_tick=Int(42) emits=1 patches=2\n"
    );
}

#[test]
fn hot_reload_function_swap_demo_runs_through_cli() {
    assert_eq!(
        run_hot_reload_demo(
            "hot_reload_function_swap_v1.lang",
            "hot_reload_function_swap_v2.lang",
        ),
        "hot reload accepted: v0 -> v1\n\
         changed functions: kill_exp, main\n\
         abi=checked old_version=0 new_version=1 old_before=Int(20) old_after=Int(20) \
         new_after=Int(30)\n"
    );
}

#[test]
fn hot_reload_demo_reports_abi_rejection() {
    let root = unique_test_dir("hot_reload_reject");
    fs::create_dir_all(&root).expect("create temp dir");
    let initial = root.join("initial.lang");
    let updated = root.join("updated.lang");
    fs::write(
        &initial,
        r#"
fn helper() {
    return 20;
}

fn main() {
    return helper();
}
"#,
    )
    .expect("write initial script");
    fs::write(
        &updated,
        r#"
fn main() {
    return 30;
}
"#,
    )
    .expect("write updated script");

    let output = run_hot_reload_paths(&initial, &updated);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("hot reload rejected: v0 unchanged"));
    assert!(stderr.contains("[reload.function.removed] helper: function `helper` was removed"));
    assert!(stderr.contains("repair: keep the function declaration"));
    fs::remove_dir_all(root).expect("clean temp dir");
}

#[test]
fn invalid_demo_script_reports_rendered_source_diagnostic() {
    let root = unique_test_dir("invalid_script");
    fs::create_dir_all(&root).expect("create temp dir");
    let script = root.join("invalid.lang");
    fs::write(
        &script,
        r#"
fn main() {
    return missing_value;
}
"#,
    )
    .expect("write invalid script");

    let output = Command::new(env!("CARGO_BIN_EXE_vela_cli"))
        .arg(&script)
        .output()
        .expect("run vela_cli invalid script");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("error[hir::unresolved_name]: unresolved name `missing_value`"));
    assert!(stderr.contains("invalid.lang:3:12"));
    assert!(stderr.contains("return missing_value;"));
    fs::remove_dir_all(root).expect("clean temp dir");
}
