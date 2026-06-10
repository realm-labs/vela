//! Central definition registry for semantic definitions.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use vela_def::{
    DefId, DefKind, DefPath, FieldId, FunctionId, MethodId, TraitId, TypeId, VariantId,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DefinitionRegistry {
    defs_by_id: BTreeMap<DefId, Def>,
    ids_by_path: BTreeMap<DefPath, DefId>,
    ids_by_semantic_key: BTreeMap<SemanticKey, DefId>,
    debug_names: DebugNameTable,
    debug_names_by_def: BTreeMap<DefId, DebugNameId>,
}

impl DefinitionRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.defs_by_id.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.defs_by_id.is_empty()
    }

    pub fn register_function(&mut self, def: FunctionDef) -> Result<FunctionId, RegistryError> {
        let id = def.id;
        self.insert(Def::Function(def)).map(|_| id)
    }

    pub fn register_type(&mut self, def: TypeDef) -> Result<TypeId, RegistryError> {
        let id = def.id;
        self.insert(Def::Type(def)).map(|_| id)
    }

    pub fn register_method(&mut self, def: MethodDef) -> Result<MethodId, RegistryError> {
        let id = def.id;
        self.insert(Def::Method(def)).map(|_| id)
    }

    pub fn register_field(&mut self, def: FieldDef) -> Result<FieldId, RegistryError> {
        let id = def.id;
        self.insert(Def::Field(def)).map(|_| id)
    }

    pub fn register_variant(&mut self, def: VariantDef) -> Result<VariantId, RegistryError> {
        let id = def.id;
        self.insert(Def::Variant(def)).map(|_| id)
    }

    pub fn register_trait(&mut self, def: TraitDef) -> Result<TraitId, RegistryError> {
        let id = def.id;
        self.insert(Def::Trait(def)).map(|_| id)
    }

    pub fn insert(&mut self, def: Def) -> Result<DefId, RegistryError> {
        let id = def.id();
        let path = def.path().clone();
        let semantic_key = def.semantic_key();

        if let Some(existing_id) = self.ids_by_path.get(&path) {
            return Err(RegistryError::DuplicatePath {
                path: Box::new(path),
                existing: *existing_id,
                incoming: id,
            });
        }

        if let Some(existing) = self.defs_by_id.get(&id) {
            return Err(RegistryError::IdCollision {
                id,
                existing: Box::new(existing.path().clone()),
                incoming: Box::new(path),
            });
        }

        if let Some(existing_id) = self.ids_by_semantic_key.get(&semantic_key) {
            let existing_path = self
                .defs_by_id
                .get(existing_id)
                .map(|def| def.path().clone())
                .unwrap_or_else(|| path.clone());
            return Err(RegistryError::DuplicateSemanticKey {
                key: Box::new(semantic_key),
                existing: Box::new(existing_path),
                incoming: Box::new(path),
            });
        }

        self.ids_by_path.insert(path, id);
        self.ids_by_semantic_key.insert(semantic_key, id);
        let debug_name = self.debug_names.intern(def.path().canonical_display());
        self.debug_names_by_def.insert(id, debug_name);
        self.defs_by_id.insert(id, def);
        Ok(id)
    }

    #[must_use]
    pub fn get(&self, id: DefId) -> Option<&Def> {
        self.defs_by_id.get(&id)
    }

    #[must_use]
    pub fn get_by_path(&self, path: &DefPath) -> Option<&Def> {
        self.id_for_path(path).and_then(|id| self.get(id))
    }

    #[must_use]
    pub fn id_for_path(&self, path: &DefPath) -> Option<DefId> {
        self.ids_by_path.get(path).copied()
    }

    #[must_use]
    pub fn id_for_semantic_key(&self, key: &SemanticKey) -> Option<DefId> {
        self.ids_by_semantic_key.get(key).copied()
    }

    #[must_use]
    pub fn debug_name(&self, id: DebugNameId) -> &str {
        self.debug_names.debug_name(id)
    }

    #[must_use]
    pub fn debug_name_for_def(&self, id: DefId) -> DebugNameId {
        self.debug_names_by_def[&id]
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct DebugNameId(u32);

impl DebugNameId {
    #[must_use]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    fn as_usize(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DebugNameTable {
    names: Vec<String>,
    ids_by_name: BTreeMap<String, DebugNameId>,
}

impl DebugNameTable {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.names.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    pub fn intern(&mut self, name: impl Into<String>) -> DebugNameId {
        let name = name.into();
        if let Some(id) = self.ids_by_name.get(&name) {
            return *id;
        }

        let next = self.names.len();
        assert!(
            u32::try_from(next).is_ok(),
            "debug name table exceeds u32::MAX entries"
        );
        let id = DebugNameId::new(next as u32);
        self.names.push(name.clone());
        self.ids_by_name.insert(name, id);
        id
    }

    #[must_use]
    pub fn debug_name(&self, id: DebugNameId) -> &str {
        &self.names[id.as_usize()]
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeDef {
    pub id: TypeId,
    pub path: DefPath,
    pub semantic_key: SemanticKey,
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
        }
    }

    #[must_use]
    pub fn with_id(mut self, id: TypeId) -> Self {
        self.id = id;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldDef {
    pub id: FieldId,
    pub path: DefPath,
    pub semantic_key: SemanticKey,
    pub owner: TypeId,
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
        }
    }

    #[must_use]
    pub fn with_id(mut self, id: FieldId) -> Self {
        self.id = id;
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
    pub return_type: Option<String>,
}

impl FunctionSignature {
    #[must_use]
    pub fn new(params: impl IntoIterator<Item = ParamDef>, return_type: Option<String>) -> Self {
        Self {
            params: params.into_iter().collect(),
            return_type,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamDef {
    pub name: String,
    pub type_hint: Option<String>,
}

impl ParamDef {
    #[must_use]
    pub fn new(name: impl Into<String>, type_hint: Option<impl Into<String>>) -> Self {
        Self {
            name: name.into(),
            type_hint: type_hint.map(Into::into),
        }
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryError {
    DuplicatePath {
        path: Box<DefPath>,
        existing: DefId,
        incoming: DefId,
    },
    DuplicateSemanticKey {
        key: Box<SemanticKey>,
        existing: Box<DefPath>,
        incoming: Box<DefPath>,
    },
    IdCollision {
        id: DefId,
        existing: Box<DefPath>,
        incoming: Box<DefPath>,
    },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicatePath {
                path,
                existing,
                incoming,
            } => write!(
                formatter,
                "duplicate definition path {path}; existing id {existing:?}, incoming id {incoming:?}"
            ),
            Self::DuplicateSemanticKey {
                key,
                existing,
                incoming,
            } => write!(
                formatter,
                "duplicate semantic key {key:?}; existing path {existing}, incoming path {incoming}"
            ),
            Self::IdCollision {
                id,
                existing,
                incoming,
            } => write!(
                formatter,
                "definition id collision {id:?}; existing path {existing}, incoming path {incoming}"
            ),
        }
    }
}

impl Error for RegistryError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn function_path(name: &str) -> DefPath {
        DefPath::function("script", ["combat"], name)
    }

    fn type_def(name: &str) -> TypeDef {
        TypeDef::new(DefPath::ty("script", ["combat"], name))
    }

    #[test]
    fn registry_lookup_by_path_and_id_works() {
        let mut registry = DefinitionRegistry::new();
        let path = function_path("score");
        let def = FunctionDef::new(path.clone(), FunctionSignature::default());
        let id = registry
            .register_function(def)
            .expect("function registration should succeed");

        assert_eq!(registry.id_for_path(&path), Some(id.def_id()));
        assert_eq!(registry.get_by_path(&path).map(Def::id), Some(id.def_id()));
        assert_eq!(registry.get(id.def_id()).map(Def::path), Some(&path));
    }

    #[test]
    fn duplicate_path_is_rejected() {
        let mut registry = DefinitionRegistry::new();
        let path = function_path("score");

        registry
            .register_function(FunctionDef::new(path.clone(), FunctionSignature::default()))
            .expect("initial function registration should succeed");

        let error = registry
            .register_function(FunctionDef::new(path.clone(), FunctionSignature::default()))
            .expect_err("duplicate path should be rejected");

        assert!(matches!(
            error,
            RegistryError::DuplicatePath { path: failed, .. } if *failed == path
        ));
    }

    #[test]
    fn duplicate_semantic_key_is_rejected() {
        let mut registry = DefinitionRegistry::new();
        let owner = type_def("Player").id;

        let first = MethodDef::new(
            DefPath::method("script", ["combat"], "Player", "score"),
            owner,
            FunctionSignature::default(),
        );
        let second = MethodDef::new(
            DefPath::method("script", ["hud"], "Player", "score"),
            owner,
            FunctionSignature::default(),
        );

        registry
            .register_method(first)
            .expect("initial method registration should succeed");
        let error = registry
            .register_method(second)
            .expect_err("duplicate semantic key should be rejected");

        assert!(matches!(error, RegistryError::DuplicateSemanticKey { .. }));
    }

    #[test]
    fn id_collision_with_different_path_is_rejected() {
        let mut registry = DefinitionRegistry::new();
        let first = FunctionDef::new(function_path("score"), FunctionSignature::default());
        let colliding_id = first.id;
        let second = FunctionDef::new(function_path("award"), FunctionSignature::default())
            .with_id(colliding_id);

        registry
            .register_function(first)
            .expect("initial function registration should succeed");
        let error = registry
            .register_function(second)
            .expect_err("id collision should be rejected");

        assert!(matches!(error, RegistryError::IdCollision { .. }));
    }

    #[test]
    fn field_and_variant_registration_use_owner_semantic_keys() {
        let mut registry = DefinitionRegistry::new();
        let owner = registry
            .register_type(type_def("Result"))
            .expect("type registration should succeed");
        let ok_variant = DefPath::variant("script", ["combat"], "Result", "Ok");
        let value_field = DefPath::field("script", ["combat"], "Result::Ok", "value");

        let variant_id = registry
            .register_variant(VariantDef::new(ok_variant.clone(), owner))
            .expect("variant registration should succeed");
        let field_id = registry
            .register_field(FieldDef::new(value_field.clone(), owner))
            .expect("field registration should succeed");

        assert_eq!(registry.id_for_path(&ok_variant), Some(variant_id.def_id()));
        assert_eq!(registry.id_for_path(&value_field), Some(field_id.def_id()));
    }

    #[test]
    fn debug_name_table_interns_names_with_stable_instance_ids() {
        let mut table = DebugNameTable::new();

        let first = table.intern("function script::combat::score");
        let second = table.intern("type script::combat::Player");
        let repeated = table.intern("function script::combat::score");

        assert_eq!(first, repeated);
        assert_ne!(first, second);
        assert_eq!(first.get(), 0);
        assert_eq!(second.get(), 1);
        assert_eq!(table.debug_name(first), "function script::combat::score");
        assert_eq!(table.debug_name(second), "type script::combat::Player");
    }

    #[test]
    fn registry_assigns_debug_names_for_definitions() {
        let mut registry = DefinitionRegistry::new();
        let path = function_path("score");
        let id = registry
            .register_function(FunctionDef::new(path.clone(), FunctionSignature::default()))
            .expect("function registration should succeed");

        let debug_name_id = registry.debug_name_for_def(id.def_id());

        assert_eq!(debug_name_id.get(), 0);
        assert_eq!(
            registry.debug_name(debug_name_id),
            "function script::combat::score"
        );
    }

    #[test]
    fn registry_debug_name_ids_are_stable_inside_registry_instance() {
        let mut registry = DefinitionRegistry::new();
        let score = registry
            .register_function(FunctionDef::new(
                function_path("score"),
                FunctionSignature::default(),
            ))
            .expect("score function registration should succeed");
        let award = registry
            .register_function(FunctionDef::new(
                function_path("award"),
                FunctionSignature::default(),
            ))
            .expect("award function registration should succeed");

        let score_debug_name = registry.debug_name_for_def(score.def_id());
        let award_debug_name = registry.debug_name_for_def(award.def_id());

        assert_eq!(
            registry.debug_name_for_def(score.def_id()),
            score_debug_name
        );
        assert_eq!(
            registry.debug_name_for_def(award.def_id()),
            award_debug_name
        );
        assert_ne!(score_debug_name, award_debug_name);
        assert_eq!(score_debug_name.get(), 0);
        assert_eq!(award_debug_name.get(), 1);
    }
}
