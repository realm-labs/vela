use super::*;
use vela_common::Capability;

fn package(id: &str) -> PackageCompilationInput {
    PackageCompilationInput {
        id: PackageId::new(id).expect("package ID"),
        version: PackageVersion::new("1.2.3").expect("package version"),
        declared_capabilities: CapabilitySet::new().with(Capability::Time),
        manifest_path: format!("{id}/vela.toml").into(),
    }
}

#[test]
fn ordinary_metadata_preserves_canonical_roots_packages_and_capabilities() {
    let first = package("app.first");
    let second = package("app.second");
    let roots = BTreeSet::from([second.id.clone(), first.id.clone()]);
    let observed = BTreeMap::from([(
        first.id.clone(),
        CapabilitySet::new().with(Capability::Random),
    )]);
    let metadata =
        PackageCompilationMetadata::new(&roots, &[second.clone(), first.clone()], &observed)
            .expect("ordinary package metadata");
    let reordered = PackageCompilationMetadata::new(&roots, &[first.clone(), second], &observed)
        .expect("reordered package metadata");
    assert_eq!(metadata, reordered);
    assert_eq!(
        metadata.request().roots(),
        roots.into_iter().collect::<Vec<_>>()
    );
    let linked = metadata.into_artifact();
    assert_eq!(linked.request(), reordered.request());
    assert_eq!(linked.packages(), reordered.packages());
    let retained = &linked.packages()[0];
    assert_eq!(retained.id(), &first.id);
    assert_eq!(retained.version(), &first.version);
    assert_eq!(retained.manifest_path(), first.manifest_path);
    assert_eq!(
        retained.declared_capabilities(),
        first.declared_capabilities
    );
    assert_eq!(retained.observed_capabilities(), observed[&first.id]);
    assert!(linked.packages()[1].observed_capabilities().is_empty());
}

#[test]
fn ordinary_metadata_rejects_empty_missing_and_duplicate_roots_or_packages() {
    let first = package("app.first");
    let missing = package("app.missing");
    for (roots, packages, expected) in [
        (BTreeSet::new(), vec![first.clone()], "at least one root"),
        (
            BTreeSet::from([missing.id]),
            vec![first.clone()],
            "root is absent",
        ),
        (
            BTreeSet::from([first.id.clone()]),
            vec![first.clone(), first],
            "duplicate package ID",
        ),
    ] {
        let error = PackageCompilationMetadata::new(&roots, &packages, &BTreeMap::new())
            .expect_err("ordinary package invariants remain mandatory");
        assert!(error.contains(expected), "{error}");
    }
}

#[cfg(feature = "artifact-codec")]
#[test]
fn ordinary_package_metadata_still_rejects_portable_encoding() {
    use crate::compiler::options::CompilerOptions;
    use crate::compiler::{PackageProgramCompilationRequest, compile_package_program};
    use crate::{Linker, PortableArtifactError, PortableProgramArtifact};
    use vela_common::SourceId;
    use vela_hir::module_graph::ModuleSource;
    use vela_hir::source_ingestion::build_module_source_set;
    use vela_package::ModulePath;

    let package = package("app.first");
    let sources = build_module_source_set(&[ModuleSource::new(
        SourceId::new(1),
        package.id.clone(),
        ModulePath::root(),
        "pub fn main() { return 1; }",
    )])
    .expect("ordinary package HIR");
    let roots = BTreeSet::from([package.id.clone()]);
    let packages = [package];
    let compile = || {
        compile_package_program(PackageProgramCompilationRequest {
            sources: &sources,
            options: &CompilerOptions::default(),
            registry: None,
            roots: &roots,
            packages: &packages,
        })
        .expect("ordinary package compilation")
    };
    assert!(matches!(
        PortableProgramArtifact::from_compiled(compile()),
        Err(PortableArtifactError::UnsupportedPackageMetadata)
    ));
    let artifact = Linker::new()
        .link_compiled_program(compile())
        .expect("ordinary package linking");
    assert!(matches!(
        PortableProgramArtifact::from_linked(&artifact),
        Err(PortableArtifactError::UnsupportedPackageMetadata)
    ));
}
