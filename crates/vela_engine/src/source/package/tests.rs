use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll, Waker};

use vela_common::{Capability, HostObjectId, HostTypeId, ScalarValue};
use vela_def::{FieldId, FunctionId, TypeId, script_trait_id, script_trait_method_id};
use vela_host::mock::MockStateAdapter;
use vela_host::path::{HostPath, HostRef};
use vela_host::value::HostValue;
use vela_package::PackageId;
use vela_reflect::registry::{FieldDesc, TypeDesc, TypeKey};
use vela_vm::budget::ExecutionBudgetKind;
use vela_vm::error::VmErrorKind;
use vela_vm::owned_value::OwnedValue;

use super::*;
use crate::native::{FunctionAccess, NativeFunctionDesc, TypeHint};
use crate::permission::ExecutionProfile;
use crate::runtime::{CallArgs, CallOptions, ProviderMethodTarget, Runtime};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

mod provider_async;
mod provider_reload;

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
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
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

    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    assert_eq!(call_i64(&mut runtime), 1);
    remove_fixture(root);
}

#[test]
fn catalog_reports_stable_ids_and_source_spans() {
    let root = package_fixture("provider_catalog_ids");
    write_package(
        &root,
        "dev.vela.app",
        "app",
        "",
        r#"pub trait CommandProvider { fn run(self, value: i64) -> i64; }
pub struct SortInventory {}
#[provider(id = "sort_inventory")]
impl CommandProvider for SortInventory {
    pub fn run(self, value: i64) -> i64 { return value; }
}
"#,
    );
    let engine = Engine::builder().build().expect("engine");
    let snapshot = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("snapshot");
    let catalog = engine.discover_providers(&snapshot).expect("catalog");
    let provider = &catalog.providers()[0];
    let service = script_trait_id("dev.vela.app", "main::CommandProvider");

    assert_eq!(catalog.snapshot(), snapshot.id());
    assert_eq!(provider.key().service(), service);
    assert_eq!(provider.key().provider().as_str(), "sort_inventory");
    assert_eq!(
        provider.methods()[0].id(),
        script_trait_method_id("dev.vela.app", "main::CommandProvider", "run")
    );
    assert_eq!(provider.methods()[0].name(), "run");
    assert_eq!(
        provider.source().path(),
        fs::canonicalize(root.join("src/main.vela")).expect("canonical source path")
    );
    assert!(provider.source().start() < provider.source().end());
    remove_fixture(root);
}

#[test]
fn discovery_does_not_execute_script_or_host_code() {
    let root = package_fixture("provider_catalog_no_execute");
    write_package(
        &root,
        "dev.vela.app",
        "app",
        "[capabilities]\nrequires = [\"time\"]\n",
        r#"pub trait ClockProvider { fn now(self) -> i64; }
pub struct Clock {}
#[provider(id = "clock")]
impl ClockProvider for Clock {
    pub fn now(self) -> i64 { return time::now(); }
}
"#,
    );
    let engine = Engine::builder().build().expect("engine without time host");
    let snapshot = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("snapshot");
    let catalog = engine
        .discover_providers(&snapshot)
        .expect("discovery is analysis only");

    assert_eq!(catalog.providers().len(), 1);
    assert!(
        catalog.providers()[0]
            .package_statically_observed_capabilities()
            .contains(Capability::Time)
    );
    remove_fixture(root);
}

#[test]
fn catalog_cannot_mix_selection_from_another_generation() {
    let root = package_fixture("provider_catalog_generation");
    write_package(
        &root,
        "dev.vela.app",
        "app",
        "",
        r#"pub trait CommandProvider { fn run(self) -> i64; }
pub struct Command {}
#[provider(id = "command")]
impl CommandProvider for Command { pub fn run(self) -> i64 { return 1; } }
"#,
    );
    let engine = Engine::builder().build().expect("engine");
    let first = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("first snapshot");
    let first_catalog = engine.discover_providers(&first).expect("first catalog");
    let selection = first_catalog
        .select([first_catalog.providers()[0].key().clone()])
        .expect("selection");
    let second = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("second snapshot");
    let second_catalog = engine.discover_providers(&second).expect("second catalog");

    assert!(matches!(
        second_catalog.validate_selection(&selection),
        Err(ProviderCatalogError::SnapshotMismatch { .. })
    ));
    remove_fixture(root);
}

