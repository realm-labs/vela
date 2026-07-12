use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use vela_common::ScalarValue;
use vela_package::PackageId;
use vela_vm::owned_value::OwnedValue;

use super::*;
use crate::permission::ExecutionProfile;
use crate::runtime::{CallArgs, CallOptions, Runtime};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

#[test]
fn ordinary_package_imports_public_dependency_function() {
    let root = package_fixture("public_function");
    write_package(
        &root,
        "dev.vela.app",
        "app",
        "[dependencies]\nutil = { path = \"util\" }\n",
        "use util::api::answer\npub fn main() { return answer(); }\n",
    );
    write_package(
        &root.join("util"),
        "dev.vela.util",
        "util",
        "",
        "pub fn answer() { return 42; }\n",
    );

    let engine = Engine::builder().build().expect("engine");
    let snapshot = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("snapshot");
    let artifact = engine
        .compile_package(&snapshot, &package("dev.vela.app"))
        .expect("ordinary package compiles");

    assert!(
        artifact
            .image()
            .functions()
            .any(|(_, function)| function.name == "main::main")
    );
    remove_fixture(root);
}

#[test]
fn ordinary_package_imports_dependency_type_and_method() {
    let root = package_fixture("public_type");
    write_package(
        &root,
        "dev.vela.app",
        "app",
        "[dependencies]\nutil = { path = \"util\" }\n",
        "use util::api::Counter\npub fn main() { let value = Counter { value: 21 }; return value.doubled(); }\n",
    );
    write_package(
        &root.join("util"),
        "dev.vela.util",
        "util",
        "",
        "pub struct Counter { value: i64 }\nimpl Counter { fn doubled(self) { return self.value * 2; } }\n",
    );

    let engine = Engine::builder().build().expect("engine");
    let snapshot = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("snapshot");
    let artifact = engine
        .compile_package(&snapshot, &package("dev.vela.app"))
        .expect("dependency type and method compile");
    let mut runtime = Runtime::from_linked_artifact(engine, artifact);

    let output = runtime
        .call("main::main", CallArgs::new(), CallOptions::unbounded())
        .expect("package entry runs");
    assert_eq!(
        runtime.value_to_owned(&output),
        Ok(OwnedValue::Scalar(ScalarValue::I64(42)))
    );
    remove_fixture(root);
}

#[test]
fn ordinary_package_rejects_private_dependency_declaration() {
    let root = package_fixture("private_function");
    write_package(
        &root,
        "dev.vela.app",
        "app",
        "[dependencies]\nutil = { path = \"util\" }\n",
        "use util::api::answer\npub fn main() { return answer(); }\n",
    );
    write_package(
        &root.join("util"),
        "dev.vela.util",
        "util",
        "",
        "fn answer() { return 42; }\n",
    );

    let engine = Engine::builder().build().expect("engine");
    let error = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect_err("private cross-package import must fail");

    assert!(matches!(error.kind, EnginePackageErrorKind::Frontend(_)));
    remove_fixture(root);
}

#[test]
fn ordinary_package_includes_transitive_dependencies_but_not_their_aliases() {
    let root = package_fixture("transitive");
    write_package(
        &root,
        "dev.vela.app",
        "app",
        "[dependencies]\nmiddle = { path = \"middle\" }\n",
        "use middle::api::answer\npub fn main() { return answer(); }\n",
    );
    write_package(
        &root.join("middle"),
        "dev.vela.middle",
        "middle",
        "[dependencies]\nleaf = { path = \"../leaf\" }\n",
        "use leaf::api::value\npub fn answer() { return value(); }\n",
    );
    write_package(
        &root.join("leaf"),
        "dev.vela.leaf",
        "leaf",
        "",
        "pub fn value() { return 7; }\n",
    );

    let engine = Engine::builder().build().expect("engine");
    let snapshot = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("transitive snapshot");
    let artifact = engine
        .compile_package(&snapshot, &package("dev.vela.app"))
        .expect("transitive closure compiles");
    assert_eq!(
        artifact
            .package_metadata()
            .expect("package metadata")
            .packages()
            .len(),
        3
    );

    fs::write(
        root.join("src/main.vela"),
        "use leaf::api::value\npub fn main() { return value(); }\n",
    )
    .expect("replace app source");
    let error = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect_err("transitive alias is not visible to app");
    assert!(matches!(error.kind, EnginePackageErrorKind::Frontend(_)));
    remove_fixture(root);
}

