use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

fn run_cli(script: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_vela_cli"))
        .arg(script)
        .output()
        .expect("run vela_cli")
}

fn run_cli_args(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_vela_cli"))
        .args(args)
        .output()
        .expect("run vela_cli")
}

struct ScriptFixture {
    root: PathBuf,
    script: PathBuf,
}

impl ScriptFixture {
    fn script(&self) -> &Path {
        &self.script
    }
}

impl Drop for ScriptFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn unique_test_dir(name: &str) -> PathBuf {
    static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(0);

    let mut path = std::env::temp_dir();
    let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_nanos();
    path.push(format!(
        "vela_cli_{name}_{}_{}_{}",
        std::process::id(),
        nanos,
        sequence
    ));
    path
}

fn write_script(name: &str, source: &str) -> ScriptFixture {
    let root = unique_test_dir(name);
    fs::create_dir_all(&root).expect("create temp dir");
    let script = root.join(format!("{name}.vela"));
    fs::write(&script, source).expect("write script");
    ScriptFixture { root, script }
}

#[test]
fn cli_runs_script_main() {
    let fixture = write_script(
        "basic",
        r#"
fn main() {
    return 2 + 3;
}
"#,
    );

    let output = run_cli(
        fixture
            .script()
            .to_str()
            .expect("script path should be utf8"),
    );

    assert!(
        output.status.success(),
        "cli script failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout should be utf8"),
        "Scalar(I64(5))\n"
    );
}

#[test]
fn cli_runs_standard_time_and_random_helpers() {
    let fixture = write_script(
        "stdlib",
        r#"
fn main() {
    return time::elapsed_since(1699999990) + math::random(1, 6);
}
"#,
    );

    let output = run_cli(
        fixture
            .script()
            .to_str()
            .expect("script path should be utf8"),
    );

    assert!(
        output.status.success(),
        "cli stdlib script failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout should be utf8"),
        "Scalar(I64(13))\n"
    );
}

#[test]
fn cli_reports_rendered_compile_diagnostics() {
    let fixture = write_script(
        "invalid",
        r#"
fn main() {
    return missing_value;
}
"#,
    );

    let output = run_cli(
        fixture
            .script()
            .to_str()
            .expect("script path should be utf8"),
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("error[hir::unresolved_name]: unresolved name `missing_value`"));
    assert!(stderr.contains("invalid.vela:3:12"));
    assert!(stderr.contains("return missing_value;"));
}

#[test]
fn cli_reports_rendered_runtime_diagnostics() {
    let fixture = write_script(
        "runtime_error",
        r#"
fn helper() {
    return 10 / 0;
}

fn main() {
    return helper();
}
"#,
    );

    let output = run_cli(
        fixture
            .script()
            .to_str()
            .expect("script path should be utf8"),
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("error[vm::division_by_zero]: division by zero"));
    assert!(stderr.contains("runtime_error.vela:3:12"));
    assert!(stderr.contains("return 10 / 0;"));
    assert!(stderr.contains("runtime_error.vela:7:12"));
    assert!(stderr.contains("return helper();"));
    assert!(stderr.contains("while executing `helper`"));
}

#[test]
fn cli_renders_clap_help() {
    let output = run_cli_args(&["--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("Run a Vela script file"));
    assert!(
        stdout.contains("Usage: vela_cli <SCRIPT>")
            || stdout.contains("Usage: vela_cli.exe <SCRIPT>")
    );
}