#[test]
fn compile_provider_selection_includes_transitive_dependencies() {
    let root = package_fixture("provider_compile_closure");
    write_package(
        &root,
        "dev.vela.plugin",
        "plugin",
        "[dependencies]\napi = { path = \"api\" }\n",
        r#"use api::api::CommandProvider
pub struct First {}
#[provider(id = "first")]
impl CommandProvider for First { pub fn run(self, value: i64) -> i64 { return value + 1; } }
pub struct Second {}
#[provider(id = "second")]
impl CommandProvider for Second { pub fn run(self, value: i64) -> i64 { return value + 2; } }
"#,
    );
    write_package(
        &root.join("api"),
        "dev.vela.api",
        "api",
        "[dependencies]\nbase = { path = \"../base\" }\n",
        "pub trait CommandProvider { fn run(self, value: i64) -> i64; }\n",
    );
    write_package(
        &root.join("base"),
        "dev.vela.base",
        "base",
        "",
        "pub fn marker() { return 1; }\n",
    );
    let engine = Engine::builder().build().expect("engine");
    let snapshot = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("snapshot");
    let catalog = engine.discover_providers(&snapshot).expect("catalog");
    let selected_key = catalog.providers()[0].key().clone();
    let selection = catalog.select([selected_key.clone()]).expect("selection");
    let request = ProviderCompileRequest::for_selection(&snapshot, selection);
    let artifact = engine
        .compile_provider_selection(&snapshot, &request)
        .expect("selected provider compiles");
    let metadata = artifact.package_metadata().expect("package metadata");

    assert_eq!(metadata.packages().len(), 3);
    assert_eq!(metadata.request().roots(), &[package("dev.vela.plugin")]);
    assert_eq!(
        metadata.request().providers().providers(),
        std::slice::from_ref(&selected_key)
    );
    assert_eq!(metadata.installed_providers().len(), 1);
    assert!(metadata.installed_providers().get(&selected_key).is_some());
    remove_fixture(root);
}

#[test]
fn linked_artifact_installs_only_selected_providers() {
    compile_provider_selection_includes_transitive_dependencies();
}

#[test]
fn linked_artifact_owns_same_generation_provider_metadata() {
    let root = package_fixture("provider_linked_metadata");
    write_package(
        &root,
        "dev.vela.plugin",
        "plugin",
        "",
        r#"pub trait CommandProvider { fn run(self, value: i64) -> i64; }
pub struct Command {}
#[provider(id = "command")]
impl CommandProvider for Command {
    pub fn run(self, value: i64) -> i64 { return value + 1; }
}
"#,
    );
    let engine = Engine::builder().build().expect("engine");
    let snapshot = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("snapshot");
    let catalog = engine.discover_providers(&snapshot).expect("catalog");
    let descriptor = &catalog.providers()[0];
    let key = descriptor.key().clone();
    let method = descriptor.methods()[0].id();
    let selection = catalog.select([key.clone()]).expect("selection");
    let request = ProviderCompileRequest::for_selection(&snapshot, selection);
    let artifact = engine
        .compile_provider_selection(&snapshot, &request)
        .expect("selected provider compiles");
    let installed = artifact
        .package_metadata()
        .expect("package metadata")
        .installed_providers();
    let provider = installed.get(&key).expect("selected provider installed");
    let dispatch = provider.method(method).expect("service method linked");

    assert_eq!(provider.key(), &key);
    assert_eq!(
        artifact
            .program()
            .ty(provider.provider_type())
            .map(|ty| ty.id),
        Some(descriptor.provider_type())
    );
    assert!(artifact.program().method_dispatch(dispatch).is_some());
    remove_fixture(root);
}

