//! Controlled reflection metadata and value access.

mod access;
mod members;
mod metadata;
mod modules;
mod permissions;
mod script_attrs;
mod script_types;

use std::collections::BTreeMap;
use std::fmt;

pub use access::{FunctionAccess, FunctionEffectSet, MethodAccess, MethodEffectSet};
pub use members::{
    attrs as attrs_metadata, docs as docs_metadata, field as field_metadata, has_field, has_method,
    kind as kind_metadata, methods, name as name_metadata, traits as trait_metadata, variant,
    variant_is, variants as variant_metadata,
};
pub use modules::{
    DeclOrigin, FunctionDesc, FunctionParamDesc, ModuleDesc, ModuleExportDesc, ModuleExportKind,
    exports as module_exports, function as function_metadata, module as module_metadata,
};
pub use permissions::{
    ReflectLookupBudget, ReflectPermission, ReflectPermissionSet, ReflectPolicy,
};
use vela_common::{
    FieldId, FunctionId, HostMethodId, HostTypeId, MethodId, TraitId, TypeId, VariantId,
};
use vela_host::{HostPath, HostRef, HostValue, PatchTx, ScriptStateAdapter};

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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldDesc {
    pub id: FieldId,
    pub name: String,
    pub writable: bool,
    pub docs: Option<String>,
    pub attrs: AttrMap,
}

impl FieldDesc {
    #[must_use]
    pub fn new(id: FieldId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            writable: false,
            docs: None,
            attrs: AttrMap::new(),
        }
    }

    #[must_use]
    pub fn writable(mut self, writable: bool) -> Self {
        self.writable = writable;
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodDesc {
    pub id: HostMethodId,
    pub name: String,
    pub effects: MethodEffectSet,
    pub access: MethodAccess,
    pub docs: Option<String>,
    pub attrs: AttrMap,
}

impl MethodDesc {
    #[must_use]
    pub fn new(id: HostMethodId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            effects: MethodEffectSet::default(),
            access: MethodAccess::default(),
            docs: None,
            attrs: AttrMap::new(),
        }
    }

    #[must_use]
    pub fn effects(mut self, effects: MethodEffectSet) -> Self {
        self.effects = effects;
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitDesc {
    pub id: TraitId,
    pub name: String,
    pub methods: Vec<TraitMethodDesc>,
    pub docs: Option<String>,
    pub attrs: AttrMap,
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitMethodDesc {
    pub id: MethodId,
    pub name: String,
    pub has_default: bool,
    pub docs: Option<String>,
    pub attrs: AttrMap,
}

impl TraitMethodDesc {
    #[must_use]
    pub fn new(id: MethodId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            has_default: false,
            docs: None,
            attrs: AttrMap::new(),
        }
    }

    #[must_use]
    pub fn defaulted(mut self, has_default: bool) -> Self {
        self.has_default = has_default;
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantDesc {
    pub id: VariantId,
    pub name: String,
    pub fields: Vec<FieldDesc>,
    pub docs: Option<String>,
    pub attrs: AttrMap,
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

    fn type_by_name_mut(&mut self, name: &str) -> Option<&mut TypeDesc> {
        let key = self
            .types_by_key
            .keys()
            .find(|key| key.name == name)
            .cloned()?;
        self.types_by_key.get_mut(&key)
    }

    fn host_field(&self, host_ref: HostRef, field_name: &str) -> ReflectResult<&FieldDesc> {
        let desc = self.type_of_host(host_ref).ok_or_else(|| {
            ReflectError::new(ReflectErrorKind::UnknownType {
                host_type_id: host_ref.type_id,
            })
        })?;
        find_field(desc, field_name)
    }

    fn host_method(&self, host_ref: HostRef, method_name: &str) -> ReflectResult<&MethodDesc> {
        let desc = self.type_of_host(host_ref).ok_or_else(|| {
            ReflectError::new(ReflectErrorKind::UnknownType {
                host_type_id: host_ref.type_id,
            })
        })?;
        find_method(desc, method_name)
    }
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

#[derive(Clone, Debug, PartialEq)]
pub enum ReflectValue {
    Host(HostValue),
    HostRef(HostRef),
    Record(BTreeMap<String, ReflectValue>),
    ScriptRecord {
        type_name: String,
        fields: BTreeMap<String, ReflectValue>,
    },
    ScriptEnum {
        enum_name: String,
        variant: String,
        fields: BTreeMap<String, ReflectValue>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReflectError {
    pub kind: ReflectErrorKind,
}

impl ReflectError {
    fn new(kind: ReflectErrorKind) -> Self {
        Self { kind }
    }
}

impl fmt::Display for ReflectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.kind)
    }
}

impl std::error::Error for ReflectError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReflectErrorKind {
    UnknownType {
        host_type_id: HostTypeId,
    },
    UnknownField {
        type_name: String,
        field: String,
        candidates: Vec<String>,
    },
    UnknownMethod {
        type_name: String,
        method: String,
        candidates: Vec<String>,
    },
    UnknownModule {
        module: String,
        candidates: Vec<String>,
    },
    UnknownFunction {
        function: String,
        candidates: Vec<String>,
    },
    PermissionDenied {
        permission: ReflectPermission,
    },
    MethodNotReflectCallable {
        type_name: String,
        method: String,
    },
    MethodPermissionDenied {
        method: String,
        permission: String,
    },
    LookupBudgetExceeded {
        limit: u64,
    },
    FieldNotWritable {
        type_name: String,
        field: String,
    },
    InvalidTarget,
    InvalidValue,
    Host(String),
}

pub type ReflectResult<T> = Result<T, ReflectError>;

pub struct ReflectContext<'a> {
    pub registry: &'a TypeRegistry,
    pub adapter: &'a dyn ScriptStateAdapter,
    pub tx: &'a mut PatchTx,
}

pub fn type_of<'a>(registry: &'a TypeRegistry, value: &ReflectValue) -> Option<&'a TypeDesc> {
    match value {
        ReflectValue::HostRef(host_ref) => registry.type_of_host(*host_ref),
        ReflectValue::ScriptRecord { type_name, .. } => registry.type_by_name(type_name),
        ReflectValue::ScriptEnum { enum_name, .. } => registry.type_by_name(enum_name),
        ReflectValue::Host(_) | ReflectValue::Record(_) => None,
    }
}

pub fn fields<'a>(registry: &'a TypeRegistry, key: &TypeKey) -> Option<&'a [FieldDesc]> {
    registry.fields(key)
}