#[test]
fn ordinary_package_compiles_and_runs_without_provider_catalog() {
    let root = package_fixture("runtime");
    write_package(
        &root,
        "dev.vela.app",
        "app",
        "",
        "pub fn main() { return 19; }\n",
    );
    let engine = Engine::builder().build().expect("engine");
    let snapshot = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("snapshot");
    let request = PackageCompileRequest::for_root(&snapshot, &package("dev.vela.app"));
    let artifact = engine
        .compile_packages(&snapshot, &request)
        .expect("artifact");
    let mut runtime = Runtime::from_linked_artifact(engine, artifact);

    let output = runtime
        .call("main::main", CallArgs::new(), CallOptions::unbounded())
        .expect("package entry runs");
    assert_eq!(
        runtime.value_to_owned(&output),
        Ok(OwnedValue::Scalar(ScalarValue::I64(19)))
    );
    remove_fixture(root);
}

#[test]
fn ordinary_package_artifact_has_empty_installed_provider_set() {
    let root = package_fixture("empty_providers");
    write_package(
        &root,
        "dev.vela.app",
        "app",
        "",
        "pub fn main() { return 1; }\n",
    );
    let engine = Engine::builder().build().expect("engine");
    let snapshot = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("snapshot");
    let artifact = engine
        .compile_package(&snapshot, &package("dev.vela.app"))
        .expect("artifact");
    let metadata = artifact.package_metadata().expect("package metadata");

    assert_eq!(metadata.request().roots(), &[package("dev.vela.app")]);
    assert!(metadata.installed_providers().is_empty());
    remove_fixture(root);
}

#[test]
fn ordinary_package_reload_rejects_a_different_root_request() {
    let root = package_fixture("reload_request");
    write_package(
        &root,
        "dev.vela.app",
        "app",
        "[dependencies]\nutil = { path = \"util\" }\n",
        "pub fn main() { return 1; }\n",
    );
    write_package(
        &root.join("util"),
        "dev.vela.util",
        "util",
        "",
        "pub fn value() { return 2; }\n",
    );
    let engine = Engine::builder().build().expect("engine");
    let snapshot = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("snapshot");
    let app_request = PackageCompileRequest::for_root(&snapshot, &package("dev.vela.app"));
    let initial = engine
        .compile_package_hot_reload_initial(&snapshot, &app_request)
        .expect("initial package version");
    let util_request = PackageCompileRequest::for_root(&snapshot, &package("dev.vela.util"));
    let error = engine
        .compile_package_hot_reload_update(&initial, &snapshot, &util_request)
        .expect_err("root request change must be explicit");

    assert_eq!(
        error.kind,
        EnginePackageErrorKind::RequestFingerprintMismatch
    );
    remove_fixture(root);
}

#[test]
fn ordinary_dependency_body_reload_updates_root_package_calls() {
    let root = package_fixture("reload_body");
    write_package(
        &root,
        "dev.vela.app",
        "app",
        "[dependencies]\nutil = { path = \"util\" }\n",
        "use util::api::value\npub fn main() { return value(); }\n",
    );
    write_package(
        &root.join("util"),
        "dev.vela.util",
        "util",
        "",
        "pub fn value() -> i64 { return 1; }\n",
    );
    let engine = Engine::builder().build().expect("engine");
    let first = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("first snapshot");
    let request = PackageCompileRequest::for_root(&first, &package("dev.vela.app"));
    let initial = engine
        .compile_package_hot_reload_initial(&first, &request)
        .expect("initial version");

    fs::write(
        root.join("util/src/api.vela"),
        "pub fn value() -> i64 { return 2; }\n",
    )
    .expect("change dependency body");
    let second = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("second snapshot");
    let request = PackageCompileRequest::for_root(&second, &package("dev.vela.app"));
    let update = engine
        .compile_package_hot_reload_update(&initial, &second, &request)
        .expect("dependency body update accepted");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    assert_eq!(call_i64(&mut runtime), 1);
    runtime
        .apply_hot_update(update)
        .expect("apply package update");
    assert_eq!(call_i64(&mut runtime), 2);
    remove_fixture(root);
}

#[test]
fn ordinary_package_request_rejects_another_snapshot() {
    let root = package_fixture("snapshot_mismatch");
    write_package(
        &root,
        "dev.vela.app",
        "app",
        "",
        "pub fn main() { return 1; }\n",
    );
    let engine = Engine::builder().build().expect("engine");
    let first = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("first snapshot");
    let second = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("second snapshot");
    let request = PackageCompileRequest::for_root(&first, &package("dev.vela.app"));
    let error = engine
        .compile_packages(&second, &request)
        .expect_err("snapshot mismatch rejected");

    assert!(matches!(
        &error.kind,
        EnginePackageErrorKind::SnapshotMismatch { .. }
    ));
    remove_fixture(root);
}

