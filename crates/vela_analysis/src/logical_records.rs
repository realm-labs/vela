//! Stable facts for standard-library records materialized as script values.
//!
//! These records have runtime/source layout names and stable semantic IDs, but
//! they are not source declarations or registry-owned schemas. Keeping their
//! logical layouts in analysis lets MIR consume exact field facts without
//! inheriting bytecode slots or rebuilding compiler-local shape flow.

use vela_common::{ShapeId, script_shape_id};
use vela_def::{DefPath, FieldId, TypeId};

use crate::type_fact::TypeFact;

const LOGICAL_RECORD_PACKAGE: &str = "std";
const LOGICAL_RECORD_MODULE: [&str; 1] = ["value_records"];

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LogicalRecordKind {
    MapEntry,
    ReflectEffectSet,
    ReflectField,
    ReflectFieldAccess,
    ReflectFunction,
    ReflectFunctionAccess,
    ReflectMethod,
    ReflectMethodAccess,
    ReflectModule,
    ReflectParam,
    ReflectSourceSpan,
    ReflectState,
    ReflectTrait,
    ReflectTraitMethod,
    ReflectType,
    ReflectTypeHint,
    ReflectVariant,
}

impl LogicalRecordKind {
    pub const ALL: [Self; 17] = [
        Self::MapEntry,
        Self::ReflectEffectSet,
        Self::ReflectField,
        Self::ReflectFieldAccess,
        Self::ReflectFunction,
        Self::ReflectFunctionAccess,
        Self::ReflectMethod,
        Self::ReflectMethodAccess,
        Self::ReflectModule,
        Self::ReflectParam,
        Self::ReflectSourceSpan,
        Self::ReflectState,
        Self::ReflectTrait,
        Self::ReflectTraitMethod,
        Self::ReflectType,
        Self::ReflectTypeHint,
        Self::ReflectVariant,
    ];

