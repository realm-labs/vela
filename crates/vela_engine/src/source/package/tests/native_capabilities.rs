use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use vela_common::CapabilitySet;
use vela_def::FunctionId;
use vela_hot_reload::error::{HotReloadError, HotReloadErrorKind};
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::native::{EffectSet, NativeFunctionDesc, TypeHint};
use crate::permission::Capability;
use crate::runtime::{CallArgs, CallOptions, Runtime};
use crate::source::package::{EnginePackageErrorKind, PackageCompileRequest};

use super::{
    call_i64, copy_package_fixture, package, package_fixture, remove_fixture, write_package,
};

const VALUE_SOURCE: &str = "fn invoke_native() -> i64 { return test::value(); } pub fn main() -> i64 { return invoke_native(); }";

fn native_engine(effects: EffectSet, calls: &Arc<AtomicUsize>) -> Engine {
    let calls = Arc::clone(calls);
    Engine::builder()
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .register_native_fn(
            NativeFunctionDesc::new("test::value", FunctionId::new(0xDCA0))
                .returns(TypeHint::i64())
                .effects(effects),
            move |_| {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(OwnedValue::i64(42))
            },
        )
        .build()
        .expect("value-only native engine")
}

fn declaration(capability: Capability) -> String {
    format!("[capabilities]\nrequires = [\"{}\"]\n", capability.as_str())
}

