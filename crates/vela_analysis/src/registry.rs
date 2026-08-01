use std::collections::{BTreeMap, BTreeSet};

use vela_common::{
    CollectionViewCapabilities, HostConstructorBinding, HostTypeId, InteropTypeId,
    ReceiverCapabilities, ReceiverCapability, Span, StoragePolicy, TypeAbiFingerprint,
    TypeBindingRegistryChecksum,
};
use vela_def::{FieldId, FunctionId, TypeId};
use vela_reflect::access::{FunctionAccess, MethodAccess};
use vela_reflect::modules::{DeclOrigin, ModuleDesc};
use vela_reflect::registry::{FieldDesc, TypeRegistry};
pub use vela_registry::{ScopedResourceKindDef, ScopedResourceParentDef, ScopedResourceReturnDef};

use crate::type_fact::TypeFact;

mod compile_view;
mod effect;
mod hint_fact;
mod reflect_view;

pub use effect::RegistryEffectFact;
use hint_fact::{registry_hint_fact, type_desc_fact};

pub use crate::callable::{
    CallableParameterFact, CallableParameterRequirementFact, CallableSignatureFact,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryMemberFact {
    pub owner: String,
    pub name: String,
    pub fact: TypeFact,
}

impl RegistryMemberFact {
    fn new(owner: impl Into<String>, name: impl Into<String>, fact: TypeFact) -> Self {
        Self {
            owner: owner.into(),
            name: name.into(),
            fact,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryFieldAccessFact {
    pub owner: String,
    pub name: String,
    pub readable: bool,
    pub writable: bool,
    pub reflect_readable: bool,
    pub reflect_writable: bool,
    pub required_permissions: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryTypeTargetFact {
    pub name: String,
    pub semantic: TypeId,
    pub host_runtime: Option<HostTypeId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryTypeBindingFact {
    pub id: InteropTypeId,
    pub storage: StoragePolicy,
    pub capabilities: ReceiverCapabilities,
    pub collection_views: Option<CollectionViewCapabilities>,
    pub constructor_ids: Vec<FunctionId>,
    pub host_constructors: Vec<HostConstructorBinding>,
    pub abi_fingerprint: TypeAbiFingerprint,
}

impl RegistryTypeTargetFact {
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        semantic: TypeId,
        host_runtime: Option<HostTypeId>,
    ) -> Self {
        Self {
            name: name.into(),
            semantic,
            host_runtime,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryFieldTargetFact {
    pub owner: TypeId,
    pub owner_name: String,
    pub name: String,
    pub semantic: FieldId,
    pub host_runtime: Option<FieldId>,
    pub variant_field: bool,
    pub declaration_order: u32,
    pub has_default: bool,
    pub access: RegistryFieldAccessFact,
}

impl RegistryFieldTargetFact {
    #[must_use]
    pub fn new(
        owner: TypeId,
        owner_name: impl Into<String>,
        name: impl Into<String>,
        semantic: FieldId,
        host_runtime: Option<FieldId>,
        variant_field: bool,
        access: RegistryFieldAccessFact,
    ) -> Self {
        Self {
            owner,
            owner_name: owner_name.into(),
            name: name.into(),
            semantic,
            host_runtime,
            variant_field,
            declaration_order: 0,
            has_default: false,
            access,
        }
    }

    #[must_use]
    pub const fn declaration_order(mut self, declaration_order: u32) -> Self {
        self.declaration_order = declaration_order;
        self
    }

    #[must_use]
    pub const fn defaulted(mut self, has_default: bool) -> Self {
        self.has_default = has_default;
        self
    }
}

impl RegistryFieldAccessFact {
    fn new(owner: impl Into<String>, name: impl Into<String>, field: &FieldDesc) -> Self {
        Self {
            owner: owner.into(),
            name: name.into(),
            readable: field.access.readable,
            writable: field.access.writable,
            reflect_readable: field.access.reflect_readable,
            reflect_writable: field.access.reflect_writable,
            required_permissions: field.access.required_permissions().to_vec(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryMethodAccessFact {
    pub owner: String,
    pub name: String,
    pub receiver: ReceiverCapability,
    pub public: bool,
    pub reflect_callable: bool,
    pub required_permissions: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryFunctionAccessFact {
    pub name: String,
    pub public: bool,
    pub reflect_visible: bool,
    pub reflect_callable: bool,
}

impl RegistryFunctionAccessFact {
    fn new(name: impl Into<String>, access: &FunctionAccess) -> Self {
        Self {
            name: name.into(),
            public: access.public,
            reflect_visible: access.reflect_visible,
            reflect_callable: access.reflect_callable,
        }
    }
}

impl RegistryMethodAccessFact {
    fn new(
        owner: impl Into<String>,
        name: impl Into<String>,
        receiver: ReceiverCapability,
        access: &MethodAccess,
    ) -> Self {
        Self {
            owner: owner.into(),
            name: name.into(),
            receiver,
            public: access.public,
            reflect_callable: access.reflect_callable,
            required_permissions: access.required_permissions().to_vec(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryIndexCapabilityFact {
    pub owner: String,
    pub readable: bool,
    pub writable: bool,
    pub addable: bool,
    pub removable: bool,
    pub key: TypeFact,
    pub value: TypeFact,
}

impl RegistryIndexCapabilityFact {
    fn new(
        owner: impl Into<String>,
        capability: &vela_reflect::registry::HostIndexCapability,
        registry: &TypeRegistry,
    ) -> Self {
        Self {
            owner: owner.into(),
            readable: capability.readable,
            writable: capability.writable,
            addable: capability.addable,
            removable: capability.removable,
            key: capability
                .key_type
                .as_deref()
                .map_or(TypeFact::Unknown, |hint| registry_hint_fact(registry, hint)),
            value: capability
                .value_type
                .as_deref()
                .map_or(TypeFact::Unknown, |hint| registry_hint_fact(registry, hint)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryFunctionFact {
    pub name: String,
    pub fact: TypeFact,
}

impl RegistryFunctionFact {
    fn new(name: impl Into<String>, fact: TypeFact) -> Self {
        Self {
            name: name.into(),
            fact,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryModuleFact {
    pub name: String,
    pub fact: TypeFact,
    pub docs: Option<String>,
    pub source_span: Option<Span>,
}

impl RegistryModuleFact {
    fn new(desc: &ModuleDesc) -> Self {
        Self {
            name: desc.name.clone(),
            fact: TypeFact::module(&desc.name),
            docs: desc.docs.clone(),
            source_span: desc.source_span,
        }
    }

    #[must_use]
    pub fn from_parts(
        name: impl Into<String>,
        fact: TypeFact,
        docs: Option<String>,
        source_span: Option<Span>,
    ) -> Self {
        Self {
            name: name.into(),
            fact,
            docs,
            source_span,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RegistryFacts {
    execution_effect_ceiling: Option<RegistryEffectFact>,
    types: BTreeMap<String, TypeFact>,
    type_targets: BTreeMap<String, RegistryTypeTargetFact>,
    type_bindings: BTreeMap<String, RegistryTypeBindingFact>,
    type_binding_checksum: Option<TypeBindingRegistryChecksum>,
    type_docs: BTreeMap<String, String>,
    traits: BTreeMap<String, TypeFact>,
    trait_docs: BTreeMap<String, String>,
    fields: BTreeMap<(String, String), TypeFact>,
    fields_by_short_owner: BTreeMap<String, BTreeSet<(String, String)>>,
    field_docs: BTreeMap<(String, String), String>,
    field_access: BTreeMap<(String, String), RegistryFieldAccessFact>,
    field_targets: BTreeMap<(String, String), RegistryFieldTargetFact>,
    variants: BTreeMap<(String, String), TypeFact>,
    variants_by_short_owner: BTreeMap<String, BTreeSet<(String, String)>>,
    variant_docs: BTreeMap<(String, String), String>,
    methods: BTreeMap<(String, String), TypeFact>,
    method_signatures: BTreeMap<(String, String), CallableSignatureFact>,
    method_docs: BTreeMap<(String, String), String>,
    method_scoped_resources: BTreeMap<(String, String), ScopedResourceReturnDef>,
    trait_methods: BTreeMap<(String, String), TypeFact>,
    trait_method_signatures: BTreeMap<(String, String), CallableSignatureFact>,
    trait_method_docs: BTreeMap<(String, String), String>,
    modules: BTreeMap<String, RegistryModuleFact>,
    functions: BTreeMap<String, TypeFact>,
    function_signatures: BTreeMap<String, CallableSignatureFact>,
    function_origins: BTreeMap<String, DeclOrigin>,
    function_docs: BTreeMap<String, String>,
    function_scoped_resources: BTreeMap<String, ScopedResourceReturnDef>,
    function_access: BTreeMap<String, RegistryFunctionAccessFact>,
    index_capabilities: BTreeMap<String, RegistryIndexCapabilityFact>,
    method_effects: BTreeMap<(String, String), RegistryEffectFact>,
    method_access: BTreeMap<(String, String), RegistryMethodAccessFact>,
    trait_method_effects: BTreeMap<(String, String), RegistryEffectFact>,
    function_effects: BTreeMap<String, RegistryEffectFact>,
}

impl RegistryFacts {
    #[must_use]
    pub const fn execution_effect_ceiling(&self) -> Option<&RegistryEffectFact> {
        self.execution_effect_ceiling.as_ref()
    }

    pub fn set_execution_effect_ceiling(&mut self, ceiling: RegistryEffectFact) {
        self.execution_effect_ceiling = Some(ceiling);
    }

    #[must_use]
    pub fn type_fact(&self, name: &str) -> Option<&TypeFact> {
        self.types.get(name)
    }

    pub fn types(&self) -> impl Iterator<Item = (&str, &TypeFact)> {
        self.types.iter().map(|(name, fact)| (name.as_str(), fact))
    }

    #[must_use]
    pub fn type_target_fact(&self, name: &str) -> Option<&RegistryTypeTargetFact> {
        self.type_targets.get(name)
    }

    #[must_use]
    pub fn type_binding_fact(&self, name: &str) -> Option<&RegistryTypeBindingFact> {
        self.type_bindings.get(name)
    }

    pub fn type_bindings(&self) -> impl Iterator<Item = (&str, &RegistryTypeBindingFact)> {
        self.type_bindings
            .iter()
            .map(|(name, binding)| (name.as_str(), binding))
    }

    #[must_use]
    pub const fn type_binding_checksum(&self) -> Option<TypeBindingRegistryChecksum> {
        self.type_binding_checksum
    }

    #[must_use]
    pub fn type_docs(&self, name: &str) -> Option<&str> {
        self.type_docs.get(name).map(String::as_str)
    }

    #[must_use]
    pub fn trait_fact(&self, name: &str) -> Option<&TypeFact> {
        self.traits.get(name)
    }

    pub fn traits(&self) -> impl Iterator<Item = (&str, &TypeFact)> {
        self.traits.iter().map(|(name, fact)| (name.as_str(), fact))
    }

    #[must_use]
    pub fn trait_docs(&self, name: &str) -> Option<&str> {
        self.trait_docs.get(name).map(String::as_str)
    }

    #[must_use]
    pub fn field_fact(&self, owner: &str, field: &str) -> Option<&TypeFact> {
        self.fields.get(&(owner.to_owned(), field.to_owned()))
    }

    #[must_use]
    pub fn field_docs(&self, owner: &str, field: &str) -> Option<&str> {
        self.field_docs
            .get(&(owner.to_owned(), field.to_owned()))
            .map(String::as_str)
    }

    #[must_use]
    pub fn field_access_fact(&self, owner: &str, field: &str) -> Option<&RegistryFieldAccessFact> {
        self.field_access.get(&(owner.to_owned(), field.to_owned()))
    }

    #[must_use]
    pub fn field_target_fact(&self, owner: &str, field: &str) -> Option<&RegistryFieldTargetFact> {
        self.field_targets
            .get(&(owner.to_owned(), field.to_owned()))
    }

    /// Resolves a host-visible field with the same direct-then-unique-variant
    /// policy as the authoritative definition registry.
    ///
    /// A plain field always wins. If no plain field exists, a field exposed by
    /// exactly one variant is available through the host target path. The
    /// fallback deliberately rejects two variants exposing the same name.
    #[must_use]
    pub fn host_field_target_fact(
        &self,
        owner: &str,
        field: &str,
    ) -> Option<&RegistryFieldTargetFact> {
        let semantic_owner = self.type_target_fact(owner)?.semantic;
        let matching = |target: &&RegistryFieldTargetFact| {
            target.owner == semantic_owner && target.name == field
        };
        let mut direct = self
            .field_targets
            .values()
            .filter(matching)
            .filter(|target| !target.variant_field);
        if let Some(target) = direct.next() {
            return direct.next().is_none().then_some(target);
        }
        let mut variants = self
            .field_targets
            .values()
            .filter(matching)
            .filter(|target| target.variant_field);
        let target = variants.next()?;
        variants.next().is_none().then_some(target)
    }

    #[must_use]
    pub fn host_field_fact(&self, owner: &str, field: &str) -> Option<&TypeFact> {
        let target = self.host_field_target_fact(owner, field)?;
        self.fields
            .get(&(target.owner_name.clone(), target.name.clone()))
    }

    #[must_use]
    pub fn field_targets_for_owner_or_short_name(
        &self,
        owner: &str,
    ) -> Vec<RegistryFieldTargetFact> {
        let exact = self
            .field_targets
            .keys()
            .filter(|(field_owner, _)| field_owner == owner)
            .cloned()
            .collect::<Vec<_>>();
        let keys = if exact.is_empty() {
            let short_owner = owner.rsplit("::").next().unwrap_or(owner);
            let indexed = self
                .fields_by_short_owner
                .get(short_owner)
                .cloned()
                .unwrap_or_default();
            let qualified_owners = indexed
                .iter()
                .map(|(field_owner, _)| field_owner)
                .collect::<BTreeSet<_>>();
            if qualified_owners.len() == 1 {
                indexed.into_iter().collect()
            } else {
                Vec::new()
            }
        } else {
            exact
        };
        let mut targets = keys
            .into_iter()
            .filter_map(|key| self.field_targets.get(&key).cloned())
            .collect::<Vec<_>>();
        targets.sort_by(|lhs, rhs| {
            (lhs.declaration_order, &lhs.owner_name, &lhs.name).cmp(&(
                rhs.declaration_order,
                &rhs.owner_name,
                &rhs.name,
            ))
        });
        targets
    }

    pub fn fields(&self) -> impl Iterator<Item = RegistryMemberFact> + '_ {
        self.fields
            .iter()
            .map(|((owner, name), fact)| RegistryMemberFact::new(owner, name, fact.clone()))
    }

    #[must_use]
    pub fn fields_for_owner(&self, owner: &str) -> Vec<RegistryMemberFact> {
        self.fields
            .range((owner.to_owned(), String::new())..)
            .take_while(|((field_owner, _), _)| field_owner == owner)
            .map(|((owner, name), fact)| RegistryMemberFact::new(owner, name, fact.clone()))
            .collect()
    }

    pub(crate) fn fields_for_exact_or_unique_short_name(
        &self,
        owner: &str,
    ) -> Option<Vec<RegistryMemberFact>> {
        let exact = self.fields_for_owner(owner);
        if !exact.is_empty() {
            return Some(exact);
        }
        let short_owner = owner.rsplit("::").next().unwrap_or(owner);
        let indexed = self
            .fields_by_short_owner
            .get(short_owner)
            .cloned()
            .unwrap_or_default();
        let qualified_owners = indexed
            .iter()
            .map(|(field_owner, _)| field_owner)
            .collect::<BTreeSet<_>>();
        if qualified_owners.len() > 1 {
            return None;
        }
        let resolved_owner = qualified_owners.into_iter().next();
        Some(resolved_owner.map_or_else(Vec::new, |owner| self.fields_for_owner(owner)))
    }

    #[must_use]
    pub fn fields_for_owner_or_short_name(&self, owner: &str) -> Vec<RegistryMemberFact> {
        let mut fields = self.fields_for_owner(owner);
        if owner.contains("::") {
            if let Some(short_owner) = owner.rsplit("::").next() {
                fields.extend(self.fields_for_owner(short_owner));
            }
        } else if let Some(keys) = self.fields_by_short_owner.get(owner) {
            fields.extend(keys.iter().filter_map(|key| {
                let fact = self.fields.get(key)?;
                Some(RegistryMemberFact::new(&key.0, &key.1, fact.clone()))
            }));
        }
        fields
    }

    #[must_use]
    pub fn index_capability_fact(&self, owner: &str) -> Option<&RegistryIndexCapabilityFact> {
        self.index_capabilities.get(owner)
    }

    #[must_use]
    pub fn variant_fact(&self, owner: &str, variant: &str) -> Option<&TypeFact> {
        self.variants.get(&(owner.to_owned(), variant.to_owned()))
    }

    #[must_use]
    pub fn variant_for_owner_or_unique_short_name(
        &self,
        owner: &str,
        variant: &str,
    ) -> Option<RegistryMemberFact> {
        if let Some(fact) = self.variant_fact(owner, variant) {
            return Some(RegistryMemberFact::new(owner, variant, fact.clone()));
        }
        let semantic_owner = self.type_target_fact(owner).map(|target| target.semantic);
        let mut keys = semantic_owner.map_or_else(BTreeSet::new, |semantic_owner| {
            self.type_targets
                .iter()
                .filter(|(name, target)| {
                    target.semantic == semantic_owner && self.variant_fact(name, variant).is_some()
                })
                .map(|(name, _)| (name.clone(), variant.to_owned()))
                .collect()
        });
        if keys.is_empty() {
            let short_owner = owner.rsplit("::").next().unwrap_or(owner);
            keys.extend(
                self.variants_by_short_owner
                    .get(short_owner)
                    .into_iter()
                    .flatten()
                    .filter(|(_, name)| name == variant)
                    .cloned(),
            );
        }
        (keys.len() == 1).then(|| {
            let (owner, name) = keys.into_iter().next().expect("one variant key");
            let fact = self
                .variants
                .get(&(owner.clone(), name.clone()))
                .expect("indexed variant fact")
                .clone();
            RegistryMemberFact::new(owner, name, fact)
        })
    }

    pub fn variant_names(&self, owner: &str) -> Vec<String> {
        self.variants
            .keys()
            .filter_map(|(variant_owner, variant)| {
                if variant_owner == owner {
                    Some(variant.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    #[must_use]
    pub fn variant_docs(&self, owner: &str, variant: &str) -> Option<&str> {
        self.variant_docs
            .get(&(owner.to_owned(), variant.to_owned()))
            .map(String::as_str)
    }

    pub fn variants(&self) -> impl Iterator<Item = RegistryMemberFact> + '_ {
        self.variants
            .iter()
            .map(|((owner, name), fact)| RegistryMemberFact::new(owner, name, fact.clone()))
    }

    #[must_use]
    pub fn variants_for_owner(&self, owner: &str) -> Vec<RegistryMemberFact> {
        self.variants
            .range((owner.to_owned(), String::new())..)
            .take_while(|((variant_owner, _), _)| variant_owner == owner)
            .map(|((owner, name), fact)| RegistryMemberFact::new(owner, name, fact.clone()))
            .collect()
    }

    #[must_use]
    pub fn variants_for_owner_or_short_name(&self, owner: &str) -> Vec<RegistryMemberFact> {
        let mut variants = self.variants_for_owner(owner);
        if owner.contains("::") {
            if let Some(short_owner) = owner.rsplit("::").next() {
                variants.extend(self.variants_for_owner(short_owner));
            }
        } else if let Some(keys) = self.variants_by_short_owner.get(owner) {
            variants.extend(keys.iter().filter_map(|key| {
                let fact = self.variants.get(key)?;
                Some(RegistryMemberFact::new(&key.0, &key.1, fact.clone()))
            }));
        }
        variants
    }

    #[must_use]
    pub fn method_fact(&self, owner: &str, method: &str) -> Option<&TypeFact> {
        self.methods.get(&(owner.to_owned(), method.to_owned()))
    }

    #[must_use]
    pub fn method_signature_fact(
        &self,
        owner: &str,
        method: &str,
    ) -> Option<&CallableSignatureFact> {
        self.method_signatures
            .get(&(owner.to_owned(), method.to_owned()))
    }

    #[must_use]
    pub fn method_docs(&self, owner: &str, method: &str) -> Option<&str> {
        self.method_docs
            .get(&(owner.to_owned(), method.to_owned()))
            .map(String::as_str)
    }

    #[must_use]
    pub fn method_scoped_resource(
        &self,
        owner: &str,
        method: &str,
    ) -> Option<ScopedResourceReturnDef> {
        self.method_scoped_resources
            .get(&(owner.to_owned(), method.to_owned()))
            .copied()
    }

    #[must_use]
    pub fn method_effect_fact(&self, owner: &str, method: &str) -> Option<&RegistryEffectFact> {
        self.method_effects
            .get(&(owner.to_owned(), method.to_owned()))
    }

    #[must_use]
    pub fn method_access_fact(
        &self,
        owner: &str,
        method: &str,
    ) -> Option<&RegistryMethodAccessFact> {
        self.method_access
            .get(&(owner.to_owned(), method.to_owned()))
    }

    pub fn methods(&self) -> impl Iterator<Item = RegistryMemberFact> + '_ {
        self.methods
            .iter()
            .map(|((owner, name), fact)| RegistryMemberFact::new(owner, name, fact.clone()))
    }

    #[must_use]
    pub fn trait_method_fact(&self, trait_name: &str, method: &str) -> Option<&TypeFact> {
        self.trait_methods
            .get(&(trait_name.to_owned(), method.to_owned()))
    }

    #[must_use]
    pub fn trait_method_signature_fact(
        &self,
        trait_name: &str,
        method: &str,
    ) -> Option<&CallableSignatureFact> {
        self.trait_method_signatures
            .get(&(trait_name.to_owned(), method.to_owned()))
    }

    #[must_use]
    pub fn trait_method_docs(&self, trait_name: &str, method: &str) -> Option<&str> {
        self.trait_method_docs
            .get(&(trait_name.to_owned(), method.to_owned()))
            .map(String::as_str)
    }

    #[must_use]
    pub fn trait_method_effect_fact(
        &self,
        trait_name: &str,
        method: &str,
    ) -> Option<&RegistryEffectFact> {
        self.trait_method_effects
            .get(&(trait_name.to_owned(), method.to_owned()))
    }

    pub fn trait_methods(&self) -> impl Iterator<Item = RegistryMemberFact> + '_ {
        self.trait_methods
            .iter()
            .map(|((owner, name), fact)| RegistryMemberFact::new(owner, name, fact.clone()))
    }

    #[must_use]
    pub fn function_fact(&self, name: &str) -> Option<&TypeFact> {
        self.functions.get(name)
    }

    #[must_use]
    pub fn function_signature_fact(&self, name: &str) -> Option<&CallableSignatureFact> {
        self.function_signatures.get(name)
    }

    #[must_use]
    pub fn function_scoped_resource(&self, name: &str) -> Option<ScopedResourceReturnDef> {
        self.function_scoped_resources.get(name).copied()
    }

    #[must_use]
    pub fn function_origin(&self, name: &str) -> Option<DeclOrigin> {
        self.function_origins.get(name).copied()
    }

    #[must_use]
    pub fn module_fact(&self, name: &str) -> Option<&TypeFact> {
        self.modules.get(name).map(|module| &module.fact)
    }

    #[must_use]
    pub fn module_docs(&self, name: &str) -> Option<&str> {
        self.modules
            .get(name)
            .and_then(|module| module.docs.as_deref())
    }

    #[must_use]
    pub fn module_source_span(&self, name: &str) -> Option<Span> {
        self.modules.get(name).and_then(|module| module.source_span)
    }

    pub fn modules(&self) -> impl Iterator<Item = RegistryModuleFact> + '_ {
        self.modules.values().cloned()
    }

    #[must_use]
    pub fn function_docs(&self, name: &str) -> Option<&str> {
        self.function_docs.get(name).map(String::as_str)
    }

    #[must_use]
    pub fn function_effect_fact(&self, name: &str) -> Option<&RegistryEffectFact> {
        self.function_effects.get(name)
    }

    #[must_use]
    pub fn function_access_fact(&self, name: &str) -> Option<&RegistryFunctionAccessFact> {
        self.function_access.get(name)
    }

    pub fn functions(&self) -> impl Iterator<Item = RegistryFunctionFact> + '_ {
        self.functions
            .iter()
            .map(|(name, fact)| RegistryFunctionFact::new(name, fact.clone()))
    }

    pub fn field_accesses(&self) -> impl Iterator<Item = RegistryFieldAccessFact> + '_ {
        self.field_access.values().cloned()
    }

    pub fn method_accesses(&self) -> impl Iterator<Item = RegistryMethodAccessFact> + '_ {
        self.method_access.values().cloned()
    }

    pub fn function_accesses(&self) -> impl Iterator<Item = RegistryFunctionAccessFact> + '_ {
        self.function_access.values().cloned()
    }

    pub fn index_capabilities(&self) -> impl Iterator<Item = RegistryIndexCapabilityFact> + '_ {
        self.index_capabilities.values().cloned()
    }

    pub fn method_effects(
        &self,
    ) -> impl Iterator<Item = (RegistryMemberFact, RegistryEffectFact)> + '_ {
        self.method_effects.iter().map(|((owner, name), effect)| {
            (
                RegistryMemberFact::new(owner, name, TypeFact::Unknown),
                effect.clone(),
            )
        })
    }

    pub fn trait_method_effects(
        &self,
    ) -> impl Iterator<Item = (RegistryMemberFact, RegistryEffectFact)> + '_ {
        self.trait_method_effects
            .iter()
            .map(|((owner, name), effect)| {
                (
                    RegistryMemberFact::new(owner, name, TypeFact::Unknown),
                    effect.clone(),
                )
            })
    }

    pub fn function_effects(&self) -> impl Iterator<Item = (&str, &RegistryEffectFact)> {
        self.function_effects
            .iter()
            .map(|(name, effect)| (name.as_str(), effect))
    }

    pub fn insert_type(&mut self, name: impl Into<String>, fact: TypeFact) {
        self.types.insert(name.into(), fact);
    }

    pub fn insert_type_target(&mut self, target: RegistryTypeTargetFact) {
        self.type_targets.insert(target.name.clone(), target);
    }

    pub fn insert_type_binding(
        &mut self,
        name: impl Into<String>,
        binding: RegistryTypeBindingFact,
    ) {
        self.type_bindings.insert(name.into(), binding);
    }

    pub fn set_type_binding_checksum(&mut self, checksum: TypeBindingRegistryChecksum) {
        self.type_binding_checksum = Some(checksum);
    }

    pub fn insert_type_docs(&mut self, name: impl Into<String>, docs: impl Into<String>) {
        self.type_docs.insert(name.into(), docs.into());
    }

    pub fn insert_trait(&mut self, name: impl Into<String>, fact: TypeFact) {
        self.traits.insert(name.into(), fact);
    }

    pub fn insert_trait_docs(&mut self, name: impl Into<String>, docs: impl Into<String>) {
        self.trait_docs.insert(name.into(), docs.into());
    }

    pub fn insert_field(
        &mut self,
        owner: impl Into<String>,
        name: impl Into<String>,
        fact: TypeFact,
    ) {
        let owner = owner.into();
        let name = name.into();
        self.index_field_owner(&owner, &name);
        self.fields.insert((owner, name), fact);
    }

    pub fn insert_field_docs(
        &mut self,
        owner: impl Into<String>,
        name: impl Into<String>,
        docs: impl Into<String>,
    ) {
        self.field_docs
            .insert((owner.into(), name.into()), docs.into());
    }

    pub fn insert_field_access(&mut self, access: RegistryFieldAccessFact) {
        self.field_access
            .insert((access.owner.clone(), access.name.clone()), access);
    }

    pub fn insert_field_target(&mut self, target: RegistryFieldTargetFact) {
        self.field_targets
            .insert((target.owner_name.clone(), target.name.clone()), target);
    }

    pub fn insert_variant(
        &mut self,
        owner: impl Into<String>,
        name: impl Into<String>,
        fact: TypeFact,
    ) {
        let owner = owner.into();
        let name = name.into();
        self.index_variant_owner(&owner, &name);
        self.variants.insert((owner, name), fact);
    }

    pub fn insert_variant_docs(
        &mut self,
        owner: impl Into<String>,
        name: impl Into<String>,
        docs: impl Into<String>,
    ) {
        self.variant_docs
            .insert((owner.into(), name.into()), docs.into());
    }

    pub fn insert_method(
        &mut self,
        owner: impl Into<String>,
        name: impl Into<String>,
        fact: TypeFact,
    ) {
        self.methods.insert((owner.into(), name.into()), fact);
    }

    pub fn insert_method_signature(
        &mut self,
        owner: impl Into<String>,
        name: impl Into<String>,
        signature: CallableSignatureFact,
    ) {
        self.method_signatures
            .insert((owner.into(), name.into()), signature);
    }

    pub fn insert_method_docs(
        &mut self,
        owner: impl Into<String>,
        name: impl Into<String>,
        docs: impl Into<String>,
    ) {
        self.method_docs
            .insert((owner.into(), name.into()), docs.into());
    }

    pub fn insert_method_scoped_resource(
        &mut self,
        owner: impl Into<String>,
        name: impl Into<String>,
        resource: ScopedResourceReturnDef,
    ) {
        self.method_scoped_resources
            .insert((owner.into(), name.into()), resource);
    }

    pub fn insert_method_effect(
        &mut self,
        owner: impl Into<String>,
        name: impl Into<String>,
        effect: RegistryEffectFact,
    ) {
        self.method_effects
            .insert((owner.into(), name.into()), effect);
    }

    pub fn insert_method_access(&mut self, access: RegistryMethodAccessFact) {
        self.method_access
            .insert((access.owner.clone(), access.name.clone()), access);
    }

    pub fn insert_trait_method(
        &mut self,
        owner: impl Into<String>,
        name: impl Into<String>,
        fact: TypeFact,
    ) {
        self.trait_methods.insert((owner.into(), name.into()), fact);
    }

    pub fn insert_trait_method_signature(
        &mut self,
        owner: impl Into<String>,
        name: impl Into<String>,
        signature: CallableSignatureFact,
    ) {
        self.trait_method_signatures
            .insert((owner.into(), name.into()), signature);
    }

    pub fn insert_trait_method_docs(
        &mut self,
        owner: impl Into<String>,
        name: impl Into<String>,
        docs: impl Into<String>,
    ) {
        self.trait_method_docs
            .insert((owner.into(), name.into()), docs.into());
    }

    pub fn insert_trait_method_effect(
        &mut self,
        owner: impl Into<String>,
        name: impl Into<String>,
        effect: RegistryEffectFact,
    ) {
        self.trait_method_effects
            .insert((owner.into(), name.into()), effect);
    }

    pub fn insert_function(&mut self, name: impl Into<String>, fact: TypeFact) {
        self.functions.insert(name.into(), fact);
    }

    pub fn insert_function_signature(
        &mut self,
        name: impl Into<String>,
        signature: CallableSignatureFact,
    ) {
        self.function_signatures.insert(name.into(), signature);
    }

    pub fn insert_function_origin(&mut self, name: impl Into<String>, origin: DeclOrigin) {
        self.function_origins.insert(name.into(), origin);
    }

    pub fn insert_module(&mut self, module: RegistryModuleFact) {
        self.modules.insert(module.name.clone(), module);
    }

    pub fn insert_function_docs(&mut self, name: impl Into<String>, docs: impl Into<String>) {
        self.function_docs.insert(name.into(), docs.into());
    }

    pub fn insert_function_scoped_resource(
        &mut self,
        name: impl Into<String>,
        resource: ScopedResourceReturnDef,
    ) {
        self.function_scoped_resources.insert(name.into(), resource);
    }

    pub fn insert_function_effect(&mut self, name: impl Into<String>, effect: RegistryEffectFact) {
        self.function_effects.insert(name.into(), effect);
    }

    pub fn insert_function_access(&mut self, access: RegistryFunctionAccessFact) {
        self.function_access.insert(access.name.clone(), access);
    }

    pub fn insert_index_capability(&mut self, capability: RegistryIndexCapabilityFact) {
        self.index_capabilities
            .insert(capability.owner.clone(), capability);
    }

    fn rebuild_owner_indexes(&mut self) {
        self.fields_by_short_owner.clear();
        for (owner, name) in self.fields.keys().cloned().collect::<Vec<_>>() {
            self.index_field_owner(&owner, &name);
        }
        self.variants_by_short_owner.clear();
        for (owner, name) in self.variants.keys().cloned().collect::<Vec<_>>() {
            self.index_variant_owner(&owner, &name);
        }
    }

    fn index_field_owner(&mut self, owner: &str, name: &str) {
        if let Some(short_owner) = owner.rsplit("::").next()
            && short_owner != owner
        {
            self.fields_by_short_owner
                .entry(short_owner.to_owned())
                .or_default()
                .insert((owner.to_owned(), name.to_owned()));
        }
    }

    fn index_variant_owner(&mut self, owner: &str, name: &str) {
        if let Some(short_owner) = owner.rsplit("::").next()
            && short_owner != owner
        {
            self.variants_by_short_owner
                .entry(short_owner.to_owned())
                .or_default()
                .insert((owner.to_owned(), name.to_owned()));
        }
    }
}

#[cfg(test)]
mod registry_tests;
