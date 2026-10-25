use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use vela_common::{HostObjectId, HostTypeId, ScalarValue};
use vela_def::{FieldId, TypeId};
use vela_host::mock::MockStateAdapter;
use vela_host::path::{HostPath, HostRef};
use vela_host::value::HostValue;
use vela_package::PackageId;
use vela_reflect::registry::{FieldDesc, TypeDesc, TypeKey};
use vela_vm::budget::ExecutionBudgetKind;
use vela_vm::error::VmErrorKind;
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
    let mut runtime =
        Runtime::from_linked_artifact(engine, artifact).expect("runtime should initialize");

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
fn ordinary_package_compiles_and_runs() {
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
    let mut runtime =
        Runtime::from_linked_artifact(engine, artifact).expect("runtime should initialize");

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
fn ordinary_package_artifact_retains_root_request() {
    let root = package_fixture("root_request");
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
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    assert_eq!(call_i64(&mut runtime), 1);
    runtime
        .apply_reload_update_for_test(update)
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

    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
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

#[test]
fn ordinary_package_reload_reapplies_previous_root_set() {
    let root = package_fixture("ordinary_root_reapply");
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
    let request = PackageCompileRequest::for_root(&first, &package("dev.vela.app"));
    let initial = engine
        .compile_package_hot_reload_initial(&first, &request)
        .expect("initial version");

    fs::write(root.join("src/main.vela"), "pub fn main() { return 2; }\n").expect("change body");
    let update = engine
        .compile_package_workspace_hot_reload_update_from_previous(&initial, root.join("vela.toml"))
        .expect("previous roots reapplied");

    assert_eq!(
        update
            .linked_artifact()
            .package_metadata()
            .expect("package metadata")
            .request()
            .roots(),
        &[package("dev.vela.app")]
    );
    remove_fixture(root);
}

#[test]
fn dependency_change_reports_impacted_packages() {
    let root = package_fixture("package_impact_report");
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
        "pub fn answer() { return 1; }\n",
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
        "pub fn answer() { return 2; }\n",
    )
    .expect("change dependency body");
    let second = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("second snapshot");
    let update = engine
        .compile_package_hot_reload_update_from_previous(&initial, &second)
        .expect("dependency body update");

    assert_eq!(update.changed_packages(), &["dev.vela.util"]);
    assert!(
        update
            .impacted_packages()
            .contains(&"dev.vela.app".to_owned())
    );
    remove_fixture(root);
}

#[test]
fn ordinary_package_call_uses_normal_execution_budget() {
    let root = package_fixture("package_budget");
    write_package(
        &root,
        "dev.vela.plugin",
        "plugin",
        "",
        r#"pub fn run() -> i64 {
        let total = 0;
        for value in 1..=100 { total += value; }
        return total;
}
"#,
    );
    let engine = Engine::builder().build().expect("engine");
    let snapshot = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("snapshot");
    let artifact = engine
        .compile_package(&snapshot, &package("dev.vela.plugin"))
        .expect("ordinary package compiles");
    let mut runtime =
        Runtime::from_linked_artifact(engine, artifact).expect("runtime should initialize");

    let error = runtime
        .call(
            "api::run",
            CallArgs::new(),
            CallOptions::new(4, usize::MAX, usize::MAX),
        )
        .expect_err("package call exhausts the execution budget");
    assert!(matches!(
        error.kind_ref(),
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::ExecutionUnits,
            ..
        }
    ));
    remove_fixture(root);
}

#[test]
fn ordinary_package_call_uses_normal_budget_host_access_and_capability_checks() {
    let root = package_fixture("package_host_access");
    write_package(
        &root,
        "dev.vela.plugin",
        "plugin",
        "[capabilities]\nrequires = [\"host_read\", \"host_write\"]\n",
        r#"pub fn level_up(player: Player) -> i64 {
        player.level += 1;
        return player.level;
}
"#,
    );
    let host_type = HostTypeId::new(1);
    let field = FieldId::new(1);
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_type_desc(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .host_type(host_type)
                .field(FieldDesc::new(field, "level").writable(true)),
        )
        .build()
        .expect("engine");
    let snapshot = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("snapshot");
    let artifact = engine
        .compile_package(&snapshot, &package("dev.vela.plugin"))
        .expect("ordinary package compiles");
    let mut runtime =
        Runtime::from_linked_artifact(engine, artifact).expect("runtime should initialize");
    let player = HostRef::new(host_type, HostObjectId::new(42), 1);
    let level = HostPath::new(player).field(field);
    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(level.clone(), HostValue::Scalar(ScalarValue::I64(9)));

    let output = runtime
        .call(
            "api::level_up",
            CallArgs::new()
                .with_host_handle("player", player)
                .with_fallback_adapter(&mut adapter),
            CallOptions::new(1_000, usize::MAX, usize::MAX),
        )
        .expect("package host mutation runs");
    assert_eq!(
        runtime.value_to_owned(&output),
        Ok(OwnedValue::Scalar(ScalarValue::I64(10)))
    );
    assert_eq!(
        adapter.read_diagnostic_path(&level),
        Ok(HostValue::Scalar(ScalarValue::I64(10)))
    );
    remove_fixture(root);
}

