use vela_bytecode::{LinkedArtifact, PackageArtifactMetadata};

use crate::error::{HotReloadError, HotReloadErrorKind, HotReloadResult};

pub(crate) fn ensure_compatible_package_update(
    previous: &LinkedArtifact,
    next: &LinkedArtifact,
) -> HotReloadResult<()> {
    match (previous.package_metadata(), next.package_metadata()) {
        (None, None) => Ok(()),
        (Some(previous), Some(next)) => ensure_compatible_metadata(previous, next),
        _ => incompatible("package request", "package metadata presence changed"),
    }
}

fn ensure_compatible_metadata(
    previous: &PackageArtifactMetadata,
    next: &PackageArtifactMetadata,
) -> HotReloadResult<()> {
    if previous.request().roots() != next.request().roots() {
        return incompatible("package roots", "compiled root package set changed");
    }
    if previous.request().providers() != next.request().providers() {
        return incompatible("provider selection", "installed provider selection changed");
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
            );
        }
    }
    let previous_providers = previous.installed_providers();
    let next_providers = next.installed_providers();
    for (key, old_provider) in previous_providers.iter() {
        let Some(new_provider) = next_providers.get(key) else {
            return incompatible(
                &format!("provider {}", key.provider()),
                "selected provider was removed",
            );
        };
        if old_provider.provider_type_id() != new_provider.provider_type_id() {
            return incompatible(
                &format!("provider {}", key.provider()),
                "provider target type changed",
            );
        }
        if !old_provider.method_ids().eq(new_provider.method_ids()) {
            return incompatible(
                &format!("provider {}", key.provider()),
                "provider service method set changed",
            );
        }
    }
    Ok(())
}

fn incompatible<T>(target: &str, reason: &str) -> HotReloadResult<T> {
    Err(HotReloadError::new(
        HotReloadErrorKind::ChangedPackageProviderAbi {
            target: target.to_owned(),
            reason: reason.to_owned(),
        },
    ))
}
