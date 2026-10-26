mod support;

use rstest::rstest;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use support::temp::TempDir;
use support::ui::{self, Exit};

fn run_cli(cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    support::process::run(
        Command::new(env!("CARGO_BIN_EXE_vela_cli"))
            .current_dir(cwd)
            .env_clear()
            .envs(env.iter().copied())
            .args(args),
    )
}

fn script_fixture(name: &str, source: &str) -> TempDir {
    let root = TempDir::new().expect("acquire isolated directory");
    fs::write(root.path().join(format!("{name}.vela")), source).expect("write script");
    root
}

#[rstest]
#[case::script_main("basic", include_str!("fixtures/ui/basic.vela"), &["basic.vela"], &[("NO_COLOR", "1"), ("TERM", "dumb")], Exit::Success)]
#[case::standard_time_and_random_helpers("stdlib", include_str!("fixtures/ui/stdlib.vela"), &["stdlib.vela"], &[("NO_COLOR", "1"), ("TERM", "dumb")], Exit::Success)]
fn cli_runs_script(
    #[case] name: &str,
    #[case] source: &str,
    #[case] args: &[&str],
    #[case] env: &[(&str, &str)],
    #[case] exit: Exit,
) {
    let fixture = script_fixture(name, source);
    ui::assert_golden(&run_cli(fixture.path(), args, env), name, exit);
}

#[rstest]
#[case::removed_async_flag(&["--async", "async_explicit.vela"], &[("NO_COLOR", "1"), ("TERM", "dumb")], Exit::UsageFailure)]
fn cli_rejects_removed_async_flag(
    #[case] args: &[&str],
    #[case] env: &[(&str, &str)],
    #[case] exit: Exit,
) {
    let fixture = script_fixture(
        "async_explicit",
        include_str!("fixtures/ui/async_explicit.vela"),
    );
    ui::assert_golden(
        &run_cli(fixture.path(), args, env),
        "removed_async_flag",
        exit,
    );
}

#[rstest]
#[case::removed_async_source("async_default", include_str!("fixtures/ui/async_default.vela"), &["async_default.vela"], &[("NO_COLOR", "1"), ("TERM", "dumb")], Exit::ScriptFailure, &["error[", "async"], &["rerun with `--async`"])]
#[case::compile_diagnostic("invalid", include_str!("fixtures/ui/invalid.vela"), &["invalid.vela"], &[("NO_COLOR", "1"), ("TERM", "dumb")], Exit::ScriptFailure, &["error[hir::unresolved_name]: unresolved name `missing_value`", "invalid.vela:3:12", "return missing_value;"], &[])]
#[case::runtime_diagnostic("runtime_error", include_str!("fixtures/ui/runtime_error.vela"), &["runtime_error.vela"], &[("NO_COLOR", "1"), ("TERM", "dumb")], Exit::ScriptFailure, &["error[vm::division_by_zero]: division by zero", "runtime_error.vela:3:12", "return 10 / 0;", "runtime_error.vela:7:12", "return helper();", "while executing `helper`"], &[])]
fn cli_reports_script_diagnostics(
    #[case] name: &str,
    #[case] source: &str,
    #[case] args: &[&str],
    #[case] env: &[(&str, &str)],
    #[case] exit: Exit,
    #[case] expected: &[&str],
    #[case] forbidden: &[&str],
) {
    let fixture = script_fixture(name, source);
    let output = run_cli(fixture.path(), args, env);
    ui::assert_golden(&output, name, exit);
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 diagnostics");
    for expected in expected {
        assert!(stderr.contains(expected), "missing {expected}: {stderr}");
    }
    for forbidden in forbidden {
        assert!(
            !stderr.contains(forbidden),
            "obsolete {forbidden}: {stderr}"
        );
    }
}

#[test]
fn cli_renders_clap_help() {
    let cwd = TempDir::new().expect("cwd");
    let output = run_cli(cwd.path(), &["--help"], &[("NO_COLOR", "1")]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("Run a Vela script file"));
    assert!(
        stdout.contains("Usage: vela_cli [OPTIONS] <SCRIPT>")
            || stdout.contains("Usage: vela_cli.exe [OPTIONS] <SCRIPT>")
    );
    assert!(!stdout.contains("--async"));
}

#[rstest]
#[case::invalid_source("fn main() { return missing_value; }")]
#[case::would_write("fn main() { fs::write_string(\"executed.txt\", \"bad\"); return 0; }")]
fn cli_print_schema_does_not_compile_or_execute(#[case] source: &str) {
    let fixture = script_fixture("schema", source);
    let output = run_cli(fixture.path(), &["--print-schema", "schema.vela"], &[]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let json = String::from_utf8(output.stdout).expect("schema JSON");
    vela_language_service::SchemaArtifact::from_json(&json).expect("supported schema format");
    assert!(json.contains("\"formatVersion\": 4"));
    assert!(json.contains("fs::write_string"));
    assert!(!fixture.path().join("executed.txt").exists());
}

#[test]
fn cli_print_schema_still_requires_positional_script() {
    let cwd = TempDir::new().expect("cwd");
    let output = run_cli(cwd.path(), &["--print-schema"], &[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .expect("stderr")
            .contains("<SCRIPT>")
    );
}

#[test]
fn cli_reports_missing_source() {
    let cwd = TempDir::new().expect("cwd");
    let output = run_cli(cwd.path(), &["missing.vela"], &[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(stderr.contains("missing.vela"), "{stderr}");
    assert!(stderr.starts_with("error:"), "{stderr}");
    assert!(!cwd.path().join("missing.vela").exists());
}

#[test]
fn cli_relative_script_uses_script_parent_as_fs_root() {
    let cwd = TempDir::new().expect("cwd");
    fs::create_dir(cwd.path().join("scripts")).expect("script parent");
    fs::write(cwd.path().join("input.txt"), "wrong cwd").expect("cwd decoy");
    fs::write(cwd.path().join("outside.txt"), "unchanged sentinel").expect("outside sentinel");
    fs::write(cwd.path().join("scripts/input.txt"), "script parent input").expect("input");
    fs::write(
        cwd.path().join("scripts/io.vela"),
        r#"
fn main() {
    let input = result::unwrap_or(fs::read_to_string("input.txt"), "missing");
    fs::write_string("output.txt", input);
    let denied = result::is_err(fs::write_string("../outside.txt", "changed"));
    let hidden = result::is_err(fs::read_to_string("../outside.txt"));
    if !denied || !hidden { return -1; }
    return input.len();
}
"#,
    )
    .expect("script");
    let output = run_cli(cwd.path(), &["scripts/io.vela"], &[]);
    ui::compare(&output, Exit::Success, b"Scalar(I64(19))\n", b"").expect("script parent IO");
    assert_eq!(
        fs::read(cwd.path().join("scripts/output.txt")).expect("written"),
        b"script parent input"
    );
    assert!(!cwd.path().join("output.txt").exists());
    assert_eq!(
        fs::read(cwd.path().join("outside.txt")).expect("sentinel"),
        b"unchanged sentinel"
    );
}
