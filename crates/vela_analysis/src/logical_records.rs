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
    ReflectTrait,
    ReflectType,
    ReflectVariant,
}

impl LogicalRecordKind {
    pub const ALL: [Self; 14] = [
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
        Self::ReflectTrait,
        Self::ReflectType,
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
            Self::ReflectTrait => "ReflectTrait",
            Self::ReflectType => "ReflectType",
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
        ReflectTrait, ReflectType, ReflectVariant,
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
            ("spawns_tasks", TypeFact::BOOL),
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
            ("source_span", fixed_record(ReflectSourceSpan)),
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
            ("is_async", TypeFact::BOOL),
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
            ("is_async", TypeFact::BOOL),
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
            ("methods", TypeFact::array(fixed_record(ReflectMethod))),
            ("name", TypeFact::STRING),
            ("origin", TypeFact::STRING),
            ("source_span", fixed_record(ReflectSourceSpan)),
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
            ("source_span", fixed_record(ReflectSourceSpan)),
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
    fn reflection_manifest_covers_nested_metadata_layouts() {
        let function = LogicalRecordFact::fixed(LogicalRecordKind::ReflectFunction);
        let access = function.field("access").expect("function access field");
        let asyncness = function.field("is_async").expect("function is_async field");
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
        assert_eq!(asyncness.fact(), &TypeFact::BOOL);
    }
}
