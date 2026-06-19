use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

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
            "handlers/dispatch.rs",
            "typed request and notification param decoding at the JSON-RPC boundary",
        ),
        (
            "lib.rs",
            "legacy tests plus diagnostic and work-done-progress extension payloads",
        ),
        (
            "lifecycle.rs",
            "legacy lifecycle compatibility tests compiled only in test builds",
        ),
        (
            "lsp/to_proto.rs",
            "diagnostic, workspace-symbol, and completion-resolve extension payloads",
        ),
        ("main_loop.rs", "inline typed main-loop tests"),
        (
            "queries.rs",
            "legacy query compatibility module compiled only in test builds",
        ),
        ("profile.rs", "profile JSONL events"),
        ("rpc.rs", "JSON-RPC wire serialization boundary"),
        (
            "stdio.rs",
            "legacy stdio compatibility module compiled only in test builds",
        ),
        ("task.rs", "inline task scheduler tests"),
        ("tests.rs", "legacy JSON fixture tests"),
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
        if relative.starts_with("tests/") {
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
