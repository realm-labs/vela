use std::fmt;

use vela_common::PrimitiveTag;
use vela_def::{
    DefId, DefKind, DefPath, FieldId, FunctionId, MethodId, TraitId, TypeId, VariantId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeHintDef {
    pub path: Vec<String>,
    pub args: Vec<TypeHintDef>,
}

impl TypeHintDef {
    #[must_use]
    pub fn new(path: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            path: path.into_iter().map(Into::into).collect(),
            args: Vec::new(),
        }
    }

    #[must_use]
    pub fn named(name: impl Into<String>) -> Self {
        Self::new(canonical_type_hint_path(name.into()))
    }

    #[must_use]
    pub fn with_args(mut self, args: impl IntoIterator<Item = TypeHintDef>) -> Self {
        self.args = args.into_iter().collect();
        self
    }

    #[must_use]
    pub fn display(&self) -> String {
        let path = self.path.join("::");
        if self.args.is_empty() {
            path
        } else {
            let args = self
                .args
                .iter()
                .map(Self::display)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{path}<{args}>")
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        TypeHintParser::new(text).parse()
    }
}

impl fmt::Display for TypeHintDef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.display())
    }
}

impl From<&str> for TypeHintDef {
    fn from(value: &str) -> Self {
        Self::parse(value).unwrap_or_else(|| Self::named(value))
    }
}

impl From<String> for TypeHintDef {
    fn from(value: String) -> Self {
        Self::from(value.as_str())
    }
}

fn canonical_type_hint_path(name: String) -> Vec<String> {
    match name.as_str() {
        "any" => vec!["Any".to_owned()],
        "string" => vec!["String".to_owned()],
        "bytes" => vec!["Bytes".to_owned()],
        "array" => vec!["Array".to_owned()],
        "map" => vec!["Map".to_owned()],
        "set" => vec!["Set".to_owned()],
        "range" => vec!["Range".to_owned()],
        "iterator" => vec!["Iterator".to_owned()],
        "function" => vec!["Function".to_owned()],
        "closure" => vec!["Closure".to_owned()],
        "option" => vec!["Option".to_owned()],
        "result" => vec!["Result".to_owned()],
        _ => name.split("::").map(str::to_owned).collect(),
    }
}

struct TypeHintParser<'a> {
    input: &'a str,
    position: usize,
}