    #[must_use]
    pub const fn runtime_name(self) -> &'static str {
        match self {
            Self::MapEntry => "MapEntry",
            Self::ReflectEffectSet => "ReflectEffectSet",
            Self::ReflectField => "ReflectField",
            Self::ReflectFieldAccess => "ReflectFieldAccess",
            Self::ReflectFunction => "ReflectFunction",
            Self::ReflectFunctionAccess => "ReflectFunctionAccess",
            Self::ReflectMethod => "ReflectMethod",
            Self::ReflectMethodAccess => "ReflectMethodAccess",
            Self::ReflectModule => "ReflectModule",
            Self::ReflectParam => "ReflectParam",
            Self::ReflectSourceSpan => "ReflectSourceSpan",
            Self::ReflectState => "ReflectState",
            Self::ReflectTrait => "ReflectTrait",
            Self::ReflectTraitMethod => "ReflectTraitMethod",
            Self::ReflectType => "ReflectType",
            Self::ReflectTypeHint => "ReflectTypeHint",
            Self::ReflectVariant => "ReflectVariant",
        }
    }

    #[must_use]
    pub fn type_id(self) -> TypeId {
        TypeId::from_def_id(self.type_path().id())
    }

    #[must_use]
    pub fn canonical_name(self) -> String {
        self.type_path().canonical_name()
    }

    fn type_path(self) -> DefPath {
        DefPath::ty(
            LOGICAL_RECORD_PACKAGE,
            LOGICAL_RECORD_MODULE,
            self.runtime_name(),
        )
    }

    #[must_use]
    pub fn from_type_id(type_id: TypeId) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.type_id() == type_id)
    }

    /// Resolves the source constructor surface owned by the logical-record
    /// manifest. Binding resolution must classify the path as dynamic before
    /// callers use this lookup, so source declarations always take priority.
    #[must_use]
    pub fn from_source_constructor_path(path: &[String]) -> Option<Self> {
        let [name] = path else {
            return None;
        };
        (name == Self::MapEntry.runtime_name()).then_some(Self::MapEntry)
    }

    #[must_use]
    pub fn field_id(self, field: &str) -> FieldId {
        FieldId::from_def_id(
            DefPath::field(
                LOGICAL_RECORD_PACKAGE,
                LOGICAL_RECORD_MODULE,
                self.runtime_name(),
                field,
            )
            .id(),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicalRecordFieldFact {
    id: FieldId,
    name: String,
    fact: TypeFact,
    canonical_slot: u32,
}

impl LogicalRecordFieldFact {
    #[must_use]
    pub const fn id(&self) -> FieldId {
        self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn fact(&self) -> &TypeFact {
        &self.fact
    }

    #[must_use]
    pub const fn canonical_slot(&self) -> u32 {
        self.canonical_slot
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicalRecordFact {
    kind: LogicalRecordKind,
    type_id: TypeId,
    shape: ShapeId,
    fields: Vec<LogicalRecordFieldFact>,
}

impl LogicalRecordFact {
    #[must_use]
    pub fn fixed(kind: LogicalRecordKind) -> Self {
        assert!(kind != LogicalRecordKind::MapEntry);
        Self::from_fields(kind, fixed_fields(kind))
    }

    #[must_use]
    pub fn map_entry(key: TypeFact, value: TypeFact) -> Self {
        Self::from_fields(
            LogicalRecordKind::MapEntry,
            [("key", key), ("value", value)],
        )
    }

    #[must_use]
    pub fn manifest(kind: LogicalRecordKind) -> Self {
        if kind == LogicalRecordKind::MapEntry {
            Self::map_entry(TypeFact::Any, TypeFact::Any)
        } else {
            Self::fixed(kind)
        }
    }

    fn from_fields(
        kind: LogicalRecordKind,
        fields: impl IntoIterator<Item = (&'static str, TypeFact)>,
    ) -> Self {
        let mut fields = fields
            .into_iter()
            .map(|(name, fact)| (name.to_owned(), fact))
            .collect::<Vec<_>>();
        fields.sort_by(|left, right| left.0.cmp(&right.0));
        assert!(
            fields.windows(2).all(|pair| pair[0].0 != pair[1].0),
            "logical record manifest contains duplicate fields"
        );
        let shape = script_shape_id(
            kind.runtime_name(),
            fields.iter().map(|(name, _)| name.as_str()),
        );
        let fields = fields
            .into_iter()
            .enumerate()
            .map(|(slot, (name, fact))| LogicalRecordFieldFact {
                id: kind.field_id(&name),
                name,
                fact,
                canonical_slot: u32::try_from(slot).expect("logical record field count fits u32"),
            })
            .collect();
        Self {
            kind,
            type_id: kind.type_id(),
            shape,
            fields,
        }
    }

    #[must_use]
    pub const fn kind(&self) -> LogicalRecordKind {
        self.kind
    }

    #[must_use]
    pub const fn runtime_name(&self) -> &'static str {
        self.kind.runtime_name()
    }

    #[must_use]
    pub const fn type_id(&self) -> TypeId {
        self.type_id
    }

    #[must_use]
    pub const fn shape(&self) -> ShapeId {
        self.shape
    }

    pub fn fields(&self) -> impl Iterator<Item = &LogicalRecordFieldFact> {
        self.fields.iter()
    }

    #[must_use]
    pub fn field(&self, name: &str) -> Option<&LogicalRecordFieldFact> {
        self.fields
            .binary_search_by_key(&name, |field| field.name.as_str())
            .ok()
            .map(|index| &self.fields[index])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicalRecordFieldTargetFact {
    pub kind: LogicalRecordKind,
    pub type_id: TypeId,
    pub shape: ShapeId,
    pub field: FieldId,
    pub name: String,
}

impl LogicalRecordFact {
    #[must_use]
    pub fn field_target(&self, name: &str) -> Option<LogicalRecordFieldTargetFact> {
        let field = self.field(name)?;
        Some(LogicalRecordFieldTargetFact {
            kind: self.kind,
            type_id: self.type_id,
            shape: self.shape,
            field: field.id,
            name: field.name.clone(),
        })
    }
}

#[must_use]
pub fn fixed_record(kind: LogicalRecordKind) -> TypeFact {
    TypeFact::logical_record(LogicalRecordFact::fixed(kind))
}

#[must_use]
pub fn map_entry(key: TypeFact, value: TypeFact) -> TypeFact {
    TypeFact::logical_record(LogicalRecordFact::map_entry(key, value))
}

fn fixed_fields(kind: LogicalRecordKind) -> Vec<(&'static str, TypeFact)> {
    use LogicalRecordKind::{
        ReflectEffectSet, ReflectField, ReflectFieldAccess, ReflectFunction, ReflectFunctionAccess,
        ReflectMethod, ReflectMethodAccess, ReflectModule, ReflectParam, ReflectSourceSpan,
        ReflectState, ReflectTrait, ReflectTraitMethod, ReflectType, ReflectTypeHint,
        ReflectVariant,
    };

    let attrs = || TypeFact::map(TypeFact::STRING, TypeFact::STRING);
    let strings = || TypeFact::array(TypeFact::STRING);
    match kind {
        LogicalRecordKind::MapEntry => unreachable!("MapEntry fields require key/value facts"),
        ReflectEffectSet => vec![
            ("calls_reflection", TypeFact::BOOL),
            ("emits_events", TypeFact::BOOL),
            ("reads_host", TypeFact::BOOL),
            ("reads_io", TypeFact::BOOL),
            ("reads_reflection", TypeFact::BOOL),
            ("reads_time", TypeFact::BOOL),
            ("uses_random", TypeFact::BOOL),
            ("writes_host", TypeFact::BOOL),
            ("writes_io", TypeFact::BOOL),
            ("writes_reflection", TypeFact::BOOL),
        ],
        ReflectModule => vec![
            ("attrs", attrs()),
            ("docs", TypeFact::Any),
            ("exports", strings()),
            ("name", TypeFact::STRING),
            ("origin", TypeFact::STRING),
            (
                "source_span",
                TypeFact::option(fixed_record(ReflectSourceSpan)),
            ),
        ],
        ReflectField => vec![
            ("access", fixed_record(ReflectFieldAccess)),
            ("attrs", attrs()),
            ("defaulted", TypeFact::BOOL),
            ("docs", TypeFact::Any),
            ("id", TypeFact::I64),
            ("name", TypeFact::STRING),
            ("origin", TypeFact::STRING),
            ("owner", TypeFact::STRING),
            ("source_span", TypeFact::Any),
            ("type", TypeFact::Any),
            ("type_desc", TypeFact::Any),
            ("writable", TypeFact::BOOL),
        ],
        ReflectFieldAccess => vec![
            ("readable", TypeFact::BOOL),
            ("reflect_readable", TypeFact::BOOL),
            ("reflect_writable", TypeFact::BOOL),
            ("required_permissions", strings()),
            ("writable", TypeFact::BOOL),
        ],
        ReflectFunction => vec![
            ("access", fixed_record(ReflectFunctionAccess)),
            ("attrs", attrs()),
            ("docs", TypeFact::Any),
            ("effects", fixed_record(ReflectEffectSet)),
            ("id", TypeFact::I64),
            ("module", TypeFact::Any),
            ("name", TypeFact::STRING),
            ("origin", TypeFact::STRING),
            ("params", TypeFact::array(fixed_record(ReflectParam))),
            ("public", TypeFact::BOOL),
            ("return", TypeFact::Any),
            ("return_desc", TypeFact::Any),
            ("returns", TypeFact::Any),
            ("returns_desc", TypeFact::Any),
            ("source_span", TypeFact::Any),
        ],
        ReflectFunctionAccess => vec![
            ("public", TypeFact::BOOL),
            ("reflect_callable", TypeFact::BOOL),
            ("reflect_visible", TypeFact::BOOL),
            ("required_permissions", strings()),
        ],
        ReflectMethod => vec![
            ("access", fixed_record(ReflectMethodAccess)),
            ("attrs", attrs()),
            ("docs", TypeFact::Any),
            ("effects", fixed_record(ReflectEffectSet)),
            ("id", TypeFact::I64),
            ("name", TypeFact::STRING),
            ("origin", TypeFact::STRING),
            ("owner", TypeFact::STRING),
            ("params", TypeFact::array(fixed_record(ReflectParam))),
            ("return", TypeFact::Any),
            ("return_desc", TypeFact::Any),
            ("returns", TypeFact::Any),
            ("returns_desc", TypeFact::Any),
            ("source_span", TypeFact::Any),
        ],
        ReflectMethodAccess => vec![
            ("public", TypeFact::BOOL),
            ("reflect_callable", TypeFact::BOOL),
            ("required_permissions", strings()),
        ],
        ReflectParam => vec![
            ("defaulted", TypeFact::BOOL),
            ("name", TypeFact::STRING),
            ("type", TypeFact::Any),
            ("type_desc", TypeFact::Any),
        ],
        ReflectSourceSpan => vec![
            ("end", TypeFact::I64),
            ("source", TypeFact::I64),
            ("start", TypeFact::I64),
        ],
        ReflectTrait => vec![
            ("attrs", attrs()),
            ("docs", TypeFact::Any),
            ("id", TypeFact::I64),
            ("methods", TypeFact::array(fixed_record(ReflectTraitMethod))),
            ("name", TypeFact::STRING),
            ("origin", TypeFact::STRING),
            (
                "source_span",
                TypeFact::option(fixed_record(ReflectSourceSpan)),
            ),
        ],
        ReflectState => vec![
            ("has_initializer", TypeFact::BOOL),
            ("id", TypeFact::I64),
            ("module", TypeFact::Any),
            ("name", TypeFact::STRING),
            ("origin", TypeFact::STRING),
            ("public", TypeFact::BOOL),
            ("source_span", TypeFact::Any),
            ("type", TypeFact::STRING),
            ("type_desc", TypeFact::Any),
        ],
        ReflectTraitMethod => vec![
            ("attrs", attrs()),
            ("defaulted", TypeFact::BOOL),
            ("docs", TypeFact::Any),
            ("id", TypeFact::I64),
            ("name", TypeFact::STRING),
            ("origin", TypeFact::STRING),
            ("owner", TypeFact::STRING),
            ("params", TypeFact::array(fixed_record(ReflectParam))),
            ("return", TypeFact::Any),
            ("return_desc", TypeFact::Any),
            ("returns", TypeFact::Any),
            ("returns_desc", TypeFact::Any),
            ("source_span", TypeFact::Any),
        ],
        ReflectTypeHint => vec![
            ("args", TypeFact::array(TypeFact::Any)),
            ("display", TypeFact::STRING),
            ("kind", TypeFact::STRING),
            ("name", TypeFact::option(TypeFact::STRING)),
        ],
        ReflectType => vec![
            ("attrs", attrs()),
            ("docs", TypeFact::Any),
            ("field_count", TypeFact::I64),
            ("id", TypeFact::I64),
            ("kind", TypeFact::STRING),
            ("method_count", TypeFact::I64),
            ("name", TypeFact::STRING),
            ("origin", TypeFact::STRING),
            ("schema_hash", TypeFact::Any),
            (
                "source_span",
                TypeFact::option(fixed_record(ReflectSourceSpan)),
            ),
            ("trait_count", TypeFact::I64),
            ("variant_count", TypeFact::I64),
        ],
        ReflectVariant => vec![
            ("attrs", attrs()),
            ("docs", TypeFact::Any),
            ("fields", TypeFact::array(fixed_record(ReflectField))),
            ("id", TypeFact::I64),
            ("name", TypeFact::STRING),
            ("origin", TypeFact::STRING),
            ("owner", TypeFact::STRING),
            ("source_span", TypeFact::Any),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_record_ids_and_shapes_are_package_qualified_and_canonical() {
        for kind in LogicalRecordKind::ALL {
            let manifest = LogicalRecordFact::manifest(kind);
            assert_eq!(manifest.type_id(), kind.type_id());
            assert_eq!(manifest.runtime_name(), kind.runtime_name());
            assert_eq!(
                manifest.shape(),
                script_shape_id(
                    kind.runtime_name(),
                    manifest.fields().map(LogicalRecordFieldFact::name)
                )
            );
            for field in manifest.fields() {
                assert_eq!(field.id(), kind.field_id(field.name()));
            }
        }
    }

    #[test]
    fn map_entry_specialization_preserves_key_and_value_facts() {
        let entry = LogicalRecordFact::map_entry(TypeFact::STRING, TypeFact::array(TypeFact::I64));

        assert_eq!(
            entry.field("key").map(LogicalRecordFieldFact::fact),
            Some(&TypeFact::STRING)
        );
        assert_eq!(
            entry.field("value").map(LogicalRecordFieldFact::fact),
            Some(&TypeFact::array(TypeFact::I64))
        );
        assert_eq!(entry.type_id(), LogicalRecordKind::MapEntry.type_id());
    }

    #[test]
    fn reflection_record_producers_match_logical_field_slots_and_shapes() {
        use vela_def::{FunctionId, TypeId};
        use vela_reflect::modules::{FunctionDesc, FunctionParamDesc};
        use vela_reflect::registry::{
            MethodDesc, MethodParamDesc, TypeDesc, TypeKey, TypeRegistry,
        };
        use vela_reflect::value::ReflectValue;

        let mut registry = TypeRegistry::new();
        registry.register_function(
            FunctionDesc::new(FunctionId::new(1), "sample")
                .param(FunctionParamDesc::new("value").type_hint("i64"))
                .return_type("i64"),
        );
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(2), "Sample")).method(
                MethodDesc::new(vela_def::MethodId::new(3), "sample")
                    .param(MethodParamDesc::new("value").type_hint("i64"))
                    .return_type("i64"),
            ),
        );
        let function = vela_reflect::modules::function(&registry, "sample").expect("function");
        let target = vela_reflect::types::type_by_name(&registry, "Sample").expect("type");
        let ReflectValue::Array(methods) =
            vela_reflect::members::methods(&registry, &target).expect("methods")
        else {
            panic!("methods must be an array");
        };
        for (value, kind) in [
            (&function, LogicalRecordKind::ReflectFunction),
            (&methods[0], LogicalRecordKind::ReflectMethod),
        ] {
            assert_record_layout(value, kind);
            let ReflectValue::ScriptRecord { fields, .. } = value else {
                unreachable!();
            };
            assert_record_layout(&fields["effects"], LogicalRecordKind::ReflectEffectSet);
            assert_record_layout(
                &fields["access"],
                if kind == LogicalRecordKind::ReflectFunction {
                    LogicalRecordKind::ReflectFunctionAccess
                } else {
                    LogicalRecordKind::ReflectMethodAccess
                },
            );
            let ReflectValue::Array(params) = &fields["params"] else {
                panic!("params must be an array");
            };
            assert_record_layout(&params[0], LogicalRecordKind::ReflectParam);
        }
    }

    #[test]
    fn all_reflection_producers_match_exact_logical_layouts_recursively() {
        use std::collections::BTreeSet;
        use vela_common::{SourceId, Span};
        use vela_def::{FunctionId, MethodId, StateId, VariantId};
        use vela_reflect::modules::{FunctionDesc, FunctionParamDesc, ModuleDesc, StateDesc};
        use vela_reflect::registry::{
            FieldDesc, MethodDesc, MethodParamDesc, TraitDesc, TraitMethodDesc, TypeDesc, TypeKey,
            TypeRegistry, VariantDesc,
        };
        use vela_reflect::value::ReflectValue;

        let span = Span::new(SourceId::new(1), 0, 10);
        let mut registry = TypeRegistry::new();
        registry.register_module(ModuleDesc::new("sample").source_span(span));
        registry.register_function(
            FunctionDesc::new(FunctionId::new(1), "sample::run")
                .module("sample")
                .param(FunctionParamDesc::new("value").type_hint("Option<i64>"))
                .return_type("i64")
                .source_span(span),
        );
        registry.register_state(
            StateDesc::new(StateId::new(2), "sample::count")
                .module("sample")
                .type_contract("i64")
                .initializer(true),
        );
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(3), "Sample"))
                .source_span(span)
                .field(FieldDesc::new(FieldId::new(4), "count").type_hint("i64"))
                .method(
                    MethodDesc::new(MethodId::new(5), "run")
                        .param(MethodParamDesc::new("value").type_hint("i64")),
                )
                .variant(
                    VariantDesc::new(VariantId::new(6), "Ready")
                        .field(FieldDesc::new(FieldId::new(7), "value").type_hint("String")),
                )
                .trait_impl(
                    TraitDesc::new("Runnable").method(
                        TraitMethodDesc::new(MethodId::new(8), "run")
                            .param(MethodParamDesc::new("value").type_hint("i64")),
                    ),
                ),
        );
        fn visit(value: &ReflectValue, seen: &mut BTreeSet<LogicalRecordKind>) {
            match value {
                ReflectValue::ScriptRecord { type_name, fields } => {
                    let kind = LogicalRecordKind::ALL
                        .into_iter()
                        .find(|kind| kind.runtime_name() == type_name)
                        .unwrap_or_else(|| panic!("missing logical layout for {type_name}"));
                    assert_record_layout(value, kind);
                    seen.insert(kind);
                    for value in fields.values() {
                        visit(value, seen);
                    }
                }
                ReflectValue::ScriptEnum { fields, .. } | ReflectValue::Record(fields) => {
                    for value in fields.values() {
                        visit(value, seen);
                    }
                }
                ReflectValue::Array(values) | ReflectValue::Set(values) => {
                    for value in values {
                        visit(value, seen);
                    }
                }
                ReflectValue::Map(entries) => {
                    for entry in entries {
                        visit(&entry.key, seen);
                        visit(&entry.value, seen);
                    }
                }
                ReflectValue::Host(_) | ReflectValue::Closure | ReflectValue::Range => {}
            }
        }
        let mut seen = BTreeSet::new();
        for value in [
            vela_reflect::types::type_list(&registry),
            vela_reflect::members::all_fields(&registry),
            vela_reflect::members::all_methods(&registry),
            vela_reflect::members::all_traits(&registry),
            vela_reflect::members::all_variants(&registry),
            vela_reflect::modules::modules(&registry),
            vela_reflect::modules::functions(&registry),
            vela_reflect::modules::states(&registry),
        ] {
            visit(&value, &mut seen);
        }
        assert_eq!(
            seen,
            LogicalRecordKind::ALL
                .into_iter()
                .filter(|kind| *kind != LogicalRecordKind::MapEntry)
                .collect()
        );
        assert!(
            LogicalRecordFact::fixed(LogicalRecordKind::ReflectState)
                .field("storage")
                .is_none()
        );
    }

    fn assert_record_layout(value: &vela_reflect::value::ReflectValue, kind: LogicalRecordKind) {
        let vela_reflect::value::ReflectValue::ScriptRecord { type_name, fields } = value else {
            panic!("metadata must be a record");
        };
        let layout = LogicalRecordFact::fixed(kind);
        assert_eq!(type_name, layout.runtime_name());
        assert_eq!(
            fields.keys().map(String::as_str).collect::<Vec<_>>(),
            layout
                .fields()
                .map(LogicalRecordFieldFact::name)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            script_shape_id(type_name, fields.keys().map(String::as_str)),
            layout.shape()
        );
    }

    #[test]
    fn reflection_manifest_covers_nested_metadata_layouts() {
        let function = LogicalRecordFact::fixed(LogicalRecordKind::ReflectFunction);
        let access = function.field("access").expect("function access field");
        let params = function.field("params").expect("function params field");
        let effects = function.field("effects").expect("function effects field");

        assert!(matches!(
            access.fact(),
            TypeFact::LogicalRecord(record)
                if record.kind() == LogicalRecordKind::ReflectFunctionAccess
        ));
        assert!(matches!(
            params.fact(),
            TypeFact::Array { element }
                if matches!(element.as_ref(), TypeFact::LogicalRecord(record)
                    if record.kind() == LogicalRecordKind::ReflectParam)
        ));
        assert!(matches!(
            effects.fact(),
            TypeFact::LogicalRecord(record)
                if record.kind() == LogicalRecordKind::ReflectEffectSet
        ));
        assert!(function.field("is_async").is_none());
        assert!(
            !function
                .fields()
                .any(|field| field.name().starts_with("detached_"))
        );
        assert!(
            LogicalRecordFact::fixed(LogicalRecordKind::ReflectMethod)
                .field("is_async")
                .is_none()
        );
        assert!(
            LogicalRecordFact::fixed(LogicalRecordKind::ReflectEffectSet)
                .field("spawns_tasks")
                .is_none()
        );
    }
}
