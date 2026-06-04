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

fn run_demo_allow_random(script: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_vela_cli"))
        .arg("--allow-random")
        .arg(script_path(script))
        .output()
        .expect("run vela_cli allowed random demo");

    assert!(
        output.status.success(),
        "allowed random demo failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    String::from_utf8(output.stdout).expect("stdout should be utf8")
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
        run_demo("level_up.vela"),
        "result=Int(10) level=Int(10) patches=1\n"
    );
}

#[test]
fn context_event_demo_runs_through_cli() {
    assert_eq!(
        run_demo("context_event.vela"),
        "result=Int(1700000042) level=Int(9) ctx_now=Int(1700000000) \
         ctx_tick=Int(42) emits=1 logs=1 patches=2\n"
    );
}

#[test]
fn context_clock_demo_runs_through_cli() {
    assert_eq!(
        run_demo("context_clock.vela"),
        "result=Int(52) level=Int(9) patches=0\n"
    );
}

#[test]
fn gameplay_helpers_demo_runs_through_cli() {
    assert_eq!(
        run_demo("gameplay_helpers.vela"),
        "result=Int(9) level=Int(9) patches=0\n"
    );
}

#[test]
fn random_allowed_demo_runs_through_cli() {
    assert_eq!(
        run_demo_allow_random("random_allowed.vela"),
        "result=Int(310) level=Int(9) patches=0\n"
    );
}

#[test]
fn random_reflect_allowed_demo_runs_through_cli() {
    assert_eq!(
        run_demo_allow_random("random_reflect_allowed.vela"),
        "result=Int(310) level=Int(9) patches=0\n"
    );
}

#[test]
fn reward_preview_demo_runs_through_cli() {
    assert_eq!(
        run_demo("reward_preview.vela"),
        "result=Int(23) level=Int(1) patches=0\n"
    );
}

#[test]
fn random_permission_demo_reports_permission_denial() {
    let output = Command::new(env!("CARGO_BIN_EXE_vela_cli"))
        .arg(script_path("random_permission_denied.vela"))
        .output()
        .expect("run vela_cli random permission demo");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.contains(
        "error[vm::permission_denied]: native `math::random` requires permission `std.random`"
    ));
    assert!(stderr.contains("native `math::random` requires permission `std.random`"));
    assert!(stderr.contains("random_permission_denied.vela:2:12"));
    assert!(stderr.contains("return math::random(1, 6);"));
}

#[test]
fn host_read_only_demo_reports_field_not_writable() {
    let output = Command::new(env!("CARGO_BIN_EXE_vela_cli"))
        .arg(script_path("host_read_only_denied.vela"))
        .output()
        .expect("run vela_cli read-only host field demo");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.contains(
        "error[analysis::field_not_writable]: field `Player.id` is read-only for script writes"
    ));
    assert!(stderr.contains("host_read_only_denied.vela:2:5"));
    assert!(stderr.contains("player.id = 8;"));
}

#[test]
fn stale_host_ref_demo_reports_generation_mismatch() {
    let output = Command::new(env!("CARGO_BIN_EXE_vela_cli"))
        .arg("--stale-player")
        .arg(script_path("stale_host_ref.vela"))
        .output()
        .expect("run vela_cli stale host ref demo");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains(
            "error[vm::host_error]: host error: StaleGeneration { expected: 2, actual: 3 }"
        )
    );
    assert!(stderr.contains("stale_host_ref.vela:2:12"));
    assert!(stderr.contains("return player.level;"));
}

#[test]
fn host_permission_demo_reports_denied_host_read() {
    let output = Command::new(env!("CARGO_BIN_EXE_vela_cli"))
        .arg("--deny-player-level-read")
        .arg(script_path("host_permission_denied.vela"))
        .output()
        .expect("run vela_cli host permission denial demo");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("error[vm::host_error]: host error: PermissionDenied"));
    assert!(stderr.contains("action: \"read\""));
    assert!(stderr.contains("host_permission_denied.vela:2:12"));
    assert!(stderr.contains("return player.level;"));
}

#[test]
fn host_write_permission_demo_reports_denied_apply() {
    let output = Command::new(env!("CARGO_BIN_EXE_vela_cli"))
        .arg("--deny-player-level-write")
        .arg(script_path("host_write_permission_denied.vela"))
        .output()
        .expect("run vela_cli host write permission denial demo");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("error[vm::host_error]: host error: PermissionDenied"));
    assert!(stderr.contains("action: \"write\""));
    assert!(stderr.contains("host_write_permission_denied.vela:2:5"));
    assert!(stderr.contains("player.level = 12;"));
}

#[test]
fn host_call_permission_demo_reports_denied_apply() {
    let output = Command::new(env!("CARGO_BIN_EXE_vela_cli"))
        .arg("--deny-ctx-emit-call")
        .arg(script_path("host_call_permission_denied.vela"))
        .output()
        .expect("run vela_cli host call permission denial demo");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("error[vm::host_error]: host error: PermissionDenied"));
    assert!(stderr.contains("action: \"call\""));
    assert!(stderr.contains("host_call_permission_denied.vela:2:5"));
    assert!(stderr.contains("ctx.emit(\"demo.denied\", 12);"));
}

#[test]
fn bad_schema_demo_reports_duplicate_field() {
    let output = Command::new(env!("CARGO_BIN_EXE_vela_cli"))
        .arg(script_path("bad_schema_duplicate_field.vela"))
        .output()
        .expect("run vela_cli bad schema demo");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("error[hir::duplicate_field]: duplicate field `item_id`"));
    assert!(stderr.contains("bad_schema_duplicate_field.vela:3:5"));
    assert!(stderr.contains("item_id: int,"));
    assert!(stderr.contains("previous field is here"));
    assert!(stderr.contains("duplicate field is here"));
}

