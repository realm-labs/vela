use std::collections::{BTreeMap, BTreeSet};

use crate::access::{FieldAccess, MethodAccess, MethodEffectSet};
use crate::modules::{FunctionDesc, ModuleDesc};
use crate::{
    ReflectError, ReflectErrorKind, ReflectResult,
    candidates::{candidate_names, ranked_candidates},
};
use vela_common::{
    FieldId, FunctionId, HostMethodId, HostTypeId, MethodId, Span, TraitId, TypeId, VariantId,
};
use vela_host::HostRef;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TypeKey {
    pub id: TypeId,
    pub name: String,
}

impl TypeKey {
    #[must_use]
    pub fn new(id: TypeId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct SchemaHash(u64);

impl SchemaHash {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeKind {
    Host,
    ScriptStruct,
    ScriptEnum,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AttrMap {
    attrs: BTreeMap<String, String>,
}

impl AttrMap {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.insert(name, value);
        self
    }

    pub fn insert(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.attrs.insert(name.into(), value.into());
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.attrs.get(name).map(String::as_str)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.attrs
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.attrs.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeDesc {
    pub key: TypeKey,
    pub kind: TypeKind,
    pub schema_hash: Option<SchemaHash>,
    pub host_type_id: Option<HostTypeId>,
    pub fields: Vec<FieldDesc>,
    pub methods: Vec<MethodDesc>,
    pub traits: Vec<TraitDesc>,
    pub variants: Vec<VariantDesc>,
    pub docs: Option<String>,
    pub attrs: AttrMap,
    pub source_span: Option<Span>,
}

impl TypeDesc {
    #[must_use]
    pub fn new(key: TypeKey) -> Self {
        Self {
            key,
            kind: TypeKind::Host,
            schema_hash: None,
            host_type_id: None,
            fields: Vec::new(),
            methods: Vec::new(),
            traits: Vec::new(),
            variants: Vec::new(),
            docs: None,
            attrs: AttrMap::new(),
            source_span: None,
        }
    }

    #[must_use]
    pub fn kind(mut self, kind: TypeKind) -> Self {
        self.kind = kind;
        self
    }

    #[must_use]
    pub fn schema_hash(mut self, schema_hash: SchemaHash) -> Self {
        self.schema_hash = Some(schema_hash);
        self
    }

    #[must_use]
    pub fn host_type(mut self, host_type_id: HostTypeId) -> Self {
        self.host_type_id = Some(host_type_id);
        self
    }

    #[must_use]
    pub fn field(mut self, field: FieldDesc) -> Self {
        self.fields.push(field);
        self
    }

    #[must_use]
    pub fn method(mut self, method: MethodDesc) -> Self {
        self.methods.push(method);
        self
    }

    #[must_use]
    pub fn trait_impl(mut self, trait_desc: TraitDesc) -> Self {
        self.traits.push(trait_desc);
        self
    }

    #[must_use]
    pub fn variant(mut self, variant: VariantDesc) -> Self {
        self.variants.push(variant);
        self
    }

    #[must_use]
    pub fn docs(mut self, docs: impl Into<String>) -> Self {
        self.docs = Some(docs.into());
        self
    }

    #[must_use]
    pub fn attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.attrs.insert(name, value);
        self
    }

    #[must_use]
    pub fn source_span(mut self, source_span: Span) -> Self {
        self.source_span = Some(source_span);
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldDesc {
    pub id: FieldId,
    pub name: String,
    pub type_hint: Option<String>,
    pub writable: bool,
    pub access: FieldAccess,
    pub docs: Option<String>,
    pub attrs: AttrMap,
    pub source_span: Option<Span>,
}

impl FieldDesc {
    #[must_use]
    pub fn new(id: FieldId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            type_hint: None,
            writable: false,
            access: FieldAccess::default(),
            docs: None,
            attrs: AttrMap::new(),
            source_span: None,
        }
    }

    #[must_use]
    pub fn writable(mut self, writable: bool) -> Self {
        self.writable = writable;
        self.access.writable = writable;
        self.access.reflect_writable = writable;
        self
    }

    #[must_use]
    pub fn access(mut self, access: FieldAccess) -> Self {
        self.writable = access.writable;
        self.access = access;
        self
    }

    #[must_use]
    pub fn type_hint(mut self, type_hint: impl Into<String>) -> Self {
        self.type_hint = Some(type_hint.into());
        self
    }

    #[must_use]
    pub fn docs(mut self, docs: impl Into<String>) -> Self {
        self.docs = Some(docs.into());
        self
    }

    #[must_use]
    pub fn attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.attrs.insert(name, value);
        self
    }

    #[must_use]
    pub fn source_span(mut self, source_span: Span) -> Self {
        self.source_span = Some(source_span);
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodDesc {
    pub id: HostMethodId,
    pub name: String,
    pub params: Vec<MethodParamDesc>,
    pub return_type: Option<String>,
    pub effects: MethodEffectSet,
    pub access: MethodAccess,
    pub docs: Option<String>,
    pub attrs: AttrMap,
    pub source_span: Option<Span>,
}

impl MethodDesc {
    #[must_use]
    pub fn new(id: HostMethodId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            params: Vec::new(),
            return_type: None,
            effects: MethodEffectSet::default(),
            access: MethodAccess::default(),
            docs: None,
            attrs: AttrMap::new(),
            source_span: None,
        }
    }

    #[must_use]
    pub fn effects(mut self, effects: MethodEffectSet) -> Self {
        self.effects = effects;
        self
    }

    #[must_use]
    pub fn param(mut self, param: MethodParamDesc) -> Self {
        self.params.push(param);
        self
    }

    #[must_use]
    pub fn return_type(mut self, return_type: impl Into<String>) -> Self {
        self.return_type = Some(return_type.into());
        self
    }

    #[must_use]
    pub fn access(mut self, access: MethodAccess) -> Self {
        self.access = access;
        self
    }

    #[must_use]
    pub fn docs(mut self, docs: impl Into<String>) -> Self {
        self.docs = Some(docs.into());
        self
    }

    #[must_use]
    pub fn attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.attrs.insert(name, value);
        self
    }

    #[must_use]
    pub fn source_span(mut self, source_span: Span) -> Self {
        self.source_span = Some(source_span);
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodParamDesc {
    pub name: String,
    pub type_hint: Option<String>,
    pub has_default: bool,
}

impl MethodParamDesc {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            type_hint: None,
            has_default: false,
        }
    }

    #[must_use]
    pub fn type_hint(mut self, type_hint: impl Into<String>) -> Self {
        self.type_hint = Some(type_hint.into());
        self
    }

    #[must_use]
    pub fn defaulted(mut self, has_default: bool) -> Self {
        self.has_default = has_default;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitDesc {
    pub id: TraitId,
    pub name: String,
    pub methods: Vec<TraitMethodDesc>,
    pub docs: Option<String>,
    pub attrs: AttrMap,
    pub source_span: Option<Span>,
}

impl TraitDesc {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            id: stable_trait_id(&name),
            name,
            methods: Vec::new(),
            docs: None,
            attrs: AttrMap::new(),
            source_span: None,
        }
    }

    #[must_use]
    pub fn method(mut self, method: TraitMethodDesc) -> Self {
        self.methods.push(method);
        self
    }

    #[must_use]
    pub fn docs(mut self, docs: impl Into<String>) -> Self {
        self.docs = Some(docs.into());
        self
    }

    #[must_use]
    pub fn attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.attrs.insert(name, value);
        self
    }

    #[must_use]
    pub fn source_span(mut self, source_span: Span) -> Self {
        self.source_span = Some(source_span);
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitMethodDesc {
    pub id: MethodId,
    pub name: String,
    pub params: Vec<MethodParamDesc>,
    pub return_type: Option<String>,
    pub has_default: bool,
    pub docs: Option<String>,
    pub attrs: AttrMap,
    pub source_span: Option<Span>,
}

impl TraitMethodDesc {
    #[must_use]
    pub fn new(id: MethodId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            params: Vec::new(),
            return_type: None,
            has_default: false,
            docs: None,
            attrs: AttrMap::new(),
            source_span: None,
        }
    }

    #[must_use]
    pub fn defaulted(mut self, has_default: bool) -> Self {
        self.has_default = has_default;
        self
    }

    #[must_use]
    pub fn param(mut self, param: MethodParamDesc) -> Self {
        self.params.push(param);
        self
    }

    #[must_use]
    pub fn return_type(mut self, return_type: impl Into<String>) -> Self {
        self.return_type = Some(return_type.into());
        self
    }

    #[must_use]
    pub fn docs(mut self, docs: impl Into<String>) -> Self {
        self.docs = Some(docs.into());
        self
    }

    #[must_use]
    pub fn attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.attrs.insert(name, value);
        self
    }

    #[must_use]
    pub fn source_span(mut self, source_span: Span) -> Self {
        self.source_span = Some(source_span);
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantDesc {
    pub id: VariantId,
    pub name: String,
    pub fields: Vec<FieldDesc>,
    pub docs: Option<String>,
    pub attrs: AttrMap,
    pub source_span: Option<Span>,
}

impl VariantDesc {
    #[must_use]
    pub fn new(id: VariantId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            fields: Vec::new(),
            docs: None,
            attrs: AttrMap::new(),
            source_span: None,
        }
    }

    #[must_use]
    pub fn field(mut self, field: FieldDesc) -> Self {
        self.fields.push(field);
        self
    }

    #[must_use]
    pub fn docs(mut self, docs: impl Into<String>) -> Self {
        self.docs = Some(docs.into());
        self
    }

    #[must_use]
    pub fn attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.attrs.insert(name, value);
        self
    }

    #[must_use]
    pub fn source_span(mut self, source_span: Span) -> Self {
        self.source_span = Some(source_span);
        self
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TypeRegistry {
    types_by_key: BTreeMap<TypeKey, TypeDesc>,
    host_keys: BTreeMap<HostTypeId, TypeKey>,
    traits_by_name: BTreeMap<String, TraitDesc>,
    modules_by_name: BTreeMap<String, ModuleDesc>,
    functions_by_id: BTreeMap<FunctionId, FunctionDesc>,
    functions_by_name: BTreeMap<String, FunctionId>,
}

impl TypeRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, desc: TypeDesc) {
        if let Some(host_type_id) = desc.host_type_id {
            self.host_keys.insert(host_type_id, desc.key.clone());
        }
        self.types_by_key.insert(desc.key.clone(), desc);
    }

    pub fn register_trait(&mut self, desc: TraitDesc) {
        self.traits_by_name.insert(desc.name.clone(), desc);
    }

    pub fn register_module(&mut self, desc: ModuleDesc) {
        self.modules_by_name.insert(desc.name.clone(), desc);
    }

    pub fn register_function(&mut self, desc: FunctionDesc) {
        self.functions_by_name.insert(desc.name.clone(), desc.id);
        if let Some(module) = &desc.module
            && let Some(module_desc) = self.modules_by_name.get_mut(module)
        {
            module_desc.export_function(desc.name.clone(), desc.id);
        }
        self.functions_by_id.insert(desc.id, desc);
    }

    #[must_use]
    pub fn type_of_host(&self, host_ref: HostRef) -> Option<&TypeDesc> {
        let key = self.host_keys.get(&host_ref.type_id)?;
        self.types_by_key.get(key)
    }

    #[must_use]
    pub fn fields(&self, key: &TypeKey) -> Option<&[FieldDesc]> {
        self.types_by_key
            .get(key)
            .map(|desc| desc.fields.as_slice())
    }

    pub fn types(&self) -> impl Iterator<Item = &TypeDesc> {
        self.types_by_key.values()
    }

    pub fn modules(&self) -> impl Iterator<Item = &ModuleDesc> {
        self.modules_by_name.values()
    }

    pub fn functions(&self) -> impl Iterator<Item = &FunctionDesc> {
        self.functions_by_id.values()
    }

    #[must_use]
    pub fn module_by_name(&self, name: &str) -> Option<&ModuleDesc> {
        self.modules_by_name.get(name)
    }

    #[must_use]
    pub fn function_by_id(&self, id: FunctionId) -> Option<&FunctionDesc> {
        self.functions_by_id.get(&id)
    }

    #[must_use]
    pub fn function_by_name(&self, name: &str) -> Option<&FunctionDesc> {
        let id = self.functions_by_name.get(name)?;
        self.functions_by_id.get(id)
    }

    #[must_use]
    pub fn type_by_name(&self, name: &str) -> Option<&TypeDesc> {
        self.types_by_key
            .values()
            .find(|desc| desc.key.name == name)
    }

    #[must_use]
    pub fn trait_by_name(&self, name: &str) -> Option<&TraitDesc> {
        self.traits_by_name.get(name)
    }

    pub(crate) fn trait_metadata_by_name(&self, name: &str) -> Option<&TraitDesc> {
        self.traits_by_name.get(name).or_else(|| {
            self.types()
                .flat_map(|type_desc| type_desc.traits.iter())
                .find(|trait_desc| trait_desc.name == name)
        })
    }

    pub(crate) fn known_trait_names(&self) -> Vec<String> {
        let mut names = BTreeSet::new();
        names.extend(self.traits_by_name.keys().cloned());
        for type_desc in self.types() {
            names.extend(
                type_desc
                    .traits
                    .iter()
                    .map(|trait_desc| trait_desc.name.clone()),
            );
        }
        names.into_iter().collect()
    }

    pub(crate) fn known_trait_candidates(&self) -> Vec<(String, Option<Span>)> {
        let mut candidates = BTreeMap::new();
        for trait_desc in self.traits_by_name.values() {
            candidates.insert(trait_desc.name.clone(), trait_desc.source_span);
        }
        for type_desc in self.types() {
            for trait_desc in &type_desc.traits {
                candidates
                    .entry(trait_desc.name.clone())
                    .or_insert(trait_desc.source_span);
            }
        }
        candidates.into_iter().collect()
    }

    pub(crate) fn type_by_name_mut(&mut self, name: &str) -> Option<&mut TypeDesc> {
        let key = self
            .types_by_key
            .keys()
            .find(|key| key.name == name)
            .cloned()?;
        self.types_by_key.get_mut(&key)
    }

    pub(crate) fn host_field(
        &self,
        host_ref: HostRef,
        field_name: &str,
    ) -> ReflectResult<&FieldDesc> {
        let desc = self.type_of_host(host_ref).ok_or_else(|| {
            ReflectError::new(ReflectErrorKind::UnknownType {
                host_type_id: host_ref.type_id,
            })
        })?;
        find_field(desc, field_name)
    }

    pub(crate) fn host_method(
        &self,
        host_ref: HostRef,
        method_name: &str,
    ) -> ReflectResult<&MethodDesc> {
        let desc = self.type_of_host(host_ref).ok_or_else(|| {
            ReflectError::new(ReflectErrorKind::UnknownType {
                host_type_id: host_ref.type_id,
            })
        })?;
        find_method(desc, method_name)
    }
}

fn find_field<'a>(desc: &'a TypeDesc, field: &str) -> ReflectResult<&'a FieldDesc> {
    desc.fields
        .iter()
        .find(|candidate| candidate.name == field)
        .ok_or_else(|| {
            let related = ranked_candidates(
                field,
                desc.fields
                    .iter()
                    .map(|field| (field.name.as_str(), field.source_span)),
            );
            ReflectError::new(ReflectErrorKind::UnknownField {
                type_name: desc.key.name.clone(),
                field: field.to_owned(),
                candidates: candidate_names(&related),
                related,
            })
        })
}

fn find_method<'a>(desc: &'a TypeDesc, method: &str) -> ReflectResult<&'a MethodDesc> {
    desc.methods
        .iter()
        .find(|candidate| candidate.name == method)
        .ok_or_else(|| {
            let related = ranked_candidates(
                method,
                desc.methods
                    .iter()
                    .map(|method| (method.name.as_str(), method.source_span)),
            );
            ReflectError::new(ReflectErrorKind::UnknownMethod {
                type_name: desc.key.name.clone(),
                method: method.to_owned(),
                candidates: candidate_names(&related),
                related,
            })
        })
}

fn stable_trait_id(name: &str) -> TraitId {
    TraitId::new(stable_reflect_id("trait", name, ""))
}

fn stable_reflect_id(kind: &str, owner: &str, member: &str) -> u32 {
    let mut hash = 0x811c_9dc5;
    for byte in kind
        .bytes()
        .chain([0])
        .chain(owner.bytes())
        .chain([0])
        .chain(member.bytes())
    {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    if hash == 0 { 1 } else { hash }
}
