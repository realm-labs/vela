//! Central definition registry for semantic definitions.

mod defs;

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use vela_common::PrimitiveTag;
use vela_def::{
    DefId, DefKind, DefPath, FieldId, FunctionId, MethodId, TraitId, TypeId, VariantId,
};

pub use defs::{
    Def, EffectSet, FieldDef, FunctionDef, FunctionSignature, MethodDef, ParamDef, SemanticKey,
    TraitDef, TypeDef, VariantDef,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DefinitionRegistry {
    defs_by_id: BTreeMap<DefId, Def>,
    ids_by_path: BTreeMap<DefPath, DefId>,
    ids_by_semantic_key: BTreeMap<SemanticKey, DefId>,
    primitive_type_ids: BTreeMap<PrimitiveTag, TypeId>,
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

    #[must_use]
    pub const fn compile_view(&self) -> RegistryCompileView<'_> {
        RegistryCompileView { registry: self }
    }

    pub fn insert(&mut self, def: Def) -> Result<DefId, RegistryError> {
        let id = def.id();
        let path = def.path().clone();
        let semantic_key = def.semantic_key();
        let primitive = def.type_primitive_tag();

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

        if let Some(primitive) = primitive
            && let Some(existing_id) = self.primitive_type_ids.get(&primitive)
        {
            let existing_path = self
                .defs_by_id
                .get(&existing_id.def_id())
                .map(|def| def.path().clone())
                .unwrap_or_else(|| path.clone());
            return Err(RegistryError::DuplicatePrimitiveType {
                primitive,
                existing: Box::new(existing_path),
                incoming: Box::new(path),
            });
        }

        self.ids_by_path.insert(path, id);
        self.ids_by_semantic_key.insert(semantic_key, id);
        if let Some(primitive) = primitive {
            let type_id = def
                .type_id()
                .expect("primitive metadata is only attached to type definitions");
            self.primitive_type_ids.insert(primitive, type_id);
        }
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

    #[must_use]
    pub fn type_primitive_kind(&self, id: TypeId) -> Option<PrimitiveTag> {
        self.get(id.def_id()).and_then(Def::type_primitive_tag)
    }

    #[must_use]
    pub fn primitive_type_id(&self, primitive: PrimitiveTag) -> Option<TypeId> {
        self.primitive_type_ids.get(&primitive).copied()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RegistryCompileView<'registry> {
    registry: &'registry DefinitionRegistry,
}

impl<'registry> RegistryCompileView<'registry> {
    #[must_use]
    pub fn resolve_native_function_path(&self, path: &DefPath) -> Option<FunctionId> {
        self.registry.get_by_path(path).and_then(Def::function_id)
    }

    #[must_use]
    pub fn resolve_native_function_name(&self, name: &str) -> Option<FunctionId> {
        self.native_function_by_source_name(name).map(|def| def.id)
    }

    #[must_use]
    pub fn native_function_params_by_name(&self, name: &str) -> Option<&'registry [ParamDef]> {
        self.native_function_by_source_name(name)
            .map(|def| def.signature.params.as_slice())
    }

    #[must_use]
    pub fn has_native_module_root(&self, root: &str) -> bool {
        self.registry.defs_by_id.values().any(|def| {
            let Def::Function(function) = def else {
                return false;
            };
            function
                .path
                .module
                .first()
                .is_some_and(|module| module == root)
        })
    }

    #[must_use]
    pub fn resolve_value_method(&self, owner: TypeId, name: &str) -> Option<MethodId> {
        self.resolve_method(owner, name)
    }

    #[must_use]
    pub fn resolve_host_method(&self, owner: TypeId, name: &str) -> Option<MethodId> {
        self.resolve_method(owner, name)
    }

    #[must_use]
    pub fn host_method_runtime_id(&self, id: MethodId) -> Option<u128> {
        self.registry
            .get(id.def_id())
            .and_then(Def::host_method_runtime_id)
    }

    #[must_use]
    pub fn resolve_host_field(&self, owner: TypeId, name: &str) -> Option<FieldId> {
        let key = SemanticKey::Field {
            owner,
            name: name.to_owned(),
        };
        self.registry
            .id_for_semantic_key(&key)
            .and_then(|id| self.registry.get(id))
            .and_then(Def::field_id)
    }

    #[must_use]
    pub fn field_writable(&self, id: FieldId) -> Option<bool> {
        self.registry.get(id.def_id()).and_then(Def::field_writable)
    }

    #[must_use]
    pub fn field_type_hint(&self, id: FieldId) -> Option<&'registry str> {
        self.registry
            .get(id.def_id())
            .and_then(Def::field_type_hint)
    }

    #[must_use]
    pub fn field_host_runtime_id(&self, id: FieldId) -> Option<u128> {
        self.registry
            .get(id.def_id())
            .and_then(Def::field_host_runtime_id)
    }

    #[must_use]
    pub fn field_is_variant_field(&self, id: FieldId) -> Option<bool> {
        self.registry
            .get(id.def_id())
            .and_then(Def::field_is_variant_field)
    }

    #[must_use]
    pub fn resolve_type(&self, path: &DefPath) -> Option<TypeId> {
        self.registry.get_by_path(path).and_then(Def::type_id)
    }

    #[must_use]
    pub fn type_names_for_package(&self, package: &str) -> Vec<&'registry str> {
        self.registry
            .defs_by_id
            .values()
            .filter_map(|def| {
                let Def::Type(ty) = def else {
                    return None;
                };
                (ty.path.package == package).then_some(ty.path.name.as_str())
            })
            .collect()
    }

    #[must_use]
    pub fn type_host_runtime_id(&self, id: TypeId) -> Option<u128> {
        self.registry
            .get(id.def_id())
            .and_then(Def::type_host_runtime_id)
    }

    #[must_use]
    pub fn type_primitive_kind(&self, id: TypeId) -> Option<PrimitiveTag> {
        self.registry.type_primitive_kind(id)
    }

    #[must_use]
    pub fn primitive_type_id(&self, primitive: PrimitiveTag) -> Option<TypeId> {
        self.registry.primitive_type_id(primitive)
    }

    #[must_use]
    pub fn function_params(&self, id: FunctionId) -> Option<&'registry [ParamDef]> {
        self.registry
            .get(id.def_id())
            .and_then(Def::function_signature)
            .map(|signature| signature.params.as_slice())
    }

    #[must_use]
    pub fn method_params(&self, id: MethodId) -> Option<&'registry [ParamDef]> {
        self.registry
            .get(id.def_id())
            .and_then(Def::method_signature)
            .map(|signature| signature.params.as_slice())
    }

    #[must_use]
    pub fn host_method_params_by_runtime_id(
        &self,
        runtime_id: u128,
    ) -> Option<&'registry [ParamDef]> {
        self.registry.defs_by_id.values().find_map(|def| {
            let Def::Method(method) = def else {
                return None;
            };
            (method.host_runtime_id == Some(runtime_id))
                .then_some(method.signature.params.as_slice())
        })
    }

    fn resolve_method(&self, owner: TypeId, name: &str) -> Option<MethodId> {
        let key = SemanticKey::Method {
            owner,
            name: name.to_owned(),
        };
        self.registry
            .id_for_semantic_key(&key)
            .and_then(|id| self.registry.get(id))
            .and_then(Def::method_id)
    }

    fn native_function_by_source_name(&self, name: &str) -> Option<&'registry FunctionDef> {
        let source = SourceFunctionName::parse(name)?;
        let mut matches = self.registry.defs_by_id.values().filter_map(|def| {
            let Def::Function(function) = def else {
                return None;
            };
            source.matches(&function.path).then_some(function)
        });
        let first = matches.next()?;
        matches.next().is_none().then_some(first)
    }
}

