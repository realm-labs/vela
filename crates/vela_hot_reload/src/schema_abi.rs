use std::collections::BTreeMap;

use vela_common::Span;
use vela_reflect::access::FieldAccess;
use vela_reflect::registry::{FieldDesc, SchemaHash, TraitDesc, TypeDesc, TypeKind, VariantDesc};

use crate::abi::TraitMethodAbi;
use crate::error::{HotReloadError, HotReloadErrorKind, HotReloadResult};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaAbi {
    pub type_name: String,
    pub hash: u64,
    pub kind: Option<SchemaKindAbi>,
    pub fields: Vec<SchemaFieldAbi>,
    pub variants: Vec<SchemaVariantAbi>,
    pub trait_impls: Vec<SchemaTraitImplAbi>,
    pub source_span: Option<Span>,
}

impl SchemaAbi {
    #[must_use]
    pub fn new(type_name: impl Into<String>, hash: SchemaHash) -> Self {
        Self {
            type_name: type_name.into(),
            hash: hash.get(),
            kind: None,
            fields: Vec::new(),
            variants: Vec::new(),
            trait_impls: Vec::new(),
            source_span: None,
        }
    }

    #[must_use]
    pub fn from_type(type_desc: &TypeDesc) -> Option<Self> {
        let schema_hash = type_desc.schema_hash?;
        let mut abi = Self::new(type_desc.key.name.clone(), schema_hash)
            .kind(SchemaKindAbi::from_type_kind(type_desc.kind));
        for field in &type_desc.fields {
            abi = abi.field(SchemaFieldAbi::from_field(field));
        }
        for variant in &type_desc.variants {
            abi = abi.variant(SchemaVariantAbi::from_variant(variant));
        }
        for trait_desc in &type_desc.traits {
            abi = abi.trait_impl(SchemaTraitImplAbi::from_trait(trait_desc));
        }
        if let Some(source_span) = type_desc.source_span {
            abi = abi.source_span(source_span);
        }
        Some(abi)
    }

    #[must_use]
    pub fn kind(mut self, kind: SchemaKindAbi) -> Self {
        self.kind = Some(kind);
        self
    }

    #[must_use]
    pub fn field(mut self, field: SchemaFieldAbi) -> Self {
        self.fields.push(field);
        self
    }

    #[must_use]
    pub fn variant(mut self, variant: SchemaVariantAbi) -> Self {
        self.variants.push(variant);
        self
    }

    #[must_use]
    pub fn trait_impl(mut self, trait_impl: SchemaTraitImplAbi) -> Self {
        self.trait_impls.push(trait_impl);
        self
    }

    #[must_use]
    pub fn source_span(mut self, source_span: Span) -> Self {
        self.source_span = Some(source_span);
        self
    }

    #[must_use]
    pub fn has_member_abi(&self) -> bool {
        !self.fields.is_empty() || !self.variants.is_empty() || !self.trait_impls.is_empty()
    }