pub fn get(
    ctx: &mut ReflectContext<'_>,
    target: &ReflectValue,
    field: &str,
) -> ReflectResult<ReflectValue> {
    match target {
        ReflectValue::HostRef(host_ref) => {
            let field_desc = ctx.registry.host_field(*host_ref, field)?;
            let value = ctx
                .tx
                .read_path(ctx.adapter, &HostPath::new(*host_ref).field(field_desc.id))
                .map_err(|error| ReflectError::new(ReflectErrorKind::Host(error.to_string())))?;
            Ok(ReflectValue::Host(value))
        }
        ReflectValue::Record(record) => get_record_field(field, record),
        ReflectValue::ScriptRecord { fields, .. } | ReflectValue::ScriptEnum { fields, .. } => {
            get_record_field(field, fields)
        }
        ReflectValue::Host(_) => Err(ReflectError::new(ReflectErrorKind::InvalidTarget)),
    }
}

pub fn set(
    ctx: &mut ReflectContext<'_>,
    target: &ReflectValue,
    field: &str,
    value: ReflectValue,
) -> ReflectResult<()> {
    match target {
        ReflectValue::HostRef(host_ref) => {
            let field_desc = ctx.registry.host_field(*host_ref, field)?;
            if !field_desc.writable {
                let type_name = ctx
                    .registry
                    .type_of_host(*host_ref)
                    .map_or_else(|| "<unknown>".to_owned(), |desc| desc.key.name.clone());
                return Err(ReflectError::new(ReflectErrorKind::FieldNotWritable {
                    type_name,
                    field: field.to_owned(),
                }));
            }
            let ReflectValue::Host(value) = value else {
                return Err(ReflectError::new(ReflectErrorKind::InvalidValue));
            };
            ctx.tx
                .set_path(HostPath::new(*host_ref).field(field_desc.id), value, None)
                .map_err(|error| ReflectError::new(ReflectErrorKind::Host(error.to_string())))?;
            Ok(())
        }
        ReflectValue::Record(_)
        | ReflectValue::ScriptRecord { .. }
        | ReflectValue::ScriptEnum { .. }
        | ReflectValue::Host(_) => Err(ReflectError::new(ReflectErrorKind::InvalidTarget)),
    }
}

