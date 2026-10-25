use std::collections::BTreeSet;
use std::sync::Arc;

use vela_bytecode::compiler::options::CompilerOptions;
use vela_bytecode::compiler::{PackageProgramCompilationRequest, compile_package_program};
use vela_bytecode::{LinkedArtifact, Linker, PackageCompilationInput};
use vela_common::{Capability, CapabilitySet, SourceId};
use vela_hir::module_graph::ModuleSource;
use vela_hir::source_ingestion::build_module_source_set;
use vela_package::{ModulePath, PackageId, PackageVersion};

use crate::abi::HotReloadAbi;
use crate::compile::{initial_version_from_linked_artifact, update_from_linked_artifact};
use crate::error::HotReloadErrorKind;
use crate::package_abi::ensure_compatible_package_update;
use crate::policy::HotReloadPolicy;

fn package(name: &str, capabilities: CapabilitySet) -> PackageCompilationInput {
    PackageCompilationInput {
        id: PackageId::new(name).expect("package ID"),
        version: PackageVersion::new("1.0.0").expect("package version"),
        declared_capabilities: capabilities,
        manifest_path: format!("{name}/vela.toml").into(),
    }
}

fn artifact(
    roots: &[PackageId],
    packages: &[PackageCompilationInput],
    value: i64,
) -> Arc<LinkedArtifact> {
    artifact_with_body(
        roots,
        packages,
        &format!("pub fn main() {{ return {value}; }}"),
    )
}

fn artifact_with_body(
    roots: &[PackageId],
    packages: &[PackageCompilationInput],
    body: &str,
) -> Arc<LinkedArtifact> {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let clock = vela_registry::FunctionDef::new(
        vela_def::DefPath::function("host", std::iter::empty::<&str>(), "clock"),
        vela_registry::FunctionSignature::new([], Some("i64")),
    )
    .effects(vela_registry::EffectSet {
        time: true,
        ..Default::default()
    });
    let clock_id = clock.id;
    registry
        .register_function(clock)
        .expect("native clock definition");
    let modules = packages
        .iter()
        .enumerate()
        .map(|(index, package)| {
            ModuleSource::new(
                SourceId::new(index as u32 + 1),
                package.id.clone(),
                ModulePath::root(),
                body,
            )
        })
        .collect::<Vec<_>>();
    let sources = build_module_source_set(&modules).expect("ordinary package source set");
    let program = compile_package_program(PackageProgramCompilationRequest {
        sources: &sources,
        options: &CompilerOptions::default(),
        registry: Some(registry.compile_view()),
        roots: &roots.iter().cloned().collect::<BTreeSet<_>>(),
        packages,
    })
    .expect("ordinary packages compile without provider selection");
    Linker::with_registry(&registry)
        .with_native_implementation(clock_id)
        .link_compiled_program(program)
        .expect("ordinary package artifact")
}

#[test]
fn ordinary_package_body_reload_retains_package_metadata() {
    let package = package("app.main", CapabilitySet::new());
    let roots = [package.id.clone()];
    let old = artifact(&roots, std::slice::from_ref(&package), 1);
    let next = artifact(&roots, std::slice::from_ref(&package), 2);
    let metadata = old.package_metadata().expect("ordinary package metadata");
    assert_eq!(metadata.request().roots(), roots);
    assert_eq!(metadata.packages()[0].id(), &package.id);
    assert_eq!(
        metadata.packages()[0].manifest_path(),
        package.manifest_path
    );
    let previous = initial_version_from_linked_artifact(HotReloadAbi::empty(), old)
        .expect("initial package version");
    update_from_linked_artifact(
        &previous,
        HotReloadAbi::empty(),
        &HotReloadPolicy::default(),
        next,
    )
    .expect("ordinary body-only reload remains compatible");
}

#[test]
fn ordinary_package_root_changes_are_rejected() {
    let first = package("app.first", CapabilitySet::new());
    let second = package("app.second", CapabilitySet::new());
    let packages = [first.clone(), second.clone()];
    let old = artifact(&[first.id], &packages, 1);
    let next = artifact(&[second.id], &packages, 1);
    let error = ensure_compatible_package_update(&old, &next).expect_err("roots remain ABI");
    assert!(matches!(
        error.kind,
        HotReloadErrorKind::ChangedPackageAbi { .. }
    ));
    assert_eq!(error.code(), "reload.package.changed_abi");
}

#[test]
fn ordinary_package_removal_is_rejected_with_the_manifest_path() {
    let root = package("app.main", CapabilitySet::new());
    let dependency = package("app.dependency", CapabilitySet::new());
    let roots = [root.id.clone()];
    let old = artifact(&roots, &[root.clone(), dependency.clone()], 1);
    let next = artifact(&roots, &[root], 1);
    let error = ensure_compatible_package_update(&old, &next)
        .expect_err("compiled package removal remains incompatible");
    assert_eq!(error.code(), "reload.package.changed_abi");
    assert_eq!(
        error.manifest_path(),
        Some(dependency.manifest_path.as_path())
    );
}

#[test]
fn ordinary_package_capability_expansion_is_rejected_and_reduction_is_allowed() {
    let old_package = package("app.main", CapabilitySet::new());
    let expanded = package("app.main", CapabilitySet::new().with(Capability::Time));
    let roots = [old_package.id.clone()];
    let old = artifact(&roots, &[old_package], 1);
    let next = artifact(&roots, std::slice::from_ref(&expanded), 1);
    let error = ensure_compatible_package_update(&old, &next)
        .expect_err("declared capability expansion remains incompatible");
    assert_eq!(error.code(), "reload.package.changed_abi");
    assert_eq!(
        error.manifest_path(),
        Some(expanded.manifest_path.as_path())
    );
    ensure_compatible_package_update(&next, &old).expect("capability reduction is compatible");
}

#[test]
fn ordinary_package_observed_capability_expansion_is_rejected() {
    let package = package("app.main", CapabilitySet::new().with(Capability::Time));
    let roots = [package.id.clone()];
    let packages = [package];
    let old = artifact(&roots, &packages, 1);
    let next = artifact_with_body(&roots, &packages, "pub fn main() { return clock(); }");
    let observed = next
        .package_metadata()
        .expect("package metadata")
        .packages()[0]
        .observed_capabilities();
    assert!(observed.contains(Capability::Time));
    let error = ensure_compatible_package_update(&old, &next)
        .expect_err("observed capability expansion remains incompatible");
    assert_eq!(error.code(), "reload.package.changed_abi");
    assert_eq!(
        error.manifest_path(),
        Some(packages[0].manifest_path.as_path())
    );
}

#[test]
fn ordinary_package_metadata_presence_changes_are_rejected() {
    let package = package("app.main", CapabilitySet::new());
    let with_metadata = artifact(
        std::slice::from_ref(&package.id),
        std::slice::from_ref(&package),
        1,
    );
    let without_metadata = super::compile_initial(SourceId::new(1), "fn main() { return 1; }")
        .expect("ordinary single-source version");
    for (old, next) in [
        (
            with_metadata.as_ref(),
            without_metadata.linked_artifact().as_ref(),
        ),
        (
            without_metadata.linked_artifact().as_ref(),
            with_metadata.as_ref(),
        ),
    ] {
        let error = ensure_compatible_package_update(old, next)
            .expect_err("metadata cannot silently disappear or appear on reload");
        assert_eq!(error.code(), "reload.package.changed_abi");
    }
}