#[test]
fn runtime_calls_provider_trait_impl_method() {
    let root = package_fixture("provider_runtime_call");
    write_package(
        &root,
        "dev.vela.plugin",
        "plugin",
        "",
        r#"pub trait CommandProvider { fn run(self, value: i64) -> i64; }
pub struct Command {}
#[provider(id = "command")]
impl CommandProvider for Command {
    pub fn run(self, value: i64) -> i64 { return value + 1; }
}
"#,
    );
    let engine = Engine::builder().build().expect("engine");
    let snapshot = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("snapshot");
    let catalog = engine.discover_providers(&snapshot).expect("catalog");
    let descriptor = &catalog.providers()[0];
    let key = descriptor.key().clone();
    let method = descriptor.methods()[0].id();
    let selection = catalog.select([key.clone()]).expect("selection");
    let request = ProviderCompileRequest::for_selection(&snapshot, selection);
    let artifact = engine
        .compile_provider_selection(&snapshot, &request)
        .expect("selected provider compiles");
    let mut runtime =
        Runtime::from_linked_artifact(engine, artifact).expect("runtime should initialize");
    let handle = runtime.provider_handle(&key).expect("provider handle");

    let output = runtime
        .call(
            handle.method(method),
            CallArgs::new().with_value("value", 41_i64),
            CallOptions::unbounded(),
        )
        .expect("provider method runs");
    let async_output = poll_to_completion(runtime.call_async(
        handle.method(method),
        CallArgs::new().with_value("value", 40_i64),
        CallOptions::unbounded(),
    ))
    .expect("async provider method runs");
    assert_eq!(
        runtime.value_to_owned(&output),
        Ok(OwnedValue::Scalar(ScalarValue::I64(42)))
    );
    assert_eq!(
        runtime.value_to_owned(&async_output),
        Ok(OwnedValue::Scalar(ScalarValue::I64(41)))
    );
    remove_fixture(root);
}

#[test]
fn runtime_primary_provider_call_uses_method_id_without_name_dispatch() {
    runtime_calls_provider_trait_impl_method();
}

#[test]
fn runtime_rejects_missing_provider_or_method() {
    let root = package_fixture("provider_runtime_missing");
    write_package(
        &root,
        "dev.vela.plugin",
        "plugin",
        "",
        r#"pub trait CommandProvider { fn run(self) -> i64; }
pub struct Installed {}
#[provider(id = "installed")]
impl CommandProvider for Installed { pub fn run(self) -> i64 { return 1; } }
pub struct Unselected {}
#[provider(id = "unselected")]
impl CommandProvider for Unselected { pub fn run(self) -> i64 { return 2; } }
"#,
    );
    let engine = Engine::builder().build().expect("engine");
    let snapshot = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("snapshot");
    let catalog = engine.discover_providers(&snapshot).expect("catalog");
    let installed = catalog
        .providers()
        .iter()
        .find(|provider| provider.key().provider().as_str() == "installed")
        .expect("installed descriptor");
    let unselected = catalog
        .providers()
        .iter()
        .find(|provider| provider.key().provider().as_str() == "unselected")
        .expect("unselected descriptor");
    let key = installed.key().clone();
    let selection = catalog.select([key.clone()]).expect("selection");
    let request = ProviderCompileRequest::for_selection(&snapshot, selection);
    let artifact = engine
        .compile_provider_selection(&snapshot, &request)
        .expect("selected provider compiles");
    let mut runtime =
        Runtime::from_linked_artifact(engine, artifact).expect("runtime should initialize");

    assert!(runtime.provider_handle(unselected.key()).is_err());
    let handle = runtime.provider_handle(&key).expect("provider handle");
    assert!(
        runtime
            .call(
                handle.method(vela_def::MethodId::new(u128::MAX)),
                CallArgs::new(),
                CallOptions::unbounded(),
            )
            .is_err()
    );
    remove_fixture(root);
}

#[test]
fn provider_call_constructs_fresh_zero_field_receiver() {
    let root = package_fixture("provider_fresh_receiver");
    write_package(
        &root,
        "dev.vela.plugin",
        "plugin",
        "",
        r#"pub trait InstanceProvider { fn instance(self) -> Instance; }
pub struct Instance {}
#[provider(id = "instance")]
impl InstanceProvider for Instance { pub fn instance(self) -> Instance { return self; } }
"#,
    );
    let engine = Engine::builder().build().expect("engine");
    let snapshot = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("snapshot");
    let catalog = engine.discover_providers(&snapshot).expect("catalog");
    let descriptor = &catalog.providers()[0];
    let key = descriptor.key().clone();
    let method = descriptor.methods()[0].id();
    let selection = catalog.select([key.clone()]).expect("selection");
    let request = ProviderCompileRequest::for_selection(&snapshot, selection);
    let artifact = engine
        .compile_provider_selection(&snapshot, &request)
        .expect("selected provider compiles");
    let mut runtime =
        Runtime::from_linked_artifact(engine, artifact).expect("runtime should initialize");
    let handle = runtime.provider_handle(&key).expect("provider handle");

    let first = runtime
        .call(
            handle.method(method),
            CallArgs::new(),
            CallOptions::unbounded(),
        )
        .expect("first provider call");
    let second = runtime
        .call(
            handle.method(method),
            CallArgs::new(),
            CallOptions::unbounded(),
        )
        .expect("second provider call");
    assert_ne!(first, second);
    remove_fixture(root);
}