fn assert_native_requires_declaration(effects: EffectSet, capability: Capability) {
    let root = package_fixture(capability.as_str());
    let calls = Arc::new(AtomicUsize::new(0));
    let engine = native_engine(effects, &calls);
    let id = package("dev.vela.app");
    write_package(&root, id.as_str(), "app", "", VALUE_SOURCE);
    let snapshot = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("undeclared snapshot");
    let error = engine
        .compile_package(&snapshot, &id)
        .expect_err("Engine grants must not replace a package declaration");
    assert!(
        matches!(error.kind, EnginePackageErrorKind::UndeclaredCapabilities { package, missing }
        if package == id && missing == CapabilitySet::new().with(capability))
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    write_package(
        &root,
        id.as_str(),
        "app",
        &declaration(capability),
        VALUE_SOURCE,
    );
    let snapshot = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("declared snapshot");
    let artifact = engine
        .compile_package(&snapshot, &id)
        .expect("declared native effect compiles");
    let metadata = artifact.package_metadata().expect("package metadata");
    let compiled = metadata
        .packages()
        .iter()
        .find(|package| package.id() == &id)
        .expect("root package metadata");
    assert_eq!(
        compiled.observed_capabilities(),
        CapabilitySet::new().with(capability)
    );
    assert_eq!(
        compiled.declared_capabilities(),
        CapabilitySet::new().with(capability)
    );
    let mut runtime = Runtime::from_linked_artifact(engine, artifact).expect("runtime");
    assert_eq!(call_i64(&mut runtime), 42);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    remove_fixture(root);
}

#[test]
fn value_native_host_read_requires_package_declaration_despite_engine_grant() {
    assert_native_requires_declaration(EffectSet::host_read(), Capability::HostRead);
}

#[test]
fn value_native_host_write_requires_package_declaration_despite_engine_grant() {
    assert_native_requires_declaration(EffectSet::host_write(), Capability::HostWrite);
}

fn assert_reload_rejects_new_native_effect(
    effects: EffectSet,
    capability: Capability,
    declared: bool,
) {
    let root = package_fixture("native_effect_reload");
    let calls = Arc::new(AtomicUsize::new(0));
    let engine = native_engine(effects, &calls);
    let id = package("dev.vela.app");
    let manifest = if declared {
        declaration(capability)
    } else {
        String::new()
    };
    write_package(
        &root,
        id.as_str(),
        "app",
        &manifest,
        "pub fn main() -> i64 { return 1; }",
    );
    let first = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("initial snapshot");
    let request = PackageCompileRequest::for_root(&first, &id);
    let initial = engine
        .compile_package_hot_reload_initial(&first, &request)
        .expect("initial generation");
    let owner = Arc::clone(initial.linked_artifact());
    let previous = owner
        .package_metadata()
        .expect("previous metadata")
        .packages()
        .iter()
        .find(|package| package.id() == &id)
        .expect("previous package");
    assert!(previous.observed_capabilities().is_empty());
    write_package(&root, id.as_str(), "app", &manifest, VALUE_SOURCE);
    let next = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("updated snapshot");
    if declared {
        let artifact = engine
            .compile_package(&next, &id)
            .expect("unchanged declaration allows ordinary compile");
        let observed = artifact
            .package_metadata()
            .expect("new metadata")
            .packages()
            .iter()
            .find(|package| package.id() == &id)
            .expect("new package");
        assert_eq!(
            observed.declared_capabilities(),
            previous.declared_capabilities()
        );
        assert_eq!(
            observed.observed_capabilities(),
            CapabilitySet::new().with(capability)
        );
    }
    let error = engine
        .compile_package_hot_reload_update_from_previous(&initial, &next)
        .expect_err("new native capability must not silently enter a reload");
    if declared {
        assert!(
            matches!(error.kind, EnginePackageErrorKind::HotReload(HotReloadError {
            kind: HotReloadErrorKind::ChangedPackageAbi { target, reason, .. }
        }) if target == format!("package {id}") && reason == "package capability requirements expanded without reload approval")
        );
    } else {
        assert!(
            matches!(error.kind, EnginePackageErrorKind::UndeclaredCapabilities { package, missing }
            if package == id && missing == CapabilitySet::new().with(capability))
        );
    }
    let mut runtime = Runtime::from_hot_reload_version(engine, initial).expect("original runtime");
    assert_eq!(call_i64(&mut runtime), 1);
    assert!(Arc::ptr_eq(
        runtime
            .hot_reload_version()
            .expect("active generation")
            .linked_artifact(),
        &owner
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    remove_fixture(root);
}

#[test]
fn reload_rejects_undeclared_value_native_host_read() {
    assert_reload_rejects_new_native_effect(EffectSet::host_read(), Capability::HostRead, false);
}

#[test]
fn reload_rejects_undeclared_value_native_host_write() {
    assert_reload_rejects_new_native_effect(EffectSet::host_write(), Capability::HostWrite, false);
}

#[test]
fn reload_rejects_new_observed_value_native_host_read_even_when_declared() {
    assert_reload_rejects_new_native_effect(EffectSet::host_read(), Capability::HostRead, true);
}

#[test]
fn reload_rejects_new_observed_value_native_host_write_even_when_declared() {
    assert_reload_rejects_new_native_effect(EffectSet::host_write(), Capability::HostWrite, true);
}

fn read_write_engine(reads: &Arc<AtomicUsize>, writes: &Arc<AtomicUsize>) -> Engine {
    let reads = Arc::clone(reads);
    let writes = Arc::clone(writes);
    Engine::builder()
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .register_native_fn(
            NativeFunctionDesc::new("test::read", FunctionId::new(0xDCA3))
                .returns(TypeHint::i64())
                .effects(EffectSet::host_read()),
            move |_| {
                reads.fetch_add(1, Ordering::SeqCst);
                Ok(OwnedValue::i64(10))
            },
        )
        .register_native_fn(
            NativeFunctionDesc::new("test::write", FunctionId::new(0xDCA4))
                .returns(TypeHint::i64())
                .effects(EffectSet::host_write()),
            move |_| {
                writes.fetch_add(1, Ordering::SeqCst);
                Ok(OwnedValue::i64(20))
            },
        )
        .build()
        .expect("read/write native engine")
}

#[test]
fn reload_accepts_native_write_to_read_reduction_and_preserves_old_closure_owner() {
    let root = package_fixture("native_write_to_read_reload");
    let reads = Arc::new(AtomicUsize::new(0));
    let writes = Arc::new(AtomicUsize::new(0));
    let engine = read_write_engine(&reads, &writes);
    let id = package("dev.vela.app");
    copy_package_fixture("native_write_to_read", &root);
    let first = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("initial snapshot");
    let request = PackageCompileRequest::for_root(&first, &id);
    let initial = engine
        .compile_package_hot_reload_initial(&first, &request)
        .expect("initial version");
    let old_owner = Arc::clone(initial.linked_artifact());
    let previous = old_owner
        .package_metadata()
        .expect("old metadata")
        .packages()
        .iter()
        .find(|package| package.id() == &id)
        .expect("old package");
    assert_eq!(
        previous.observed_capabilities(),
        CapabilitySet::new().with(Capability::HostWrite)
    );
    let mut runtime =
        Runtime::from_hot_reload_version(engine.clone(), initial.clone()).expect("runtime");
    assert_eq!(call_i64(&mut runtime), 31);
    let callback = runtime
        .call(
            "main::make",
            CallArgs::new(),
            CallOptions::new(10_000, 1 << 20, 32),
        )
        .expect("old closure");

    std::fs::write(
        root.join("src/main.vela"),
        include_str!("../../../../tests/fixtures/packages/updates/native_read_only.vela"),
    )
    .expect("write read-only update");
    let next = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("updated snapshot");
    let artifact = engine
        .compile_package(&next, &id)
        .expect("read-only body compiles");
    let current = artifact
        .package_metadata()
        .expect("new metadata")
        .packages()
        .iter()
        .find(|package| package.id() == &id)
        .expect("new package");
    assert_eq!(
        current.declared_capabilities(),
        previous.declared_capabilities()
    );
    assert_eq!(
        current.observed_capabilities(),
        CapabilitySet::new().with(Capability::HostRead)
    );
    let update = engine
        .compile_package_hot_reload_update_from_previous(&initial, &next)
        .expect("dropping native writes is not a capability expansion");
    let report = runtime
        .apply_reload_update_for_test(update)
        .expect("compatible update activates");
    assert!(report.accepted);
    assert!(!Arc::ptr_eq(
        runtime
            .hot_reload_version()
            .expect("active generation")
            .linked_artifact(),
        &old_owner
    ));
    assert_eq!(call_i64(&mut runtime), 12);
    assert_eq!(
        writes.load(Ordering::SeqCst),
        1,
        "new calls do not invoke the removed write"
    );

    let result = runtime
        .call(
            "main::invoke",
            CallArgs::new().with_vela_value(callback),
            CallOptions::new(10_000, 1 << 20, 32),
        )
        .expect("old closure runs its original nested main");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(33)));
    assert_eq!(runtime.state("main::calls"), Ok(Some(OwnedValue::i64(3))));
    assert_eq!(reads.load(Ordering::SeqCst), 3);
    assert_eq!(
        writes.load(Ordering::SeqCst),
        2,
        "only the retained old closure invokes the old write"
    );
    assert_eq!(call_i64(&mut runtime), 14);
    assert_eq!(writes.load(Ordering::SeqCst), 2);
    remove_fixture(root);
}

#[test]
fn reload_rejects_native_read_to_write_expansion_with_unchanged_declarations() {
    let root = package_fixture("native_read_to_write_reload");
    let reads = Arc::new(AtomicUsize::new(0));
    let writes = Arc::new(AtomicUsize::new(0));
    let engine = read_write_engine(&reads, &writes);
    let id = package("dev.vela.app");
    copy_package_fixture("native_read_to_write", &root);
    let first = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("initial snapshot");
    let request = PackageCompileRequest::for_root(&first, &id);
    let initial = engine
        .compile_package_hot_reload_initial(&first, &request)
        .expect("initial version");
    let owner = Arc::clone(initial.linked_artifact());
    let mut runtime =
        Runtime::from_hot_reload_version(engine.clone(), initial.clone()).expect("runtime");
    assert_eq!(call_i64(&mut runtime), 10);
    std::fs::write(
        root.join("src/main.vela"),
        include_str!("../../../../tests/fixtures/packages/updates/native_write_expansion.vela"),
    )
    .expect("write capability expansion");
    let next = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("updated snapshot");
    let error = engine
        .compile_package_hot_reload_update_from_previous(&initial, &next)
        .expect_err("new write capability must still reject");
    assert!(matches!(
        error.kind,
        EnginePackageErrorKind::HotReload(HotReloadError {
            kind: HotReloadErrorKind::ChangedPackageAbi { .. }
        })
    ));
    assert!(Arc::ptr_eq(
        runtime
            .hot_reload_version()
            .expect("unchanged generation")
            .linked_artifact(),
        &owner
    ));
    assert_eq!(call_i64(&mut runtime), 10);
    assert_eq!(reads.load(Ordering::SeqCst), 2);
    assert_eq!(writes.load(Ordering::SeqCst), 0);
    remove_fixture(root);
}
