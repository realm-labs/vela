use rstest::rstest;
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

#[rstest]
#[case::script_main(
    "basic",
    r#"
fn main() {
    return 2 + 3;
}
"#,
    "Scalar(I64(5))\n"
)]
#[case::standard_time_and_random_helpers(
    "stdlib",
    r#"
fn main() {
    return time::elapsed_since(1699999990) + math::random(1, 6);
}
"#,
    "Scalar(I64(13))\n"
)]
fn cli_runs_script(#[case] name: &str, #[case] source: &str, #[case] expected: &str) {
    let fixture = write_script(name, source);

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
        expected
    );
}

#[test]
fn cli_rejects_removed_async_flag() {
    let fixture = write_script(
        "async_explicit",
        r#"
async fn answer() {
    return 42;
}

async fn main() {
    return answer().await;
}
"#,
    );
    let script = fixture
        .script()
        .to_str()
        .expect("script path should be utf8");

    let output = run_cli_args(&["--async", script]);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("unexpected argument '--async'"));
}

#[rstest]
#[case::removed_async_source(
    "async_default",
    r#"
async fn main() {
    return 42;
}
"#,
    &["error[", "async"],
    &["rerun with `--async`"],
)]
#[case::compile_diagnostic(
    "invalid",
    r#"
fn main() {
    return missing_value;
}
"#,
    &[
        "error[hir::unresolved_name]: unresolved name `missing_value`",
        "invalid.vela:3:12",
        "return missing_value;",
    ],
    &[],
)]
#[case::runtime_diagnostic(
    "runtime_error",
    r#"
fn helper() {
    return 10 / 0;
}

fn main() {
    return helper();
}
"#,
    &[
        "error[vm::division_by_zero]: division by zero",
        "runtime_error.vela:3:12",
        "return 10 / 0;",
        "runtime_error.vela:7:12",
        "return helper();",
        "while executing `helper`",
    ],
    &[],
)]
fn cli_reports_script_diagnostics(
    #[case] name: &str,
    #[case] source: &str,
    #[case] expected: &[&str],
    #[case] forbidden: &[&str],
) {
    let fixture = write_script(name, source);

    let output = run_cli(
        fixture
            .script()
            .to_str()
            .expect("script path should be utf8"),
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    for expected in expected {
        assert!(
            stderr.contains(expected),
            "missing `{expected}` in {stderr}"
        );
    }
    for forbidden in forbidden {
        assert!(
            !stderr.contains(forbidden),
            "unexpected `{forbidden}` in {stderr}"
        );
    }
}

#[test]
fn cli_renders_clap_help() {
    let output = run_cli_args(&["--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("Run a Vela script file"));
    assert!(
        stdout.contains("Usage: vela_cli [OPTIONS] <SCRIPT>")
            || stdout.contains("Usage: vela_cli.exe [OPTIONS] <SCRIPT>")
    );
    assert!(!stdout.contains("--async"));
}