pub fn call(
    ctx: &mut ReflectContext<'_>,
    target: &ReflectValue,
    method: &str,
    args: Vec<ReflectValue>,
) -> ReflectResult<ReflectValue> {
    call_impl(ctx, target, method, args, None)
}

pub fn call_with_policy(
    ctx: &mut ReflectContext<'_>,
    target: &ReflectValue,
    method: &str,
    args: Vec<ReflectValue>,
    policy: &ReflectPolicy,
) -> ReflectResult<ReflectValue> {
    call_impl(ctx, target, method, args, Some(policy))
}

fn call_impl(
    ctx: &mut ReflectContext<'_>,
    target: &ReflectValue,
    method: &str,
    args: Vec<ReflectValue>,
    policy: Option<&ReflectPolicy>,
) -> ReflectResult<ReflectValue> {
    let ReflectValue::HostRef(host_ref) = target else {
        return Err(ReflectError::new(ReflectErrorKind::InvalidTarget));
    };
    let type_name = ctx
        .registry
        .type_of_host(*host_ref)
        .map_or_else(|| "<unknown>".to_owned(), |desc| desc.key.name.clone());
    let method_desc = ctx.registry.host_method(*host_ref, method)?;
    if let Some(policy) = policy {
        policy.require_method_access(&type_name, method_desc)?;
    }
    let args = args
        .into_iter()
        .map(host_arg)
        .collect::<ReflectResult<Vec<_>>>()?;
    ctx.tx
        .call_method(HostPath::new(*host_ref), method_desc.id, args, None)
        .map_err(|error| ReflectError::new(ReflectErrorKind::Host(error.to_string())))?;
    Ok(ReflectValue::Host(HostValue::Null))
}

pub fn implements(
    registry: &TypeRegistry,
    target: &ReflectValue,
    trait_name: &str,
) -> ReflectResult<bool> {
    match target {
        ReflectValue::HostRef(host_ref) => {
            let desc = registry.type_of_host(*host_ref).ok_or_else(|| {
                ReflectError::new(ReflectErrorKind::UnknownType {
                    host_type_id: host_ref.type_id,
                })
            })?;
            Ok(desc
                .traits
                .iter()
                .any(|trait_desc| trait_desc.name == trait_name))
        }
        ReflectValue::ScriptRecord { type_name, .. }
        | ReflectValue::ScriptEnum {
            enum_name: type_name,
            ..
        } => {
            let Some(desc) = registry.type_by_name(type_name) else {
                return Ok(false);
            };
            Ok(desc
                .traits
                .iter()
                .any(|trait_desc| trait_desc.name == trait_name))
        }
        ReflectValue::Host(_) | ReflectValue::Record(_) => {
            Err(ReflectError::new(ReflectErrorKind::InvalidTarget))
        }
    }
}

fn get_record_field(
    field: &str,
    record: &BTreeMap<String, ReflectValue>,
) -> ReflectResult<ReflectValue> {
    record
        .get(field)
        .cloned()
        .ok_or_else(|| ReflectError::new(record_unknown_field(field, record)))
}

fn find_field<'a>(desc: &'a TypeDesc, field: &str) -> ReflectResult<&'a FieldDesc> {
    desc.fields
        .iter()
        .find(|candidate| candidate.name == field)
        .ok_or_else(|| {
            ReflectError::new(ReflectErrorKind::UnknownField {
                type_name: desc.key.name.clone(),
                field: field.to_owned(),
                candidates: name_candidates(
                    field,
                    desc.fields.iter().map(|field| field.name.as_str()),
                ),
            })
        })
}

fn find_method<'a>(desc: &'a TypeDesc, method: &str) -> ReflectResult<&'a MethodDesc> {
    desc.methods
        .iter()
        .find(|candidate| candidate.name == method)
        .ok_or_else(|| {
            ReflectError::new(ReflectErrorKind::UnknownMethod {
                type_name: desc.key.name.clone(),
                method: method.to_owned(),
                candidates: name_candidates(
                    method,
                    desc.methods.iter().map(|method| method.name.as_str()),
                ),
            })
        })
}

