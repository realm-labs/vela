use std::collections::{BTreeMap, BTreeSet};

use vela_common::{HostTypeId, PrimitiveTag, Span};
use vela_def::{FieldId, TypeId};
use vela_reflect::access::{FunctionAccess, MethodAccess};
use vela_reflect::modules::{DeclOrigin, ModuleDesc};
use vela_reflect::registry::{FieldDesc, TypeDesc, TypeKind, TypeRegistry};
use vela_registry::TypeHintDef;

use crate::type_fact::TypeFact;

mod compile_view;
mod reflect_view;

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
    fn new(owner: impl Into<String>, name: impl Into<String>, access: &MethodAccess) -> Self {
        Self {
            owner: owner.into(),
            name: name.into(),
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
pub struct RegistryEffectFact {
    pub reads_host: bool,
    pub writes_host: bool,
    pub emits_events: bool,
    pub reads_time: bool,
    pub uses_random: bool,
    pub reads_io: bool,
    pub writes_io: bool,
    pub reads_reflection: bool,
    pub writes_reflection: bool,
    pub calls_reflection: bool,
}

impl RegistryEffectFact {
    #[must_use]
    pub const fn pure() -> Self {
        Self {
            reads_host: false,
            writes_host: false,
            emits_events: false,
            reads_time: false,
            uses_random: false,
            reads_io: false,
            writes_io: false,
            reads_reflection: false,
            writes_reflection: false,
            calls_reflection: false,
        }
    }

    #[must_use]
    pub const fn host_read() -> Self {
        Self {
            reads_host: true,
            writes_host: false,
            emits_events: false,
            reads_time: false,
            uses_random: false,
            reads_io: false,
            writes_io: false,
            reads_reflection: false,
            writes_reflection: false,
            calls_reflection: false,
        }
    }

    #[must_use]
    pub const fn host_write() -> Self {
        Self {
            reads_host: true,
            writes_host: true,
            emits_events: false,
            reads_time: false,
            uses_random: false,
            reads_io: false,
            writes_io: false,
            reads_reflection: false,
            writes_reflection: false,
            calls_reflection: false,
        }
    }

    #[must_use]
    pub const fn event_emit() -> Self {
        Self {
            reads_host: false,
            writes_host: false,
            emits_events: true,
            reads_time: false,
            uses_random: false,
            reads_io: false,
            writes_io: false,
            reads_reflection: false,
            writes_reflection: false,
            calls_reflection: false,
        }
    }

    #[must_use]
    pub fn denied_by(&self, allowed: &Self) -> Vec<&'static str> {
        self.effect_flags()
            .into_iter()
            .zip(allowed.effect_flags())
            .filter_map(|((name, required), (_, allowed))| (required && !allowed).then_some(name))
            .collect()
    }

    #[must_use]
    pub fn display_name(&self) -> String {
        let effects = self
            .effect_flags()
            .into_iter()
            .filter_map(|(name, enabled)| enabled.then_some(name))
            .collect::<Vec<_>>();
        if effects.is_empty() {
            "pure".to_owned()
        } else {
            effects.join(", ")
        }
    }

    fn effect_flags(&self) -> [(&'static str, bool); 10] {
        [
            ("reads_host", self.reads_host && !self.writes_host),
            ("writes_host", self.writes_host),
            ("emits_events", self.emits_events),
            ("reads_time", self.reads_time),
            ("uses_random", self.uses_random),
            ("reads_io", self.reads_io),
            ("writes_io", self.writes_io),
            ("reads_reflection", self.reads_reflection),
            ("writes_reflection", self.writes_reflection),
            ("calls_reflection", self.calls_reflection),
        ]
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RegistryFacts {
    types: BTreeMap<String, TypeFact>,
    type_targets: BTreeMap<String, RegistryTypeTargetFact>,
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
    trait_methods: BTreeMap<(String, String), TypeFact>,
    trait_method_signatures: BTreeMap<(String, String), CallableSignatureFact>,
    trait_method_docs: BTreeMap<(String, String), String>,
    modules: BTreeMap<String, RegistryModuleFact>,
    functions: BTreeMap<String, TypeFact>,
    function_signatures: BTreeMap<String, CallableSignatureFact>,
    function_origins: BTreeMap<String, DeclOrigin>,
    function_docs: BTreeMap<String, String>,
    function_access: BTreeMap<String, RegistryFunctionAccessFact>,
    index_capabilities: BTreeMap<String, RegistryIndexCapabilityFact>,
    method_effects: BTreeMap<(String, String), RegistryEffectFact>,
    method_access: BTreeMap<(String, String), RegistryMethodAccessFact>,
    trait_method_effects: BTreeMap<(String, String), RegistryEffectFact>,
    function_effects: BTreeMap<String, RegistryEffectFact>,
}

impl RegistryFacts {
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

fn type_desc_fact(desc: &TypeDesc) -> TypeFact {
    if let Some(tag) = PrimitiveTag::from_name(&desc.key.name) {
        return TypeFact::primitive(tag);
    }

    match desc.kind {
        TypeKind::Unit => TypeFact::UNIT,
        TypeKind::Bool => TypeFact::BOOL,
        TypeKind::I8 => TypeFact::I8,
        TypeKind::I16 => TypeFact::I16,
        TypeKind::I32 => TypeFact::I32,
        TypeKind::I64 => TypeFact::I64,
        TypeKind::U8 => TypeFact::U8,
        TypeKind::U16 => TypeFact::U16,
        TypeKind::U32 => TypeFact::U32,
        TypeKind::U64 => TypeFact::U64,
        TypeKind::F32 => TypeFact::F32,
        TypeKind::F64 => TypeFact::F64,
        TypeKind::Char => TypeFact::CHAR,
        TypeKind::String => TypeFact::STRING,
        TypeKind::Bytes => TypeFact::BYTES,
        TypeKind::Array => TypeFact::array(TypeFact::Any),
        TypeKind::Map => TypeFact::map(TypeFact::Any, TypeFact::Any),
        TypeKind::Set => TypeFact::set(TypeFact::Any),
        TypeKind::Range => TypeFact::Range,
        TypeKind::Function => TypeFact::function(Vec::new(), TypeFact::Any),
        TypeKind::Closure => TypeFact::Closure,
        TypeKind::Host => TypeFact::host(&desc.key.name),
        TypeKind::ScriptStruct => TypeFact::record(&desc.key.name),
        TypeKind::ScriptEnum => TypeFact::enum_type(&desc.key.name, None::<String>),
    }
}

fn registry_hint_fact(registry: &TypeRegistry, hint: &str) -> TypeFact {
    TypeHintDef::parse(hint).map_or_else(
        || raw_registry_hint_fact(registry, hint),
        |hint| type_hint_def_fact(registry, &hint),
    )
}

fn type_hint_def_fact(registry: &TypeRegistry, hint: &TypeHintDef) -> TypeFact {
    let path = hint.path.join("::");
    match (path.as_str(), hint.args.as_slice()) {
        ("()", []) => TypeFact::UNIT,
        ("()", elements) if elements.len() >= 2 => TypeFact::tuple(
            elements
                .iter()
                .map(|element| type_hint_def_fact(registry, element)),
        ),
        ("Any", []) => TypeFact::Any,
        ("String", []) => TypeFact::STRING,
        ("Bytes", []) => TypeFact::BYTES,
        ("Array", []) => TypeFact::array(TypeFact::Unknown),
        ("Array", [element]) => TypeFact::array(type_hint_def_fact(registry, element)),
        ("Map", []) => TypeFact::map(TypeFact::Unknown, TypeFact::Unknown),
        ("Map", [key, value]) => TypeFact::map(
            type_hint_def_fact(registry, key),
            type_hint_def_fact(registry, value),
        ),
        ("Set", []) => TypeFact::set(TypeFact::Unknown),
        ("Set", [element]) => TypeFact::set(type_hint_def_fact(registry, element)),
        ("Iterator", []) => TypeFact::iterator(TypeFact::Unknown),
        ("Iterator", [item]) => TypeFact::iterator(type_hint_def_fact(registry, item)),
        ("Function", []) => TypeFact::function(Vec::new(), TypeFact::Unknown),
        ("Closure", []) => TypeFact::Closure,
        ("Option", []) => TypeFact::option(TypeFact::Unknown),
        ("Option", [some]) => TypeFact::option(type_hint_def_fact(registry, some)),
        ("Result", []) => TypeFact::result(TypeFact::Unknown, TypeFact::Unknown),
        ("Result", [ok, err]) => TypeFact::result(
            type_hint_def_fact(registry, ok),
            type_hint_def_fact(registry, err),
        ),
        (name, []) => raw_registry_hint_fact(registry, name),
        _ => TypeFact::Unknown,
    }
}

fn raw_registry_hint_fact(registry: &TypeRegistry, hint: &str) -> TypeFact {
    if let Some(tag) = PrimitiveTag::from_name(hint) {
        return TypeFact::primitive(tag);
    }

    match hint {
        "Any" => TypeFact::Any,
        "String" => TypeFact::STRING,
        "Bytes" => TypeFact::BYTES,
        "Array" => TypeFact::array(TypeFact::Unknown),
        "Map" => TypeFact::map(TypeFact::Unknown, TypeFact::Unknown),
        "Set" => TypeFact::set(TypeFact::Unknown),
        "Iterator" => TypeFact::iterator(TypeFact::Unknown),
        "Function" => TypeFact::function(Vec::new(), TypeFact::Unknown),
        "Closure" => TypeFact::Closure,
        "Option" => TypeFact::option(TypeFact::Unknown),
        "Result" => TypeFact::result(TypeFact::Unknown, TypeFact::Unknown),
        name => registry
            .type_by_name(name)
            .map_or_else(|| trait_or_unknown(registry, name), type_desc_fact),
    }
}

fn trait_or_unknown(registry: &TypeRegistry, name: &str) -> TypeFact {
    if registry.trait_by_name(name).is_some()
        || registry
            .types()
            .flat_map(|type_desc| type_desc.traits.iter())
            .any(|trait_desc| trait_desc.name == name)
    {
        TypeFact::trait_type(name)
    } else {
        TypeFact::Unknown
    }
}

include!("registry/tests.rs");
