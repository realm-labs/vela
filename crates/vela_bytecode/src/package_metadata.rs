use std::collections::{BTreeMap, BTreeSet};

use vela_common::CapabilitySet;
use vela_package::{PackageId, PackageVersion};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageCompilationInput {
    pub id: PackageId,
    pub version: PackageVersion,
    pub declared_capabilities: CapabilitySet,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledPackageMetadata {
    id: PackageId,
    version: PackageVersion,
    declared_capabilities: CapabilitySet,
    observed_capabilities: CapabilitySet,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InstalledProviderSet {
    _sealed: (),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageCompileRequestFingerprint {
    roots: Box<[PackageId]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageArtifactMetadata {
    request: PackageCompileRequestFingerprint,
    packages: Box<[CompiledPackageMetadata]>,
    installed_providers: InstalledProviderSet,
}

impl CompiledPackageMetadata {
    #[must_use]
    pub const fn id(&self) -> &PackageId {
        &self.id
    }

    #[must_use]
    pub const fn version(&self) -> &PackageVersion {
        &self.version
    }

    #[must_use]
    pub const fn declared_capabilities(&self) -> CapabilitySet {
        self.declared_capabilities
    }

    #[must_use]
    pub const fn observed_capabilities(&self) -> CapabilitySet {
        self.observed_capabilities
    }
}

impl InstalledProviderSet {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        true
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        0
    }
}

impl PackageCompileRequestFingerprint {
    #[must_use]
    pub fn roots(&self) -> &[PackageId] {
        &self.roots
    }
}

impl PackageArtifactMetadata {
    pub(crate) fn ordinary(
        roots: &BTreeSet<PackageId>,
        packages: &[PackageCompilationInput],
        observed: &BTreeMap<PackageId, CapabilitySet>,
    ) -> Result<Self, String> {
        if roots.is_empty() {
            return Err("ordinary package compilation requires at least one root".to_owned());
        }
        let mut compiled = packages
            .iter()
            .map(|package| CompiledPackageMetadata {
                id: package.id.clone(),
                version: package.version.clone(),
                declared_capabilities: package.declared_capabilities,
                observed_capabilities: observed.get(&package.id).copied().unwrap_or_default(),
            })
            .collect::<Vec<_>>();
        compiled.sort_by(|left, right| left.id.cmp(&right.id));
        if compiled.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return Err("package compilation metadata contains a duplicate package ID".to_owned());
        }
        if roots
            .iter()
            .any(|root| compiled.binary_search_by(|item| item.id.cmp(root)).is_err())
        {
            return Err("package compilation root is absent from package metadata".to_owned());
        }
        Ok(Self {
            request: PackageCompileRequestFingerprint {
                roots: roots.iter().cloned().collect::<Vec<_>>().into_boxed_slice(),
            },
            packages: compiled.into_boxed_slice(),
            installed_providers: InstalledProviderSet::default(),
        })
    }

    #[must_use]
    pub const fn request(&self) -> &PackageCompileRequestFingerprint {
        &self.request
    }

    #[must_use]
    pub fn packages(&self) -> &[CompiledPackageMetadata] {
        &self.packages
    }

    #[must_use]
    pub const fn installed_providers(&self) -> &InstalledProviderSet {
        &self.installed_providers
    }
}