struct SourceFunctionName<'name> {
    module: Vec<&'name str>,
    name: &'name str,
}

impl<'name> SourceFunctionName<'name> {
    fn parse(name: &'name str) -> Option<Self> {
        let parts = name
            .split("::")
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        let (name, module) = parts.split_last()?;
        Some(Self {
            module: module.to_vec(),
            name: *name,
        })
    }

    fn matches(&self, path: &DefPath) -> bool {
        path.kind == DefKind::Function
            && path.name.as_str() == self.name
            && path
                .module
                .iter()
                .map(String::as_str)
                .eq(self.module.iter().copied())
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
    DuplicatePrimitiveType {
        primitive: PrimitiveTag,
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
            Self::DuplicatePrimitiveType {
                primitive,
                existing,
                incoming,
            } => write!(
                formatter,
                "duplicate primitive type {primitive}; existing path {existing}, incoming path {incoming}"
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

    fn primitive_type_def(name: &str, primitive: PrimitiveTag) -> TypeDef {
        TypeDef::primitive(DefPath::ty("script", ["primitive"], name), primitive)
    }

    fn int_param(name: &str) -> ParamDef {
        ParamDef::new(name, Some("i64"))
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
    fn primitive_type_metadata_registers_and_queries_by_type_and_tag() {
        let mut registry = DefinitionRegistry::new();
        let primitive_defs = [
            primitive_type_def("i64", PrimitiveTag::I64),
            primitive_type_def("u8", PrimitiveTag::U8),
            primitive_type_def("f32", PrimitiveTag::F32),
            primitive_type_def("bytes", PrimitiveTag::Bytes),
        ];

        let ids = primitive_defs
            .into_iter()
            .map(|def| {
                let primitive = def.primitive.expect("test primitive should be set");
                let id = registry
                    .register_type(def)
                    .expect("primitive registration should succeed");
                (primitive, id)
            })
            .collect::<Vec<_>>();

        for (primitive, id) in ids {
            assert_eq!(registry.type_primitive_kind(id), Some(primitive));
            assert_eq!(registry.primitive_type_id(primitive), Some(id));
        }

        let view = registry.compile_view();
        assert_eq!(
            view.primitive_type_id(PrimitiveTag::Bytes)
                .and_then(|id| view.type_primitive_kind(id)),
            Some(PrimitiveTag::Bytes)
        );
        assert_eq!(view.primitive_type_id(PrimitiveTag::I16), None);
    }

    #[test]
    fn duplicate_primitive_type_tag_is_rejected() {
        let mut registry = DefinitionRegistry::new();
        registry
            .register_type(primitive_type_def("i64", PrimitiveTag::I64))
            .expect("initial primitive registration should succeed");

        let error = registry
            .register_type(primitive_type_def("script_i64", PrimitiveTag::I64))
            .expect_err("duplicate primitive tag should be rejected");

        assert!(matches!(
            error,
            RegistryError::DuplicatePrimitiveType {
                primitive: PrimitiveTag::I64,
                ..
            }
        ));
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

    #[test]
    fn compile_view_resolves_function_path_and_params() {
        let mut registry = DefinitionRegistry::new();
        let path = function_path("score");
        let signature = FunctionSignature::new([int_param("amount")], Some("i64".to_owned()));
        let function_id = registry
            .register_function(FunctionDef::new(path.clone(), signature))
            .expect("function registration should succeed");
        let view = registry.compile_view();

        assert_eq!(view.resolve_native_function_path(&path), Some(function_id));
        assert_eq!(
            view.function_params(function_id),
            Some([int_param("amount")].as_slice())
        );
        assert_eq!(
            view.resolve_native_function_path(&type_def("Player").path),
            None
        );
    }

    #[test]
    fn compile_view_resolves_value_and_host_methods_with_params() {
        let mut registry = DefinitionRegistry::new();
        let owner = registry
            .register_type(type_def("Player"))
            .expect("type registration should succeed");
        let method_path = DefPath::method("script", ["combat"], "Player", "grant_exp");
        let signature = FunctionSignature::new([int_param("amount")], None);
        let method_id = registry
            .register_method(MethodDef::new(method_path, owner, signature))
            .expect("method registration should succeed");
        let view = registry.compile_view();

        assert_eq!(
            view.resolve_value_method(owner, "grant_exp"),
            Some(method_id)
        );
        assert_eq!(
            view.resolve_host_method(owner, "grant_exp"),
            Some(method_id)
        );
        assert_eq!(view.resolve_value_method(owner, "missing"), None);
        assert_eq!(
            view.method_params(method_id),
            Some([int_param("amount")].as_slice())
        );
    }

    #[test]
    fn compile_view_resolves_host_fields_and_types() {
        let mut registry = DefinitionRegistry::new();
        let type_path = DefPath::ty("script", ["combat"], "Player");
        let owner = registry
            .register_type(TypeDef::new(type_path.clone()))
            .expect("type registration should succeed");
        let field_id = registry
            .register_field(FieldDef::new(
                DefPath::field("script", ["combat"], "Player", "level"),
                owner,
            ))
            .expect("field registration should succeed");
        let view = registry.compile_view();

        assert_eq!(view.resolve_type(&type_path), Some(owner));
        assert_eq!(view.resolve_host_field(owner, "level"), Some(field_id));
        assert_eq!(view.resolve_host_field(owner, "missing"), None);
        assert_eq!(view.resolve_type(&function_path("score")), None);
    }
}
