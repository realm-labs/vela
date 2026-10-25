use vela_bytecode::{LinkedArtifact, PackageArtifactMetadata};

use crate::error::{HotReloadError, HotReloadErrorKind, HotReloadResult};

pub(crate) fn ensure_compatible_package_update(
    previous: &LinkedArtifact,
    next: &LinkedArtifact,
) -> HotReloadResult<()> {
    match (previous.package_metadata(), next.package_metadata()) {
        (None, None) => Ok(()),
        (Some(previous), Some(next)) => ensure_compatible_metadata(previous, next),
        _ => incompatible(
            "package request",
            "package metadata presence changed",
            None,
            None,
        ),
    }
}

fn ensure_compatible_metadata(
    previous: &PackageArtifactMetadata,
    next: &PackageArtifactMetadata,
) -> HotReloadResult<()> {
    if previous.request().roots() != next.request().roots() {
        return incompatible(
            "package roots",
            "compiled root package set changed",
            None,
            None,
        );
    }
    for old_package in previous.packages() {
        let Some(new_package) = next
            .packages()
            .iter()
            .find(|package| package.id() == old_package.id())
        else {
            return incompatible(
                &format!("package {}", old_package.id()),
                "compiled package was removed",
                None,
                Some(old_package.manifest_path().to_owned()),
            );
        };
        let declared_expansion = new_package
            .declared_capabilities()
            .difference(old_package.declared_capabilities());
        let observed_expansion = new_package
            .observed_capabilities()
            .difference(old_package.observed_capabilities());
        if !declared_expansion.is_empty() || !observed_expansion.is_empty() {
            return incompatible(
                &format!("package {}", old_package.id()),
                "package capability requirements expanded without reload approval",
                None,
                Some(new_package.manifest_path().to_owned()),
            );
        }
    }
    Ok(())
}

fn incompatible<T>(
    target: &str,
    reason: &str,
    source_span: Option<vela_common::Span>,
    manifest_path: Option<std::path::PathBuf>,
) -> HotReloadResult<T> {
    Err(HotReloadError::new(HotReloadErrorKind::ChangedPackageAbi {
        target: target.to_owned(),
        reason: reason.to_owned(),
        source_span: source_span.map(Box::new),
        manifest_path,
    }))
}
