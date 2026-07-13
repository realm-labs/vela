use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use vela_common::{CallableAsyncness, CapabilitySet, ShapeId, Span};
use vela_def::{MethodId, TypeId};
use vela_hir::provider::ProviderKey;
use vela_package::{PackageId, PackageVersion};

use crate::{LinkedProgram, MethodDispatchHandle, TypeHandle};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageCompilationInput {
    pub id: PackageId,
    pub version: PackageVersion,
    pub declared_capabilities: CapabilitySet,
    pub manifest_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderMethodCompilationInput {
    pub id: MethodId,
    pub name: String,
    pub asyncness: CallableAsyncness,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderCompilationInput {
    pub key: ProviderKey,
    pub provider_type: TypeId,
    pub provider_type_name: String,
    pub provider_shape: ShapeId,
    pub source: Span,
    pub methods: Box<[ProviderMethodCompilationInput]>,
    pub package_declared_capabilities: CapabilitySet,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledPackageMetadata {
    id: PackageId,
    version: PackageVersion,
    declared_capabilities: CapabilitySet,
    observed_capabilities: CapabilitySet,
    manifest_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageCompilationMetadata {
    request: PackageCompileRequestFingerprint,
    packages: Box<[CompiledPackageMetadata]>,
    providers: Box<[ProviderCompilationInput]>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InstalledProviderSet {
    providers: BTreeMap<ProviderKey, LinkedProviderEntry>,
    selection: ProviderSelectionFingerprint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkedProviderEntry {
    key: ProviderKey,
    provider_type: TypeHandle,
    provider_type_id: TypeId,
    receiver: ProviderReceiverPlan,
    methods: BTreeMap<MethodId, MethodDispatchHandle>,
    method_asyncness: BTreeMap<MethodId, CallableAsyncness>,
    package_declared_capabilities: CapabilitySet,
    source: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderReceiverPlan {
    FreshZeroField { shape: ShapeId },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProviderSelectionFingerprint {
    providers: Box<[ProviderKey]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageCompileRequestFingerprint {
    roots: Box<[PackageId]>,
    providers: ProviderSelectionFingerprint,
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

    #[must_use]
    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }
}

impl PackageCompilationMetadata {
    pub(crate) fn new(
        roots: &BTreeSet<PackageId>,
        packages: &[PackageCompilationInput],
        providers: &[ProviderCompilationInput],
        observed: &BTreeMap<PackageId, CapabilitySet>,
    ) -> Result<Self, String> {
        if roots.is_empty() {
            return Err("package compilation requires at least one root".to_owned());
        }
        let mut compiled = packages
            .iter()
            .map(|package| CompiledPackageMetadata {
                id: package.id.clone(),
                version: package.version.clone(),
                declared_capabilities: package.declared_capabilities,
                observed_capabilities: observed.get(&package.id).copied().unwrap_or_default(),
                manifest_path: package.manifest_path.clone(),
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
        let mut providers = providers.to_vec();
        providers.sort_by(|left, right| left.key.cmp(&right.key));
        if providers.windows(2).any(|pair| pair[0].key == pair[1].key) {
            return Err(
                "package compilation metadata contains a duplicate provider key".to_owned(),
            );
        }
        if providers.iter().any(|provider| {
            compiled
                .binary_search_by(|package| package.id.cmp(provider.key.package()))
                .is_err()
        }) {
            return Err("selected provider package is absent from package metadata".to_owned());
        }
        let selected = providers
            .iter()
            .map(|provider| provider.key.clone())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok(Self {
            request: PackageCompileRequestFingerprint {
                roots: roots.iter().cloned().collect::<Vec<_>>().into_boxed_slice(),
                providers: ProviderSelectionFingerprint {
                    providers: selected,
                },
            },
            packages: compiled.into_boxed_slice(),
            providers: providers.into_boxed_slice(),
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

    pub(crate) fn link(
        self,
        program: &mut LinkedProgram,
    ) -> Result<PackageArtifactMetadata, String> {
        let mut installed = BTreeMap::new();
        for provider in self.providers {
            let existing_type = program
                .types()
                .find_map(|(handle, ty)| (ty.id == provider.provider_type).then_some(handle));
            let provider_type = match existing_type {
                Some(handle) => handle,
                None => {
                    let name = program.intern_debug_name(provider.provider_type_name);
                    program.push_type(crate::LinkedType::new(provider.provider_type, name))
                }
            };
            let mut methods = BTreeMap::new();
            let mut method_asyncness = BTreeMap::new();
            for method in provider.methods {
                let dispatch = program
                    .script_method_dispatch(provider.provider_type, &method.name)
                    .ok_or_else(|| {
                        format!(
                            "selected provider `{}` has no linked method `{}`",
                            provider.key.provider(),
                            method.name
                        )
                    })?;
                if methods.insert(method.id, dispatch).is_some() {
                    return Err(format!(
                        "selected provider `{}` has duplicate method identity {:?}",
                        provider.key.provider(),
                        method.id
                    ));
                }
                method_asyncness.insert(method.id, method.asyncness);
            }
            let key = provider.key;
            let entry = LinkedProviderEntry {
                key: key.clone(),
                provider_type,
                provider_type_id: provider.provider_type,
                receiver: ProviderReceiverPlan::FreshZeroField {
                    shape: provider.provider_shape,
                },
                methods,
                method_asyncness,
                package_declared_capabilities: provider.package_declared_capabilities,
                source: provider.source,
            };
            installed.insert(key, entry);
        }
        let selection = self.request.providers.clone();
        Ok(PackageArtifactMetadata {
            request: self.request,
            packages: self.packages,
            installed_providers: InstalledProviderSet {
                providers: installed,
                selection,
            },
        })
    }
}

impl InstalledProviderSet {
    #[must_use]
    pub fn get(&self, key: &ProviderKey) -> Option<&LinkedProviderEntry> {
        self.providers.get(key)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ProviderKey, &LinkedProviderEntry)> {
        self.providers.iter()
    }

    #[must_use]
    pub const fn selection(&self) -> &ProviderSelectionFingerprint {
        &self.selection
    }
}

impl LinkedProviderEntry {
    #[must_use]
    pub const fn key(&self) -> &ProviderKey {
        &self.key
    }

    #[must_use]
    pub const fn provider_type(&self) -> TypeHandle {
        self.provider_type
    }

    #[must_use]
    pub const fn provider_type_id(&self) -> TypeId {
        self.provider_type_id
    }

    #[must_use]
    pub const fn receiver(&self) -> ProviderReceiverPlan {
        self.receiver
    }

    #[must_use]
    pub fn method(&self, id: MethodId) -> Option<MethodDispatchHandle> {
        self.methods.get(&id).copied()
    }

    #[must_use]
    pub fn method_asyncness(&self, id: MethodId) -> Option<CallableAsyncness> {
        self.method_asyncness.get(&id).copied()
    }

    pub fn method_ids(&self) -> impl Iterator<Item = MethodId> + '_ {
        self.methods.keys().copied()
    }

    #[must_use]
    pub const fn package_declared_capabilities(&self) -> CapabilitySet {
        self.package_declared_capabilities
    }

    #[must_use]
    pub const fn source(&self) -> Span {
        self.source
    }
}

impl ProviderSelectionFingerprint {
    #[must_use]
    pub fn providers(&self) -> &[ProviderKey] {
        &self.providers
    }
}

impl PackageCompileRequestFingerprint {
    #[must_use]
    pub fn roots(&self) -> &[PackageId] {
        &self.roots
    }

    #[must_use]
    pub const fn providers(&self) -> &ProviderSelectionFingerprint {
        &self.providers
    }
}

impl PackageArtifactMetadata {
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
