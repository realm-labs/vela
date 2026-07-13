use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("vela_package lives under workspace/crates")
        .to_owned()
}

#[test]
fn package_crate_stays_dependency_light() {
    let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("vela_package manifest");
    for forbidden in [
        "vela_engine",
        "vela_hir",
        "vela_bytecode",
        "vela_vm",
        "vela_hot_reload",
        "vela_language_service",
        "vela_lsp_server",
    ] {
        assert!(
            !manifest.contains(forbidden),
            "vela_package must not depend on {forbidden}"
        );
    }
}

#[test]
fn package_identity_and_manifest_legacy_paths_do_not_return() {
    let root = workspace_root().join("crates");
    for path in rust_files(&root) {
        let text = fs::read_to_string(&path).expect("Rust source");
        let normalized = path.to_string_lossy().replace('\\', "/");
        if normalized.ends_with("/vela_package/tests/architecture.rs") {
            continue;
        }
        if !normalized.contains("/vela_package/") {
            assert!(
                !text.contains("toml_edit::Document")
                    && !text.contains("fn parse_manifest(")
                    && !text.contains("[workspace].roots"),
                "manifest parsing must remain in vela_package: {normalized}"
            );
        }
        assert!(
            !text.contains("const SCRIPT_PACKAGE")
                && !text.contains("DefPath::function(\"script\"")
                && !text.contains("BTreeMap<ModulePath, ModuleId>")
                && !text.contains("module_by_path"),
            "legacy package identity returned in {normalized}"
        );
    }
}

#[test]
fn provider_runtime_and_reload_keep_artifact_boundaries() {
    let root = workspace_root();
    let provider_runtime =
        fs::read_to_string(root.join("crates/vela_engine/src/runtime/provider.rs"))
            .expect("provider runtime");
    assert!(provider_runtime.contains("method: MethodId"));
    assert!(!provider_runtime.contains("method_name"));

    let reload_manifest = fs::read_to_string(root.join("crates/vela_hot_reload/Cargo.toml"))
        .expect("reload manifest");
    let production_dependencies = reload_manifest
        .split("[dev-dependencies]")
        .next()
        .expect("reload production dependency section");
    assert!(!production_dependencies.contains("vela_package"));
    for path in rust_files(&root.join("crates/vela_hot_reload/src")) {
        let text = fs::read_to_string(&path).expect("hot reload source");
        assert!(!text.contains("load_package_graph"));
        assert!(!text.contains("compile_package_program"));
    }
}

#[test]
fn active_workspace_files_follow_the_reviewed_size_policy() {
    const MAX_LINES: usize = 1_200;

    let root = workspace_root();
    let exceptions = fs::read_to_string(root.join("docs/architecture/file-size-exceptions.md"))
        .expect("file-size exception ledger");
    let mut violations = Vec::new();
    for source_root in [root.join("crates"), root.join("examples")] {
        for path in rust_files(&source_root) {
            let text = fs::read_to_string(&path).expect("Rust source");
            let line_count = text.lines().count();
            if line_count <= MAX_LINES {
                continue;
            }
            let relative = path
                .strip_prefix(&source_root)
                .expect("source belongs to scanned root")
                .to_string_lossy()
                .replace('\\', "/");
            let ledger_path = if source_root.ends_with("crates") {
                relative
            } else {
                format!("examples/{relative}")
            };
            if !exceptions.contains(&format!("`{ledger_path}`")) {
                violations.push(format!("{ledger_path}: {line_count} lines"));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "active Rust files above {MAX_LINES} lines require a reviewed exception: {violations:?}"
    );
}

fn rust_files(root: &Path) -> Vec<PathBuf> {
    let mut pending = vec![root.to_owned()];
    let mut files = Vec::new();
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path).expect("architecture directory") {
            let entry = entry.expect("architecture entry");
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }
    files
}