#[test]
fn generic_type_hint_demo_reports_unsupported_generics() {
    let output = Command::new(env!("CARGO_BIN_EXE_vela_cli"))
        .arg(script_path("generic_type_hint_denied.vela"))
        .output()
        .expect("run vela_cli generic type hint demo");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains(
            "error[syntax::generic_type_hint]: script type hints do not support generics"
        )
    );
    assert!(stderr.contains("generic_type_hint_denied.vela:1:22"));
    assert!(stderr.contains("fn main(values: Array<int>) {"));
    assert!(stderr.contains("remove generic type arguments"));
}

#[test]
fn monster_kill_reward_demo_runs_through_cli() {
    assert_eq!(
        run_demo("monster_kill_reward.vela"),
        "result=Int(2) level=Int(2) exp=Int(0) quest_count=Int(3) \
         quest_done=Bool(true) inventory_gold=Int(3) reward_calls=1 emits=3 patches=10\n"
    );
}

#[test]
fn quest_progress_demo_runs_through_cli() {
    assert_eq!(
        run_demo("quest_progress.vela"),
        "result=Int(3) level=Int(1) exp=Int(90) quest_count=Int(3) \
         quest_done=Bool(true) inventory_gold=Int(0) reward_calls=0 emits=1 patches=3\n"
    );
}

#[test]
fn reflect_debug_demo_runs_through_cli() {
    assert_eq!(
        run_demo("reflect_debug.vela"),
        "result=Int(22) level=Int(12) ctx_now=Int(1700000000) \
         ctx_tick=Int(42) emits=1 patches=2\n"
    );
}

#[test]
fn reflect_schema_mutation_demo_reports_invalid_target() {
    let output = Command::new(env!("CARGO_BIN_EXE_vela_cli"))
        .arg(script_path("reflect_schema_mutation_denied.vela"))
        .output()
        .expect("run vela_cli schema mutation denial demo");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("error[reflect::invalid_target]: invalid reflection target"));
    assert!(stderr.contains("reflect_schema_mutation_denied.vela:3:12"));
    assert!(stderr.contains("return reflect::set(player_type, \"name\", \"Monster\");"));
}

#[test]
fn hot_reload_function_swap_demo_runs_through_cli() {
    assert_eq!(
        run_hot_reload_demo(
            "hot_reload_function_swap_v1.vela",
            "hot_reload_function_swap_v2.vela",
        ),
        "hot reload accepted: v0 -> v1\n\
         changed functions: kill_exp\n\
         safe_point=tick_boundary abi=checked old_version=0 new_version=1 old_before=Int(20) \
         old_after=Int(20) new_after=Int(30)\n"
    );
}

#[test]
fn hot_reload_demo_reports_abi_rejection() {
    let output = run_hot_reload_paths(
        &script_path("hot_reload_function_swap_v1.vela"),
        &script_path("hot_reload_function_swap_invalid.vela"),
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("hot reload rejected: v0 unchanged"));
    assert!(stderr.contains("[reload.function.removed] kill_exp: function `kill_exp` was removed"));
    assert!(stderr.contains("repair: keep the function declaration"));
}

#[test]
fn hot_reload_demo_renders_source_spans_for_abi_rejections() {
    let root = unique_test_dir("hot_reload_abi_span");
    fs::create_dir_all(&root).expect("create temp dir");
    let updated = root.join("hot_reload_return_abi.vela");
    fs::write(
        &updated,
        r#"
fn kill_exp() -> float {
    return 30;
}

fn main() {
    return kill_exp();
}
"#,
    )
    .expect("write ABI-invalid hot reload script");

    let output = run_hot_reload_paths(&script_path("hot_reload_function_swap_v1.vela"), &updated);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("hot reload rejected: v0 unchanged"));
    assert!(stderr.contains(
        "[reload.function.return_abi_changed] kill_exp: function `kill_exp` changed return ABI"
    ));
    assert!(stderr.contains("error[reload.function.return_abi_changed]"));
    assert!(stderr.contains("hot_reload_return_abi.vela:2:1"));
    assert!(stderr.contains("fn kill_exp() -> float {"));
    assert!(!stderr.contains("ChangedFunctionReturnAbi"));
    fs::remove_dir_all(root).expect("clean temp dir");
}

#[test]
fn invalid_demo_script_reports_rendered_source_diagnostic() {
    let root = unique_test_dir("invalid_script");
    fs::create_dir_all(&root).expect("create temp dir");
    let script = root.join("invalid.vela");
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
    assert!(stderr.contains("invalid.vela:3:12"));
    assert!(stderr.contains("return missing_value;"));
    fs::remove_dir_all(root).expect("clean temp dir");
}

#[test]
fn runtime_demo_error_reports_rendered_diagnostic() {
    let root = unique_test_dir("runtime_error");
    fs::create_dir_all(&root).expect("create temp dir");
    let script = root.join("runtime_error.vela");
    fs::write(
        &script,
        r#"
fn helper() {
    return 10 / 0;
}

fn main() {
    return helper();
}
"#,
    )
    .expect("write runtime error script");

    let output = Command::new(env!("CARGO_BIN_EXE_vela_cli"))
        .arg(&script)
        .output()
        .expect("run vela_cli runtime error script");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("error[vm::division_by_zero]: division by zero"));
    assert!(stderr.contains("runtime_error.vela:3:12"));
    assert!(stderr.contains("return 10 / 0;"));
    assert!(stderr.contains("runtime_error.vela:7:12"));
    assert!(stderr.contains("return helper();"));
    assert!(stderr.contains("while executing `helper`"));
    assert!(!stderr.contains("DivisionByZero"));
    fs::remove_dir_all(root).expect("clean temp dir");
}