fn host_arg(value: ReflectValue) -> ReflectResult<HostValue> {
    let ReflectValue::Host(value) = value else {
        return Err(ReflectError::new(ReflectErrorKind::InvalidValue));
    };
    Ok(value)
}

fn record_unknown_field(field: &str, record: &BTreeMap<String, ReflectValue>) -> ReflectErrorKind {
    ReflectErrorKind::UnknownField {
        type_name: "record".to_owned(),
        field: field.to_owned(),
        candidates: name_candidates(field, record.keys().map(String::as_str)),
    }
}

pub(crate) fn name_candidates<'a>(
    name: &str,
    candidates: impl Iterator<Item = &'a str>,
) -> Vec<String> {
    let mut candidates = candidates
        .map(|candidate| (edit_distance(name, candidate), candidate.to_owned()))
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    candidates
        .into_iter()
        .take(3)
        .map(|(_, candidate)| candidate)
        .collect()
}

fn edit_distance(left: &str, right: &str) -> usize {
    let left = left.chars().collect::<Vec<_>>();
    let right = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right.len()).collect::<Vec<_>>();
    let mut current = vec![0; right.len() + 1];

    for (left_index, left_ch) in left.iter().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_ch) in right.iter().enumerate() {
            let substitution = usize::from(left_ch != right_ch);
            current[right_index + 1] = (previous[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(previous[right_index] + substitution);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[right.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_common::{HostObjectId, SourceId, Span};
    use vela_host::{HostObjectSnapshot, MockStateAdapter, PatchOp};

    fn player_ref() -> HostRef {
        HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 3)
    }

    fn registry() -> TypeRegistry {
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
                .host_type(HostTypeId::new(1))
                .field(FieldDesc::new(FieldId::new(1), "id"))
                .field(FieldDesc::new(FieldId::new(2), "level").writable(true))
                .method(MethodDesc::new(HostMethodId::new(5), "grant_exp"))
                .trait_impl(TraitDesc::new("Damageable")),
        );
        registry
    }

    fn adapter_with_level(value: HostValue) -> MockStateAdapter {
        let mut adapter = MockStateAdapter::new();
        adapter.insert_value(HostPath::new(player_ref()).field(FieldId::new(2)), value);
        adapter
    }

    #[test]
    fn reflect_set_host_ref_creates_patch() {
        let registry = registry();
        let adapter = adapter_with_level(HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut ctx = ReflectContext {
            registry: &registry,
            adapter: &adapter,
            tx: &mut tx,
        };

        set(
            &mut ctx,
            &ReflectValue::HostRef(player_ref()),
            "level",
            ReflectValue::Host(HostValue::Int(10)),
        )
        .expect("reflect set");

        assert_eq!(ctx.tx.patches().len(), 1);
        assert_eq!(ctx.tx.patches()[0].op, PatchOp::Set(HostValue::Int(10)));
    }

    #[test]
    fn reflect_get_host_ref_reads_overlay_before_adapter() {
        let registry = registry();
        let adapter = adapter_with_level(HostValue::Int(9));
        let mut tx = PatchTx::new();
        tx.set_path(
            HostPath::new(player_ref()).field(FieldId::new(2)),
            HostValue::Int(12),
            Some(Span::new(SourceId::new(1), 0, 1)),
        )
        .expect("set overlay");
        let mut ctx = ReflectContext {
            registry: &registry,
            adapter: &adapter,
            tx: &mut tx,
        };

        let value =
            get(&mut ctx, &ReflectValue::HostRef(player_ref()), "level").expect("reflect get");

        assert_eq!(value, ReflectValue::Host(HostValue::Int(12)));
    }

    #[test]
    fn reflect_get_record_field_reads_value() {
        let registry = TypeRegistry::new();
        let adapter = MockStateAdapter::new();
        let mut tx = PatchTx::new();
        let mut record = BTreeMap::new();
        record.insert("field".to_owned(), ReflectValue::Host(HostValue::Int(42)));
        let mut ctx = ReflectContext {
            registry: &registry,
            adapter: &adapter,
            tx: &mut tx,
        };

        let value = get(&mut ctx, &ReflectValue::Record(record), "field").expect("record get");

        assert_eq!(value, ReflectValue::Host(HostValue::Int(42)));
    }

    #[test]
    fn reflect_set_read_only_host_field_fails() {
        let registry = registry();
        let adapter = adapter_with_level(HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut ctx = ReflectContext {
            registry: &registry,
            adapter: &adapter,
            tx: &mut tx,
        };

        let error = set(
            &mut ctx,
            &ReflectValue::HostRef(player_ref()),
            "id",
            ReflectValue::Host(HostValue::Int(10)),
        )
        .expect_err("read-only set");

        assert_eq!(
            error.kind,
            ReflectErrorKind::FieldNotWritable {
                type_name: "Player".to_owned(),
                field: "id".to_owned()
            }
        );
        assert!(ctx.tx.patches().is_empty());
    }

    #[test]
    fn unknown_fields_include_candidate_hints() {
        let registry = registry();
        let adapter = adapter_with_level(HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut ctx = ReflectContext {
            registry: &registry,
            adapter: &adapter,
            tx: &mut tx,
        };

        let error = get(&mut ctx, &ReflectValue::HostRef(player_ref()), "levle")
            .expect_err("unknown field");

        assert_eq!(
            error.kind,
            ReflectErrorKind::UnknownField {
                type_name: "Player".to_owned(),
                field: "levle".to_owned(),
                candidates: vec!["level".to_owned(), "id".to_owned()]
            }
        );
    }

    #[test]
    fn type_registry_exposes_host_type_fields() {
        let registry = registry();
        let desc = type_of(&registry, &ReflectValue::HostRef(player_ref())).expect("type desc");
        let fields = fields(&registry, &desc.key).expect("fields");

        assert_eq!(desc.key.name, "Player");
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[1].name, "level");
    }

    #[test]
    fn reflect_get_propagates_host_generation_errors() {
        let registry = registry();
        let mut adapter = MockStateAdapter::new();
        let fresh_ref = player_ref();
        adapter.insert_value(
            HostPath::new(fresh_ref).field(FieldId::new(2)),
            HostValue::Int(9),
        );
        let stale_ref = HostRef::new(fresh_ref.type_id, fresh_ref.object_id, 2);
        let mut tx = PatchTx::new();
        let mut ctx = ReflectContext {
            registry: &registry,
            adapter: &adapter,
            tx: &mut tx,
        };

        let error =
            get(&mut ctx, &ReflectValue::HostRef(stale_ref), "level").expect_err("stale get");

        assert!(matches!(error.kind, ReflectErrorKind::Host(_)));
        assert_eq!(
            vela_host::PatchTx::require_fresh_ref(
                stale_ref,
                &HostObjectSnapshot {
                    type_id: fresh_ref.type_id,
                    object_id: fresh_ref.object_id,
                    generation: 3,
                }
            )
            .expect_err("stale ref")
            .kind,
            vela_host::HostErrorKind::StaleGeneration {
                expected: 2,
                actual: 3
            }
        );
    }

    #[test]
    fn reflect_call_host_ref_records_patch() {
        let registry = registry();
        let mut adapter = adapter_with_level(HostValue::Int(9));
        adapter.insert_method_return(HostMethodId::new(5), HostValue::Null);
        let mut tx = PatchTx::new();
        {
            let mut ctx = ReflectContext {
                registry: &registry,
                adapter: &adapter,
                tx: &mut tx,
            };

            let value = call(
                &mut ctx,
                &ReflectValue::HostRef(player_ref()),
                "grant_exp",
                vec![ReflectValue::Host(HostValue::Int(20))],
            )
            .expect("reflect call");

            assert_eq!(value, ReflectValue::Host(HostValue::Null));
            assert_eq!(ctx.tx.patches().len(), 1);
            assert_eq!(
                ctx.tx.patches()[0].op,
                PatchOp::CallHostMethod {
                    method: HostMethodId::new(5),
                    args: vec![HostValue::Int(20)]
                }
            );
            assert!(adapter.method_calls().is_empty());
        }

        tx.apply(&mut adapter).expect("apply reflect call");
        assert_eq!(
            adapter.method_calls(),
            &[(
                HostPath::new(player_ref()),
                HostMethodId::new(5),
                vec![HostValue::Int(20)]
            )]
        );
    }

    #[test]
    fn reflect_call_rejects_non_host_args() {
        let registry = registry();
        let adapter = adapter_with_level(HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut ctx = ReflectContext {
            registry: &registry,
            adapter: &adapter,
            tx: &mut tx,
        };

        let error = call(
            &mut ctx,
            &ReflectValue::HostRef(player_ref()),
            "grant_exp",
            vec![ReflectValue::Record(BTreeMap::new())],
        )
        .expect_err("invalid arg");

        assert_eq!(error.kind, ReflectErrorKind::InvalidValue);
        assert!(ctx.tx.patches().is_empty());
    }

    #[test]
    fn reflect_call_with_policy_denies_unapproved_methods_before_patch() {
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
                .host_type(HostTypeId::new(1))
                .method(
                    MethodDesc::new(HostMethodId::new(5), "grant_exp")
                        .access(MethodAccess::new().reflect_callable(false)),
                )
                .method(
                    MethodDesc::new(HostMethodId::new(6), "admin_grant")
                        .access(MethodAccess::new().require_permission("player.admin")),
                ),
        );
        let adapter = adapter_with_level(HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut ctx = ReflectContext {
            registry: &registry,
            adapter: &adapter,
            tx: &mut tx,
        };

        let error = call_with_policy(
            &mut ctx,
            &ReflectValue::HostRef(player_ref()),
            "grant_exp",
            vec![ReflectValue::Host(HostValue::Int(20))],
            &ReflectPolicy::all(),
        )
        .expect_err("not reflect callable");
        assert_eq!(
            error.kind,
            ReflectErrorKind::MethodNotReflectCallable {
                type_name: "Player".to_owned(),
                method: "grant_exp".to_owned()
            }
        );
        assert!(ctx.tx.patches().is_empty());

        let error = call_with_policy(
            &mut ctx,
            &ReflectValue::HostRef(player_ref()),
            "admin_grant",
            vec![ReflectValue::Host(HostValue::Int(20))],
            &ReflectPolicy::all(),
        )
        .expect_err("missing method permission");
        assert_eq!(
            error.kind,
            ReflectErrorKind::MethodPermissionDenied {
                method: "admin_grant".to_owned(),
                permission: "player.admin".to_owned()
            }
        );
        assert!(ctx.tx.patches().is_empty());
    }

    #[test]
    fn unknown_methods_include_candidate_hints() {
        let registry = registry();
        let adapter = adapter_with_level(HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut ctx = ReflectContext {
            registry: &registry,
            adapter: &adapter,
            tx: &mut tx,
        };

        let error = call(
            &mut ctx,
            &ReflectValue::HostRef(player_ref()),
            "grant_xp",
            Vec::new(),
        )
        .expect_err("unknown method");

        assert_eq!(
            error.kind,
            ReflectErrorKind::UnknownMethod {
                type_name: "Player".to_owned(),
                method: "grant_xp".to_owned(),
                candidates: vec!["grant_exp".to_owned()]
            }
        );
    }

    #[test]
    fn reflect_implements_uses_registry_metadata() {
        let registry = registry();

        assert!(
            implements(
                &registry,
                &ReflectValue::HostRef(player_ref()),
                "Damageable"
            )
            .expect("implements check")
        );
        assert!(
            !implements(&registry, &ReflectValue::HostRef(player_ref()), "Inventory")
                .expect("implements check")
        );
    }

    #[test]
    fn reflect_implements_uses_script_type_metadata() {
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(200), "game.Player"))
                .kind(TypeKind::ScriptStruct)
                .trait_impl(TraitDesc::new("game.Damageable")),
        );
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(201), "game.Progress"))
                .kind(TypeKind::ScriptEnum)
                .trait_impl(TraitDesc::new("game.Trackable")),
        );

        assert!(
            implements(
                &registry,
                &ReflectValue::ScriptRecord {
                    type_name: "game.Player".to_owned(),
                    fields: BTreeMap::new(),
                },
                "game.Damageable",
            )
            .expect("script record implements check")
        );
        assert!(
            implements(
                &registry,
                &ReflectValue::ScriptEnum {
                    enum_name: "game.Progress".to_owned(),
                    variant: "Active".to_owned(),
                    fields: BTreeMap::new(),
                },
                "game.Trackable",
            )
            .expect("script enum implements check")
        );
        assert!(
            !implements(
                &registry,
                &ReflectValue::ScriptRecord {
                    type_name: "game.Player".to_owned(),
                    fields: BTreeMap::new(),
                },
                "game.Trackable",
            )
            .expect("script record negative implements check")
        );
    }
}