#[test]
fn provider_handle_rejects_another_runtime() {
    let root = package_fixture("provider_runtime_handle");
    write_package(
        &root,
        "dev.vela.plugin",
        "plugin",
        "",
        r#"pub trait CommandProvider { fn run(self) -> i64; }
pub struct Command {}
#[provider(id = "command")]
impl CommandProvider for Command { pub fn run(self) -> i64 { return 1; } }
"#,
    );
    let engine = Engine::builder().build().expect("engine");
    let snapshot = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("snapshot");
    let catalog = engine.discover_providers(&snapshot).expect("catalog");
    let descriptor = &catalog.providers()[0];
    let key = descriptor.key().clone();
    let method = descriptor.methods()[0].id();
    let selection = catalog.select([key.clone()]).expect("selection");
    let request = ProviderCompileRequest::for_selection(&snapshot, selection);
    let artifact = engine
        .compile_provider_selection(&snapshot, &request)
        .expect("selected provider compiles");
    let first = Runtime::from_linked_artifact(engine.clone(), artifact.clone())
        .expect("runtime should initialize");
    let mut second =
        Runtime::from_linked_artifact(engine, artifact).expect("runtime should initialize");
    let handle = first.provider_handle(&key).expect("provider handle");

    let error = second
        .call(
            handle.method(method),
            CallArgs::new(),
            CallOptions::unbounded(),
        )
        .expect_err("provider handle from another Runtime should fail");
    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "provider handle belongs to another runtime",
        }
    );
    remove_fixture(root);
}

#[test]
fn provider_handle_rebinds_after_compatible_reload() {
    let root = package_fixture("provider_handle_reload");
    let source = |value| {
        format!(
            r#"pub trait CommandProvider {{ fn run(self) -> i64; }}
pub struct Command {{}}
#[provider(id = "command")]
impl CommandProvider for Command {{ pub fn run(self) -> i64 {{ return {value}; }} }}
"#
        )
    };
    write_package(&root, "dev.vela.plugin", "plugin", "", &source(1));
    let engine = Engine::builder().build().expect("engine");
    let first = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("first snapshot");
    let first_catalog = engine.discover_providers(&first).expect("first catalog");
    let key = first_catalog.providers()[0].key().clone();
    let method = first_catalog.providers()[0].methods()[0].id();
    let first_selection = first_catalog.select([key.clone()]).expect("selection");
    let first_request = ProviderCompileRequest::for_selection(&first, first_selection);
    let initial = engine
        .compile_provider_hot_reload_initial(&first, &first_request)
        .expect("initial version");
    let mut runtime = Runtime::from_hot_reload_version(engine.clone(), initial.clone())
        .expect("runtime should initialize");
    let handle = runtime.provider_handle(&key).expect("provider handle");
    assert_eq!(call_provider_i64(&mut runtime, &handle, method), 1);

    fs::write(root.join("src/api.vela"), source(2)).expect("replace provider body");
    let second = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("second snapshot");
    let second_catalog = engine.discover_providers(&second).expect("second catalog");
    let second_selection = second_catalog.select([key.clone()]).expect("selection");
    let second_request = ProviderCompileRequest::for_selection(&second, second_selection);
    let update = engine
        .compile_provider_hot_reload_update(&initial, &second, &second_request)
        .expect("body-only update");
    runtime.stage_hot_update(update).expect("stage update");
    runtime
        .check_reload()
        .expect("safe-point update")
        .expect("reload report");

    assert_eq!(call_provider_i64(&mut runtime, &handle, method), 2);
    remove_fixture(root);
}