#[test]
fn ordinary_package_capability_use_must_be_declared_and_granted() {
    let root = package_fixture("capabilities");
    write_package(
        &root,
        "dev.vela.app",
        "app",
        "",
        "pub fn main() { return time::now(); }\n",
    );
    let trusted = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .with_time_clock(1, 1)
        .build()
        .expect("trusted engine");
    let snapshot = trusted
        .load_package_workspace(root.join("vela.toml"))
        .expect("snapshot");
    let error = trusted
        .compile_package(&snapshot, &package("dev.vela.app"))
        .expect_err("undeclared time effect rejected");
    assert!(
        matches!(
            &error.kind,
            EnginePackageErrorKind::UndeclaredCapabilities { .. }
        ),
        "unexpected error: {error:?}"
    );

    fs::write(
        root.join("vela.toml"),
        "[package]\nid = \"dev.vela.app\"\nname = \"app\"\nversion = \"0.1.0\"\n[source]\nroots = [\"src\"]\n[capabilities]\nrequires = [\"time\"]\n",
    )
    .expect("declare time capability");
    let sandboxed = Engine::builder()
        .with_time_clock(1, 1)
        .build()
        .expect("sandboxed engine");
    let snapshot = sandboxed
        .load_package_workspace(root.join("vela.toml"))
        .expect("declared snapshot");
    let error = sandboxed
        .compile_package(&snapshot, &package("dev.vela.app"))
        .expect_err("missing host grant rejected");
    assert!(matches!(
        error.kind,
        EnginePackageErrorKind::MissingHostGrants { .. }
    ));

    let snapshot = trusted
        .load_package_workspace(root.join("vela.toml"))
        .expect("trusted declared snapshot");
    trusted
        .compile_package(&snapshot, &package("dev.vela.app"))
        .expect("declared and granted time capability compiles");
    remove_fixture(root);
}

#[test]
fn ordinary_dependency_abi_change_is_rejected_without_image_advance() {
    let root = package_fixture("reload_abi");
    write_package(
        &root,
        "dev.vela.app",
        "app",
        "[dependencies]\nutil = { path = \"util\" }\n",
        "use util::api::value\npub fn main() { return value(); }\n",
    );
    write_package(
        &root.join("util"),
        "dev.vela.util",
        "util",
        "",
        "pub fn value() -> i64 { return 1; }\n",
    );
    let engine = Engine::builder().build().expect("engine");
    let first = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("first snapshot");
    let request = PackageCompileRequest::for_root(&first, &package("dev.vela.app"));
    let initial = engine
        .compile_package_hot_reload_initial(&first, &request)
        .expect("initial version");

    fs::write(
        root.join("util/src/api.vela"),
        "pub fn value() -> String { return \"changed\"; }\n",
    )
    .expect("change dependency ABI");
    let second = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("second snapshot");
    let request = PackageCompileRequest::for_root(&second, &package("dev.vela.app"));
    let error = engine
        .compile_package_hot_reload_update(&initial, &second, &request)
        .expect_err("dependency ABI change rejected");
    assert!(matches!(error.kind, EnginePackageErrorKind::HotReload(_)));

    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    assert_eq!(call_i64(&mut runtime), 1);
    remove_fixture(root);
}

fn package_fixture(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "vela_package_compile_{name}_{}_{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).expect("create fixture root");
    root
}

fn write_package(root: &Path, id: &str, name: &str, extra_manifest: &str, source: &str) {
    fs::create_dir_all(root.join("src")).expect("create package source");
    fs::write(
        root.join("vela.toml"),
        format!(
            "[package]\nid = \"{id}\"\nname = \"{name}\"\nversion = \"0.1.0\"\n[source]\nroots = [\"src\"]\n{extra_manifest}"
        ),
    )
    .expect("write manifest");
    fs::write(root.join("src/api.vela"), source).expect("write source");
    if name == "app" {
        fs::rename(root.join("src/api.vela"), root.join("src/main.vela"))
            .expect("name app module main");
    }
}

fn package(id: &str) -> PackageId {
    PackageId::new(id).expect("valid package ID")
}

fn call_i64(runtime: &mut Runtime) -> i64 {
    let output = runtime
        .call("main::main", CallArgs::new(), CallOptions::unbounded())
        .expect("package entry runs");
    match runtime.value_to_owned(&output).expect("materialize result") {
        OwnedValue::Scalar(ScalarValue::I64(value)) => value,
        other => panic!("expected i64 package result, got {other:?}"),
    }
}

fn remove_fixture(root: PathBuf) {
    fs::remove_dir_all(root).expect("remove fixture");
}
