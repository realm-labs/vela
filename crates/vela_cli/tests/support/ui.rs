use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Output;

#[derive(Clone, Copy, Debug)]
pub enum Exit {
    Success,
    ScriptFailure,
    UsageFailure,
}

impl Exit {
    fn code(self) -> i32 {
        match self {
            Self::Success => 0,
            Self::ScriptFailure => 1,
            Self::UsageFailure => 2,
        }
    }
}

pub fn compare(output: &Output, exit: Exit, stdout: &[u8], stderr: &[u8]) -> Result<(), String> {
    if output.status.code() != Some(exit.code()) {
        return Err(format!("exit: expected {exit:?}, got {}", output.status));
    }
    if output.stdout != stdout {
        return Err(format!(
            "stdout mismatch: {:?}",
            String::from_utf8_lossy(&output.stdout)
        ));
    }
    if output.stderr != stderr {
        return Err(format!(
            "stderr mismatch: {:?}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

pub fn assert_golden(output: &Output, stem: &str, exit: Exit) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ui");
    let stdout = fs::read(root.join(format!("{stem}.stdout")))
        .expect("stdout golden, including empty streams");
    let mut stderr = fs::read(root.join(format!("{stem}.stderr")))
        .expect("stderr golden, including empty streams");
    // Clap derives the executable spelling on Windows; all other bytes are exact.
    if cfg!(windows) && stem == "removed_async_flag" {
        stderr = String::from_utf8(stderr)
            .expect("UTF-8 golden")
            .replace("Usage: vela_cli ", "Usage: vela_cli.exe ")
            .into_bytes();
    }
    compare(output, exit, &stdout, &stderr).expect("complete CLI UI golden");
}

fn inventory(root: &Path) -> Result<(), String> {
    let pairs = [
        ("basic", "basic"),
        ("stdlib", "stdlib"),
        ("invalid", "invalid"),
        ("runtime_error", "runtime_error"),
        ("async_default", "async_default"),
        ("async_explicit", "removed_async_flag"),
    ];
    let expected: BTreeSet<_> = pairs
        .into_iter()
        .flat_map(|(source, output)| {
            [
                format!("{source}.vela"),
                format!("{output}.stdout"),
                format!("{output}.stderr"),
            ]
        })
        .collect();
    let actual: BTreeSet<_> = fs::read_dir(root)
        .map_err(|e| e.to_string())?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    if actual != expected {
        return Err(format!(
            "UI inventory mismatch; missing: {:?}; orphan: {:?}",
            expected.difference(&actual),
            actual.difference(&expected)
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::support::{process, temp::TempDir};
    use rstest::rstest;
    use std::process::Command;

    #[test]
    fn ui_inventory_is_nonempty_and_registered() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ui");
        inventory(&root).expect("exact source/output pairs");
        let registration = include_str!("../run_scripts.rs");
        for entry in fs::read_dir(root).expect("inventory") {
            let path = entry.expect("entry").path();
            if path.extension().is_some_and(|ext| ext == "vela") {
                let include = format!(
                    "include_str!(\"fixtures/ui/{}\")",
                    path.file_name().expect("filename").to_str().expect("UTF-8")
                );
                assert_eq!(
                    registration.matches(&include).count(),
                    1,
                    "register {include} exactly once"
                );
            }
        }
    }

    #[rstest]
    #[case::missing_stdout("basic.stdout", false)]
    #[case::missing_stderr("runtime_error.stderr", false)]
    #[case::missing_source("invalid.vela", false)]
    #[case::orphan_output("orphan.stderr", true)]
    #[case::unregistered_source("orphan.vela", true)]
    fn inventory_rejects_missing_and_orphan_pairs(#[case] name: &str, #[case] add: bool) {
        let copy = TempDir::new().expect("copy");
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ui");
        for entry in fs::read_dir(root).expect("inventory") {
            let entry = entry.expect("entry");
            fs::copy(entry.path(), copy.path().join(entry.file_name())).expect("copy fixture");
        }
        inventory(copy.path()).expect("positive control");
        if add {
            fs::write(copy.path().join(name), b"").expect("orphan");
        } else {
            fs::remove_file(copy.path().join(name)).expect("remove pair");
        }
        assert!(inventory(copy.path()).is_err());
    }

    #[test]
    fn inventory_rejects_empty_selection() {
        let empty = TempDir::new().expect("empty");
        assert!(inventory(empty.path()).is_err());
    }

    #[test]
    fn comparator_rejects_wrong_status_output_and_extra_diagnostics() {
        let cwd = TempDir::new().expect("cwd");
        let output = process::run(
            Command::new(env!("CARGO_BIN_EXE_vela_cli"))
                .arg("--help")
                .current_dir(cwd.path())
                .env_clear(),
        );
        compare(&output, Exit::Success, &output.stdout, &output.stderr).expect("positive control");
        assert!(
            compare(&output, Exit::ScriptFailure, &output.stdout, &output.stderr)
                .expect_err("status")
                .starts_with("exit:")
        );
        assert!(compare(&output, Exit::UsageFailure, &output.stdout, &output.stderr).is_err());
        assert!(
            compare(&output, Exit::Success, b"wrong output", &output.stderr)
                .expect_err("stdout")
                .starts_with("stdout")
        );
        assert!(
            compare(
                &output,
                Exit::Success,
                &output.stdout,
                b"extra diagnostic\n"
            )
            .expect_err("stderr")
            .starts_with("stderr")
        );
    }
}
