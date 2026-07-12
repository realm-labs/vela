use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const MAX_RUST_FILE_LINES: usize = 1_200;

#[test]
fn serde_json_usage_stays_at_protocol_boundaries() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let allowed_files = BTreeMap::from([
        (
            "architecture_tests.rs",
            "this guard describes the serde_json allowlist",
        ),
        (
            "capabilities.rs",
            "initialize response payload compatibility at the JSON-RPC boundary",
        ),
        ("completion.rs", "completion resolve data extraction"),
        ("config.rs", "editor configuration settings payloads"),
        (
            "global_state.rs",
            "JSON-RPC response boundary, initialize settings, completion resolve data, and tests",
        ),
        (
            "global_state/diagnostics.rs",
            "typed diagnostics and work-done progress protocol projection",
        ),
        (
            "global_state/responses.rs",
            "typed JSON-RPC response projection",
        ),
        (
            "handlers/dispatch.rs",
            "typed request and notification param decoding at the JSON-RPC boundary",
        ),
        (
            "lsp/to_proto.rs",
            "diagnostic, workspace-symbol, and completion-resolve extension payloads",
        ),
        ("main_loop.rs", "inline typed main-loop tests"),
        ("profile.rs", "profile JSONL events"),
        ("rpc.rs", "JSON-RPC wire serialization boundary"),
        ("task.rs", "inline task scheduler tests"),
        (
            "tests.rs",
            "typed protocol-boundary fixture decoding and final message-shape assertions",
        ),
        ("tracing.rs", "trace JSONL events"),
        (
            "transport.rs",
            "typed protocol metadata extraction and typed harness fixtures",
        ),
        (
            "transport/config_tests.rs",
            "typed configuration harness tests",
        ),
        (
            "watching.rs",
            "client/registerCapability registerOptions extension payload",
        ),
    ]);

    let mut violations = Vec::new();
    for path in rust_files(&source_root) {
        let relative = path
            .strip_prefix(&source_root)
            .expect("source file should be under source root");
        let relative = relative.to_string_lossy().replace('\\', "/");
        if relative.starts_with("tests/")
            || relative.contains("/tests/")
            || relative.ends_with("/tests.rs")
        {
            continue;
        }

        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        if !source.contains("serde_json") {
            continue;
        }
        if allowed_files.contains_key(relative.as_str()) {
            continue;
        }

        violations.push(relative);
    }

    assert!(
        violations.is_empty(),
        "serde_json usage must stay at typed protocol boundaries, extension payloads, completion resolve data, configuration settings, schema artifact JSON, profiling/tracing JSONL, or tests; unexpected files: {violations:?}"
    );
}

#[test]
fn single_owner_architecture_cannot_regress() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let removed_owner = ["Lsp", "Server"].concat();
    let delegated_owner = ["self", ".server"].concat();
    let legacy_state = ["legacy", "_server"].concat();
    let forbidden_prefix = "sync_";
    let forbidden_suffix = "legacy";
    let legacy_sync = [
        forbidden_prefix,
        "workspace_analysis_from_",
        forbidden_suffix,
    ]
    .concat();
    let mut violations = Vec::new();

    for path in rust_files(&source_root) {
        let relative = relative_path(&source_root, &path);
        if relative == "architecture_tests.rs" {
            continue;
        }
        let source = read_source(&path);
        for forbidden in [
            &removed_owner,
            &delegated_owner,
            &legacy_state,
            &legacy_sync,
        ] {
            if source.contains(forbidden) {
                violations.push(format!("{relative}: forbidden `{forbidden}`"));
            }
        }

        if relative.starts_with("tests/")
            && (source.contains("fn dispatch_request")
                || source.contains("fn dispatch_notification")
                || source.contains("match method.as_str()"))
        {
            violations.push(format!("{relative}: duplicate test dispatcher"));
        }
    }

    assert!(
        violations.is_empty(),
        "GlobalState must remain the sole LSP coordinator and tests must use production dispatch: {violations:?}"
    );
}

#[test]
fn active_lsp_rust_files_stay_within_size_policy() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let violations = rust_files(&source_root)
        .into_iter()
        .filter_map(|path| {
            let line_count = read_source(&path).lines().count();
            (line_count > MAX_RUST_FILE_LINES)
                .then(|| format!("{}: {line_count} lines", relative_path(&source_root, &path)))
        })
        .collect::<Vec<_>>();

    assert!(
        violations.is_empty(),
        "active LSP Rust files must not exceed {MAX_RUST_FILE_LINES} lines: {violations:?}"
    );
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .expect("source file should be under source root")
        .to_string_lossy()
        .replace('\\', "/")
}

fn read_source(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

fn rust_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rust_files(root, &mut files);
    files.sort();
    files
}

fn collect_rust_files(path: &Path, files: &mut Vec<PathBuf>) {
    if path.is_file() {
        if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path.to_owned());
        }
        return;
    }

    let entries = fs::read_dir(path)
        .unwrap_or_else(|error| panic!("failed to read directory {}: {error}", path.display()));
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("failed to read directory entry: {error}"));
        collect_rust_files(&entry.path(), files);
    }
}