impl<'a> TypeHintParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, position: 0 }
    }

    fn parse(mut self) -> Option<TypeHintDef> {
        let hint = self.parse_hint()?;
        self.skip_ws();
        (self.position == self.input.len()).then_some(hint)
    }

    fn parse_hint(&mut self) -> Option<TypeHintDef> {
        self.skip_ws();
        let path = self.parse_path()?;
        let args = if self.consume('<') {
            let mut args = Vec::new();
            loop {
                args.push(self.parse_hint()?);
                self.skip_ws();
                if self.consume(',') {
                    continue;
                }
                if self.consume('>') {
                    break;
                }
                return None;
            }
            args
        } else {
            Vec::new()
        };
        Some(TypeHintDef {
            path: canonical_type_hint_path(path.join("::")),
            args,
        })
    }

    fn parse_path(&mut self) -> Option<Vec<String>> {
        let mut path = vec![self.parse_segment()?];
        while self.remaining().starts_with("::") {
            self.position += 2;
            path.push(self.parse_segment()?);
        }
        Some(path)
    }

    fn parse_segment(&mut self) -> Option<String> {
        self.skip_ws();
        let start = self.position;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                self.position += ch.len_utf8();
            } else {
                break;
            }
        }
        (self.position > start).then(|| self.input[start..self.position].to_owned())
    }

    fn consume(&mut self, expected: char) -> bool {
        self.skip_ws();
        if self.peek() == Some(expected) {
            self.position += expected.len_utf8();
            true
        } else {
            false
        }
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.peek()
            && ch.is_ascii_whitespace()
        {
            self.position += ch.len_utf8();
        }
    }

    fn peek(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    fn remaining(&self) -> &'a str {
        &self.input[self.position..]
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Def {
    Function(FunctionDef),
    Method(MethodDef),
    Type(TypeDef),
    Field(FieldDef),
    Variant(VariantDef),
    Trait(TraitDef),
}

impl Def {
    #[must_use]
    pub const fn kind(&self) -> DefKind {
        match self {
            Self::Function(_) => DefKind::Function,
            Self::Method(_) => DefKind::Method,
            Self::Type(_) => DefKind::Type,
            Self::Field(_) => DefKind::Field,
            Self::Variant(_) => DefKind::Variant,
            Self::Trait(_) => DefKind::Trait,
        }
    }

    #[must_use]
    pub const fn id(&self) -> DefId {
        match self {
            Self::Function(def) => def.id.def_id(),
            Self::Method(def) => def.id.def_id(),
            Self::Type(def) => def.id.def_id(),
            Self::Field(def) => def.id.def_id(),
            Self::Variant(def) => def.id.def_id(),
            Self::Trait(def) => def.id.def_id(),
        }
    }

    #[must_use]
    pub const fn path(&self) -> &DefPath {
        match self {
            Self::Function(def) => &def.path,
            Self::Method(def) => &def.path,
            Self::Type(def) => &def.path,
            Self::Field(def) => &def.path,
            Self::Variant(def) => &def.path,
            Self::Trait(def) => &def.path,
        }
    }

    #[must_use]
    pub fn semantic_key(&self) -> SemanticKey {
        match self {
            Self::Function(def) => def.semantic_key.clone(),
            Self::Method(def) => def.semantic_key.clone(),
            Self::Type(def) => def.semantic_key.clone(),
            Self::Field(def) => def.semantic_key.clone(),
            Self::Variant(def) => def.semantic_key.clone(),
            Self::Trait(def) => def.semantic_key.clone(),
        }
    }

    #[must_use]
    pub const fn function_id(&self) -> Option<FunctionId> {
        match self {
            Self::Function(def) => Some(def.id),
            Self::Method(_)
            | Self::Type(_)
            | Self::Field(_)
            | Self::Variant(_)
            | Self::Trait(_) => None,
        }
    }

    #[must_use]
    pub const fn method_id(&self) -> Option<MethodId> {
        match self {
            Self::Method(def) => Some(def.id),
            Self::Function(_)
            | Self::Type(_)
            | Self::Field(_)
            | Self::Variant(_)
            | Self::Trait(_) => None,
        }
    }

    #[must_use]
    pub const fn type_id(&self) -> Option<TypeId> {
        match self {
            Self::Type(def) => Some(def.id),
            Self::Function(_)
            | Self::Method(_)
            | Self::Field(_)
            | Self::Variant(_)
            | Self::Trait(_) => None,
        }
    }

    #[must_use]
    pub const fn field_id(&self) -> Option<FieldId> {
        match self {
            Self::Field(def) => Some(def.id),
            Self::Function(_)
            | Self::Method(_)
            | Self::Type(_)
            | Self::Variant(_)
            | Self::Trait(_) => None,
        }
    }

    #[must_use]
    pub const fn variant_id(&self) -> Option<VariantId> {
        match self {
            Self::Variant(def) => Some(def.id),
            Self::Function(_)
            | Self::Method(_)
            | Self::Type(_)
            | Self::Field(_)
            | Self::Trait(_) => None,
        }
    }

    #[must_use]
    pub const fn function_signature(&self) -> Option<&FunctionSignature> {
        match self {
            Self::Function(def) => Some(&def.signature),
            Self::Method(_)
            | Self::Type(_)
            | Self::Field(_)
            | Self::Variant(_)
            | Self::Trait(_) => None,
        }
    }

    #[must_use]
    pub const fn method_signature(&self) -> Option<&FunctionSignature> {
        match self {
            Self::Method(def) => Some(&def.signature),
            Self::Function(_)
            | Self::Type(_)
            | Self::Field(_)
            | Self::Variant(_)
            | Self::Trait(_) => None,
        }
    }

    #[must_use]
    pub const fn type_host_runtime_id(&self) -> Option<u128> {
        match self {
            Self::Type(def) => def.host_runtime_id,
            Self::Function(_)
            | Self::Method(_)
            | Self::Field(_)
            | Self::Variant(_)
            | Self::Trait(_) => None,
        }
    }

    #[must_use]
    pub const fn type_primitive_tag(&self) -> Option<PrimitiveTag> {
        match self {
            Self::Type(def) => def.primitive,
            Self::Function(_)
            | Self::Method(_)
            | Self::Field(_)
            | Self::Variant(_)
            | Self::Trait(_) => None,
        }
    }

    #[must_use]
    pub const fn host_method_runtime_id(&self) -> Option<u128> {
        match self {
            Self::Method(def) => def.host_runtime_id,
            Self::Function(_)
            | Self::Type(_)
            | Self::Field(_)
            | Self::Variant(_)
            | Self::Trait(_) => None,
        }
    }

    #[must_use]
    pub const fn field_writable(&self) -> Option<bool> {
        match self {
            Self::Field(def) => Some(def.writable),
            Self::Function(_)
            | Self::Method(_)
            | Self::Type(_)
            | Self::Variant(_)
            | Self::Trait(_) => None,
        }
    }

    #[must_use]
    pub fn field_type_hint(&self) -> Option<&TypeHintDef> {
        match self {
            Self::Field(def) => def.type_hint.as_ref(),
            Self::Function(_)
            | Self::Method(_)
            | Self::Type(_)
            | Self::Variant(_)
            | Self::Trait(_) => None,
        }
    }

    #[must_use]
    pub const fn field_host_runtime_id(&self) -> Option<u128> {
        match self {
            Self::Field(def) => def.host_runtime_id,
            Self::Function(_)
            | Self::Method(_)
            | Self::Type(_)
            | Self::Variant(_)
            | Self::Trait(_) => None,
        }
    }

    #[must_use]
    pub const fn field_is_variant_field(&self) -> Option<bool> {
        match self {
            Self::Field(def) => Some(def.variant_field),
            Self::Function(_)
            | Self::Method(_)
            | Self::Type(_)
            | Self::Variant(_)
            | Self::Trait(_) => None,
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticKey {
    Function {
        package: String,
        module: Vec<String>,
        name: String,
    },
    Method {
        owner: TypeId,
        name: String,
    },
    Type {
        package: String,
        module: Vec<String>,
        name: String,
    },
    Field {
        owner: TypeId,
        name: String,
    },
    Variant {
        owner: TypeId,
        name: String,
    },
    Trait {
        package: String,
        module: Vec<String>,
        name: String,
    },
}

impl SemanticKey {
    #[must_use]
    pub fn from_path(path: &DefPath) -> Self {
        match path.kind {
            DefKind::Function => Self::Function {
                package: path.package.clone(),
                module: path.module.clone(),
                name: path.name.clone(),
            },
            DefKind::Method => Self::Method {
                owner: owner_type_id(path),
                name: path.name.clone(),
            },
            DefKind::Type => Self::Type {
                package: path.package.clone(),
                module: path.module.clone(),
                name: path.name.clone(),
            },
            DefKind::Field => Self::Field {
                owner: owner_type_id(path),
                name: path.name.clone(),
            },
            DefKind::Variant => Self::Variant {
                owner: owner_type_id(path),
                name: path.name.clone(),
            },
            DefKind::Trait => Self::Trait {
                package: path.package.clone(),
                module: path.module.clone(),
                name: path.name.clone(),
            },
            DefKind::Module | DefKind::Global => Self::Function {
                package: path.package.clone(),
                module: path.module.clone(),
                name: path.name.clone(),
            },
        }
    }
}

fn owner_type_id(path: &DefPath) -> TypeId {
    let owner = path.owner.clone().unwrap_or_default();
    TypeId::from_def_id(DefPath::ty(&path.package, path.module.clone(), owner).id())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionDef {
    pub id: FunctionId,
    pub path: DefPath,
    pub semantic_key: SemanticKey,
    pub signature: FunctionSignature,
    pub effects: EffectSet,
}

impl FunctionDef {
    #[must_use]
    pub fn new(path: DefPath, signature: FunctionSignature) -> Self {
        let id = FunctionId::from_def_id(path.id());
        let semantic_key = SemanticKey::from_path(&path);
        Self {
            id,
            path,
            semantic_key,
            signature,
            effects: EffectSet::default(),
        }
    }

    #[must_use]
    pub fn with_id(mut self, id: FunctionId) -> Self {
        self.id = id;
        self
    }

    #[must_use]
    pub fn effects(mut self, effects: EffectSet) -> Self {
        self.effects = effects;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodDef {
    pub id: MethodId,
    pub path: DefPath,
    pub semantic_key: SemanticKey,
    pub owner: TypeId,
    pub signature: FunctionSignature,
    pub effects: EffectSet,
    pub host_runtime_id: Option<u128>,
}

impl MethodDef {
    #[must_use]
    pub fn new(path: DefPath, owner: TypeId, signature: FunctionSignature) -> Self {
        let id = MethodId::from_def_id(path.id());
        let name = path.name.clone();
        Self {
            id,
            path,
            semantic_key: SemanticKey::Method { owner, name },
            owner,
            signature,
            effects: EffectSet::default(),
            host_runtime_id: None,
        }
    }

    #[must_use]
    pub fn with_id(mut self, id: MethodId) -> Self {
        self.id = id;
        self
    }

    #[must_use]
    pub fn effects(mut self, effects: EffectSet) -> Self {
        self.effects = effects;
        self
    }

    #[must_use]
    pub const fn host_runtime_id(mut self, id: u128) -> Self {
        self.host_runtime_id = Some(id);
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeDef {
    pub id: TypeId,
    pub path: DefPath,
    pub semantic_key: SemanticKey,
    pub primitive: Option<PrimitiveTag>,
    pub host_runtime_id: Option<u128>,
}

impl TypeDef {
    #[must_use]
    pub fn new(path: DefPath) -> Self {
        let id = TypeId::from_def_id(path.id());
        let semantic_key = SemanticKey::from_path(&path);
        Self {
            id,
            path,
            semantic_key,
            primitive: None,
            host_runtime_id: None,
        }
    }

    #[must_use]
    pub fn primitive(path: DefPath, primitive: PrimitiveTag) -> Self {
        Self::new(path).primitive_tag(primitive)
    }

    #[must_use]
    pub fn with_id(mut self, id: TypeId) -> Self {
        self.id = id;
        self
    }

    #[must_use]
    pub const fn host_runtime_id(mut self, id: u128) -> Self {
        self.host_runtime_id = Some(id);
        self
    }

    #[must_use]
    pub const fn primitive_tag(mut self, primitive: PrimitiveTag) -> Self {
        self.primitive = Some(primitive);
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldDef {
    pub id: FieldId,
    pub path: DefPath,
    pub semantic_key: SemanticKey,
    pub owner: TypeId,
    pub writable: bool,
    pub type_hint: Option<TypeHintDef>,
    pub host_runtime_id: Option<u128>,
    pub variant_field: bool,
}

impl FieldDef {
    #[must_use]
    pub fn new(path: DefPath, owner: TypeId) -> Self {
        let id = FieldId::from_def_id(path.id());
        let name = path.name.clone();
        Self {
            id,
            path,
            semantic_key: SemanticKey::Field { owner, name },
            owner,
            writable: true,
            type_hint: None,
            host_runtime_id: None,
            variant_field: false,
        }
    }

    #[must_use]
    pub fn with_id(mut self, id: FieldId) -> Self {
        self.id = id;
        self
    }

    #[must_use]
    pub const fn writable(mut self, writable: bool) -> Self {
        self.writable = writable;
        self
    }

    #[must_use]
    pub fn type_hint(mut self, type_hint: Option<impl Into<TypeHintDef>>) -> Self {
        self.type_hint = type_hint.map(Into::into);
        self
    }

    #[must_use]
    pub const fn host_runtime_id(mut self, id: u128) -> Self {
        self.host_runtime_id = Some(id);
        self
    }

    #[must_use]
    pub const fn variant_field(mut self, variant_field: bool) -> Self {
        self.variant_field = variant_field;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantDef {
    pub id: VariantId,
    pub path: DefPath,
    pub semantic_key: SemanticKey,
    pub owner: TypeId,
}

impl VariantDef {
    #[must_use]
    pub fn new(path: DefPath, owner: TypeId) -> Self {
        let id = VariantId::from_def_id(path.id());
        let name = path.name.clone();
        Self {
            id,
            path,
            semantic_key: SemanticKey::Variant { owner, name },
            owner,
        }
    }

    #[must_use]
    pub fn with_id(mut self, id: VariantId) -> Self {
        self.id = id;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitDef {
    pub id: TraitId,
    pub path: DefPath,
    pub semantic_key: SemanticKey,
}

impl TraitDef {
    #[must_use]
    pub fn new(path: DefPath) -> Self {
        let id = TraitId::from_def_id(path.id());
        let semantic_key = SemanticKey::from_path(&path);
        Self {
            id,
            path,
            semantic_key,
        }
    }

    #[must_use]
    pub fn with_id(mut self, id: TraitId) -> Self {
        self.id = id;
        self
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FunctionSignature {
    pub params: Vec<ParamDef>,
    pub return_type: Option<TypeHintDef>,
}

impl FunctionSignature {
    #[must_use]
    pub fn new(
        params: impl IntoIterator<Item = ParamDef>,
        return_type: Option<impl Into<TypeHintDef>>,
    ) -> Self {
        Self {
            params: params.into_iter().collect(),
            return_type: return_type.map(Into::into),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamDef {
    pub name: String,
    pub type_hint: Option<TypeHintDef>,
    pub has_default: bool,
}

impl ParamDef {
    #[must_use]
    pub fn new(name: impl Into<String>, type_hint: Option<impl Into<TypeHintDef>>) -> Self {
        Self {
            name: name.into(),
            type_hint: type_hint.map(Into::into),
            has_default: false,
        }
    }

    #[must_use]
    pub const fn defaulted(mut self, has_default: bool) -> Self {
        self.has_default = has_default;
        self
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EffectSet {
    pub host_read: bool,
    pub host_write: bool,
    pub reflection_read: bool,
    pub reflection_call: bool,
    pub event_emit: bool,
    pub time: bool,
    pub random: bool,
    pub io_read: bool,
    pub io_write: bool,
}