    pub(crate) fn ensure_compatible(&self, next: &Self) -> HotReloadResult<()> {
        let compatible = self.kind == next.kind
            && fields_compatible(&self.fields, &next.fields)
            && variants_compatible(&self.variants, &next.variants)
            && trait_impls_compatible(&self.trait_impls, &next.trait_impls);
        if compatible {
            return Ok(());
        }
        Err(HotReloadError::new(HotReloadErrorKind::ChangedSchemaAbi {
            type_name: self.type_name.clone(),
            old: Box::new(self.clone()),
            new: Box::new(next.clone()),
            source_span: next.source_span.map(Box::new),
        }))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaTraitImplAbi {
    pub id: u32,
    pub name: String,
    pub methods: Vec<TraitMethodAbi>,
}

impl SchemaTraitImplAbi {
    #[must_use]
    pub fn new(id: u32, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            methods: Vec::new(),
        }
    }

    #[must_use]
    pub fn from_trait(trait_desc: &TraitDesc) -> Self {
        let mut abi = Self::new(trait_desc.id.get(), trait_desc.name.clone());
        for method in &trait_desc.methods {
            abi = abi.method(TraitMethodAbi::from_method(method));
        }
        abi
    }

    #[must_use]
    pub fn method(mut self, method: TraitMethodAbi) -> Self {
        self.methods.push(method);
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchemaKindAbi {
    Null,
    Bool,
    Int,
    Float,
    String,
    Array,
    Map,
    Set,
    Range,
    Function,
    Closure,
    Host,
    ScriptStruct,
    ScriptEnum,
}

impl SchemaKindAbi {
    #[must_use]
    pub const fn from_type_kind(kind: TypeKind) -> Self {
        match kind {
            TypeKind::Null => Self::Null,
            TypeKind::Bool => Self::Bool,
            TypeKind::Int => Self::Int,
            TypeKind::Float => Self::Float,
            TypeKind::String => Self::String,
            TypeKind::Array => Self::Array,
            TypeKind::Map => Self::Map,
            TypeKind::Set => Self::Set,
            TypeKind::Range => Self::Range,
            TypeKind::Function => Self::Function,
            TypeKind::Closure => Self::Closure,
            TypeKind::Host => Self::Host,
            TypeKind::ScriptStruct => Self::ScriptStruct,
            TypeKind::ScriptEnum => Self::ScriptEnum,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool => "bool",
            Self::Int => "int",
            Self::Float => "float",
            Self::String => "string",
            Self::Array => "array",
            Self::Map => "map",
            Self::Set => "set",
            Self::Range => "range",
            Self::Function => "function",
            Self::Closure => "closure",
            Self::Host => "host",
            Self::ScriptStruct => "script_struct",
            Self::ScriptEnum => "script_enum",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaFieldAbi {
    pub id: u32,
    pub name: String,
    pub type_hint: Option<String>,
    pub has_default: bool,
    pub writable: bool,
    pub access: FieldAccessAbi,
}

impl SchemaFieldAbi {
    #[must_use]
    pub fn new(id: u32, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            type_hint: None,
            has_default: false,
            writable: false,
            access: FieldAccessAbi::default(),
        }
    }

    #[must_use]
    pub fn from_field(field: &FieldDesc) -> Self {
        let mut abi = Self::new(field.id.get(), field.name.clone())
            .defaulted(field.has_default)
            .writable(field.writable)
            .access(FieldAccessAbi::from_access(&field.access));
        if let Some(type_hint) = &field.type_hint {
            abi = abi.type_hint(type_hint.clone());
        }
        abi
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

    #[must_use]
    pub fn writable(mut self, writable: bool) -> Self {
        self.writable = writable;
        self
    }

    #[must_use]
    pub fn access(mut self, access: FieldAccessAbi) -> Self {
        self.access = access;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaVariantAbi {
    pub id: u32,
    pub name: String,
    pub fields: Vec<SchemaFieldAbi>,
}

impl SchemaVariantAbi {
    #[must_use]
    pub fn new(id: u32, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            fields: Vec::new(),
        }
    }

    #[must_use]
    pub fn from_variant(variant: &VariantDesc) -> Self {
        let mut abi = Self::new(variant.id.get(), variant.name.clone());
        for field in &variant.fields {
            abi = abi.field(SchemaFieldAbi::from_field(field));
        }
        abi
    }

    #[must_use]
    pub fn field(mut self, field: SchemaFieldAbi) -> Self {
        self.fields.push(field);
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldAccessAbi {
    pub readable: bool,
    pub writable: bool,
    pub reflect_readable: bool,
    pub reflect_writable: bool,
    pub required_permissions: Vec<String>,
}

impl FieldAccessAbi {
    #[must_use]
    pub fn from_access(access: &FieldAccess) -> Self {
        Self::new(
            access.readable,
            access.writable,
            access.reflect_readable,
            access.reflect_writable,
            access.required_permissions().to_vec(),
        )
    }

    #[must_use]
    pub fn new(
        readable: bool,
        writable: bool,
        reflect_readable: bool,
        reflect_writable: bool,
        mut required_permissions: Vec<String>,
    ) -> Self {
        required_permissions.sort();
        required_permissions.dedup();
        Self {
            readable,
            writable,
            reflect_readable,
            reflect_writable,
            required_permissions,
        }
    }
}

impl Default for FieldAccessAbi {
    fn default() -> Self {
        Self::new(true, false, true, false, Vec::new())
    }
}

fn fields_compatible(old: &[SchemaFieldAbi], new: &[SchemaFieldAbi]) -> bool {
    let new_fields = new
        .iter()
        .map(|field| (field.name.as_str(), field))
        .collect::<BTreeMap<_, _>>();
    let old_fields = old
        .iter()
        .map(|field| field.name.as_str())
        .collect::<Vec<_>>();
    let existing_compatible = old.iter().all(|old_field| {
        new_fields
            .get(old_field.name.as_str())
            .is_some_and(|new_field| *new_field == old_field)
    });
    let additions_defaulted = new
        .iter()
        .filter(|field| !old_fields.contains(&field.name.as_str()))
        .all(|field| field.has_default);
    existing_compatible && additions_defaulted
}

fn variants_compatible(old: &[SchemaVariantAbi], new: &[SchemaVariantAbi]) -> bool {
    let new_variants = new
        .iter()
        .map(|variant| (variant.name.as_str(), variant))
        .collect::<BTreeMap<_, _>>();
    old.iter().all(|old_variant| {
        new_variants
            .get(old_variant.name.as_str())
            .is_some_and(|new_variant| {
                old_variant.id == new_variant.id
                    && fields_compatible(&old_variant.fields, &new_variant.fields)
            })
    })
}

fn trait_impls_compatible(old: &[SchemaTraitImplAbi], new: &[SchemaTraitImplAbi]) -> bool {
    let new_traits = new
        .iter()
        .map(|trait_impl| (trait_impl.name.as_str(), trait_impl))
        .collect::<BTreeMap<_, _>>();
    old.iter().all(|old_trait| {
        new_traits
            .get(old_trait.name.as_str())
            .is_some_and(|new_trait| *new_trait == old_trait)
    })
}