#[test]
fn provider_call_uses_normal_execution_budget() {
    let root = package_fixture("provider_budget");
    write_package(
        &root,
        "dev.vela.plugin",
        "plugin",
        "",
        r#"pub trait WorkProvider { fn run(self) -> i64; }
pub struct Work {}
#[provider(id = "work")]
impl WorkProvider for Work {
    pub fn run(self) -> i64 {
        let total = 0;
        for value in 1..=100 { total += value; }
        return total;
    }
}
"#,
    );
    let engine = Engine::builder().build().expect("engine");
    let snapshot = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("snapshot");
    let catalog = engine.discover_providers(&snapshot).expect("catalog");
    let descriptor = &catalog.providers()[0];
    let key = descriptor.key().clone();
    let method = descriptor.methods()[0].id();
    let selection = catalog.select([key.clone()]).expect("selection");
    let request = ProviderCompileRequest::for_selection(&snapshot, selection);
    let artifact = engine
        .compile_provider_selection(&snapshot, &request)
        .expect("selected provider compiles");
    let mut runtime =
        Runtime::from_linked_artifact(engine, artifact).expect("runtime should initialize");
    let handle = runtime.provider_handle(&key).expect("provider handle");

    let error = runtime
        .call(
            handle.method(method),
            CallArgs::new(),
            CallOptions::new(4, usize::MAX, usize::MAX),
        )
        .expect_err("provider call exhausts the execution budget");
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
fn provider_call_uses_normal_budget_host_access_and_capability_checks() {
    let root = package_fixture("provider_host_access");
    write_package(
        &root,
        "dev.vela.plugin",
        "plugin",
        "[capabilities]\nrequires = [\"host_read\", \"host_write\"]\n",
        r#"pub trait LevelProvider { fn level_up(self, player: Player) -> i64; }
pub struct LevelUp {}
#[provider(id = "level_up")]
impl LevelProvider for LevelUp {
    pub fn level_up(self, player: Player) -> i64 {
        player.level += 1;
        return player.level;
    }
}
"#,
    );
    let host_type = HostTypeId::new(1);
    let field = FieldId::new(1);
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .host_type(host_type)
                .field(FieldDesc::new(field, "level").writable(true)),
        )
        .build()
        .expect("engine");
    let snapshot = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("snapshot");
    let catalog = engine.discover_providers(&snapshot).expect("catalog");
    let descriptor = &catalog.providers()[0];
    let key = descriptor.key().clone();
    let method = descriptor.methods()[0].id();
    let selection = catalog.select([key.clone()]).expect("selection");
    let request = ProviderCompileRequest::for_selection(&snapshot, selection);
    let artifact = engine
        .compile_provider_selection(&snapshot, &request)
        .expect("selected provider compiles");
    let mut runtime =
        Runtime::from_linked_artifact(engine, artifact).expect("runtime should initialize");
    let player = HostRef::new(host_type, HostObjectId::new(42), 1);
    let level = HostPath::new(player).field(field);
    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(level.clone(), HostValue::Scalar(ScalarValue::I64(9)));
    let handle = runtime.provider_handle(&key).expect("provider handle");

    let output = runtime
        .call(
            handle.method(method),
            CallArgs::new()
                .with_host_handle("player", player)
                .with_fallback_adapter(&mut adapter),
            CallOptions::new(1_000, usize::MAX, usize::MAX),
        )
        .expect("provider host mutation runs");
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
fn statically_observed_effect_must_be_declared_by_package() {
    let root = package_fixture("provider_catalog_capability");
    write_package(
        &root,
        "dev.vela.app",
        "app",
        "",
        r#"pub trait ClockProvider { fn now(self) -> i64; }
pub struct Clock {}
#[provider(id = "clock")]
impl ClockProvider for Clock { pub fn now(self) -> i64 { return time::now(); } }
"#,
    );
    let engine = Engine::builder().build().expect("engine");
    let snapshot = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("snapshot");
    let error = engine
        .discover_providers(&snapshot)
        .expect_err("undeclared provider effect");

    assert!(matches!(
        error.kind,
        EnginePackageErrorKind::UndeclaredCapabilities { .. }
    ));
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

fn call_provider_i64(
    runtime: &mut Runtime,
    handle: &crate::runtime::ProviderHandle,
    method: vela_def::MethodId,
) -> i64 {
    let output = runtime
        .call(
            handle.method(method),
            CallArgs::new(),
            CallOptions::unbounded(),
        )
        .expect("provider call");
    match runtime.value_to_owned(&output).expect("materialize result") {
        OwnedValue::Scalar(ScalarValue::I64(value)) => value,
        other => panic!("expected i64 provider result, got {other:?}"),
    }
}

fn poll_to_completion<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(output) = Pin::new(&mut future).poll(&mut context) {
            return output;
        }
    }
}

fn remove_fixture(root: PathBuf) {
    fs::remove_dir_all(root).expect("remove fixture");
}