#[test]
fn old_package_closure_keeps_old_generation_and_new_call_uses_new_generation() {
    let root = package_fixture("package_retained_closure_reload");
    let source = |value| {
        format!(
            "fn helper() {{ return {value}; }}\n\
         pub fn callback() {{ return || helper(); }}\n\
         pub fn invoke(callback) {{ return callback(); }}"
        )
    };
    write_package(&root, "dev.vela.plugin", "plugin", "", &source(1));
    let engine = Engine::builder().build().expect("engine");
    let first = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("first snapshot");
    let request = PackageCompileRequest::for_root(&first, &package("dev.vela.plugin"));
    let initial = engine
        .compile_package_hot_reload_initial(&first, &request)
        .expect("initial version");
    let mut runtime =
        Runtime::from_hot_reload_version(engine.clone(), initial.clone()).expect("runtime");
    let old_callback = runtime
        .call("api::callback", CallArgs::new(), CallOptions::unbounded())
        .expect("old callback");

    fs::write(root.join("src/api.vela"), source(2)).expect("change helper body");
    let second = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("second snapshot");
    let update = engine
        .compile_package_hot_reload_update_from_previous(&initial, &second)
        .expect("compatible update");
    runtime.stage_reload_update(update).expect("stage update");
    assert!(
        runtime
            .activate_reload()
            .expect("safe point")
            .expect("report")
            .accepted
    );
    let new_callback = runtime
        .call("api::callback", CallArgs::new(), CallOptions::unbounded())
        .expect("new callback");

    for (callback, expected) in [(old_callback, 1), (new_callback, 2)] {
        let value = runtime
            .call(
                "api::invoke",
                CallArgs::new().with_named_vela_value("callback", callback),
                CallOptions::unbounded(),
            )
            .expect("invoke callback");
        assert_eq!(
            runtime.value_to_owned(&value),
            Ok(OwnedValue::i64(expected))
        );
    }
    remove_fixture(root);
}

#[test]
fn ordinary_package_capability_expansion_requires_host_approval() {
    let root = package_fixture("package_capability_reload");
    write_package(
        &root,
        "dev.vela.app",
        "app",
        "",
        "pub fn main() -> i64 { return 1; }",
    );
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .with_time_clock(1, 1)
        .build()
        .expect("engine");
    let first = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("first snapshot");
    let request = PackageCompileRequest::for_root(&first, &package("dev.vela.app"));
    let initial = engine
        .compile_package_hot_reload_initial(&first, &request)
        .expect("initial version");
    fs::write(root.join("vela.toml"), "[package]\nid = \"dev.vela.app\"\nname = \"app\"\nversion = \"0.1.0\"\n[source]\nroots = [\"src\"]\n[capabilities]\nrequires = [\"time\"]\n").expect("expand capabilities");
    fs::write(
        root.join("src/main.vela"),
        "pub fn main() -> i64 { return time::now(); }",
    )
    .expect("use expanded capability");
    let second = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("second snapshot");
    let error = engine
        .compile_package_hot_reload_update_from_previous(&initial, &second)
        .expect_err("capability expansion rejected");
    assert!(matches!(
        error.kind,
        EnginePackageErrorKind::HotReload(vela_hot_reload::error::HotReloadError {
            kind: vela_hot_reload::error::HotReloadErrorKind::ChangedPackageAbi { .. }
        })
    ));
    let mut runtime = Runtime::from_hot_reload_version(engine, initial).expect("runtime");
    assert_eq!(call_i64(&mut runtime), 1);
    remove_fixture(root);
}
