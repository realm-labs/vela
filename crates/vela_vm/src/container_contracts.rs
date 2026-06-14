use vela_bytecode::{StandardTypeGuard, TypeGuardPlan, UnlinkedTypeGuardPlan};
use vela_common::{HostTypeId, PrimitiveTag};

use crate::heap::{HeapValue, ScriptHeap};
use crate::option_result::{StdEnumKind, std_enum_tag};
use crate::value::Value;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ContainerContracts {
    key_summary: ContainerTypeSummary,
    value_summary: ContainerTypeSummary,
    stamps: Vec<ContainerContractStamp>,
}

impl ContainerContracts {
    pub(crate) fn for_array(values: &[Value], heap: &ScriptHeap) -> Self {
        Self {
            value_summary: ContainerTypeSummary::from_values(values.iter().copied(), heap),
            ..Self::default()
        }
    }

    pub(crate) fn for_set(values: &[Value], heap: &ScriptHeap) -> Self {
        Self {
            value_summary: ContainerTypeSummary::from_values(values.iter().copied(), heap),
            ..Self::default()
        }
    }

    pub(crate) fn for_map(
        keys: impl IntoIterator<Item = Value>,
        values: impl IntoIterator<Item = Value>,
        heap: &ScriptHeap,
    ) -> Self {
        Self {
            key_summary: ContainerTypeSummary::from_values(keys, heap),
            value_summary: ContainerTypeSummary::from_values(values, heap),
            ..Self::default()
        }
    }

    pub(crate) fn key_summary(&self) -> ContainerTypeSummary {
        self.key_summary
    }

    pub(crate) fn value_summary(&self) -> ContainerTypeSummary {
        self.value_summary
    }

    pub(crate) fn has_stamp(&self, stamp: &ContainerContractStamp) -> bool {
        self.stamps.iter().any(|candidate| candidate == stamp)
    }

    pub(crate) fn install_stamp(&mut self, stamp: ContainerContractStamp) {
        if !self.has_stamp(&stamp) {
            self.stamps.push(stamp);
        }
    }

    pub(crate) fn clear_stamps(&mut self) {
        self.stamps.clear();
    }

    pub(crate) fn note_inserted_value(&mut self, key: Option<ShallowTypeKey>) {
        self.value_summary.observe(key);
        self.stamps.clear();
    }

    pub(crate) fn note_inserted_map_entry(
        &mut self,
        key: Option<ShallowTypeKey>,
        value: Option<ShallowTypeKey>,
    ) {
        self.key_summary.observe(key);
        self.value_summary.observe(value);
        self.stamps.clear();
    }

    pub(crate) fn note_replaced_or_removed_value(&mut self) {
        self.value_summary = ContainerTypeSummary::Unknown;
        self.stamps.clear();
    }

    pub(crate) fn note_replaced_map_value(&mut self) {
        self.value_summary = ContainerTypeSummary::Unknown;
        self.stamps.clear();
    }

    pub(crate) fn note_removed_map_entry(&mut self) {
        self.key_summary = ContainerTypeSummary::Unknown;
        self.value_summary = ContainerTypeSummary::Unknown;
        self.stamps.clear();
    }

    pub(crate) fn note_cleared(&mut self) {
        self.key_summary = ContainerTypeSummary::Empty;
        self.value_summary = ContainerTypeSummary::Empty;
        self.stamps.clear();
    }

    pub(crate) fn resummarize_array(&mut self, values: &[Value], heap: &ScriptHeap) {
        self.value_summary = ContainerTypeSummary::from_values(values.iter().copied(), heap);
    }

    pub(crate) fn resummarize_set(&mut self, values: &[Value], heap: &ScriptHeap) {
        self.value_summary = ContainerTypeSummary::from_values(values.iter().copied(), heap);
    }

    pub(crate) fn resummarize_map(
        &mut self,
        keys: impl IntoIterator<Item = Value>,
        values: impl IntoIterator<Item = Value>,
        heap: &ScriptHeap,
    ) {
        self.key_summary = ContainerTypeSummary::from_values(keys, heap);
        self.value_summary = ContainerTypeSummary::from_values(values, heap);
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ContainerTypeSummary {
    #[default]
    Empty,
    Exact(ShallowTypeKey),
    Mixed,
    Unknown,
}

impl ContainerTypeSummary {
    fn from_values(values: impl IntoIterator<Item = Value>, heap: &ScriptHeap) -> Self {
        let mut summary = Self::Empty;
        for value in values {
            summary.observe(ShallowTypeKey::from_value(&value, heap));
        }
        summary
    }

    fn observe(&mut self, key: Option<ShallowTypeKey>) {
        match (*self, key) {
            (Self::Unknown, _) | (_, None) => *self = Self::Unknown,
            (Self::Empty, Some(key)) => *self = Self::Exact(key),
            (Self::Exact(previous), Some(next)) if previous == next => {}
            (Self::Exact(_), Some(_)) | (Self::Mixed, Some(_)) => *self = Self::Mixed,
        }
    }

    pub(crate) fn prove_unlinked_plan(self, plan: &UnlinkedTypeGuardPlan) -> ContainerSummaryProof {
        self.prove_plan(
            unlinked_complete_shallow_key(plan),
            unlinked_required_shallow_key(plan),
        )
    }

    pub(crate) fn prove_linked_plan(self, plan: &TypeGuardPlan) -> ContainerSummaryProof {
        self.prove_plan(
            linked_complete_shallow_key(plan),
            linked_required_shallow_key(plan),
        )
    }

    pub(crate) fn prove_exact_key(self, expected: ShallowTypeKey) -> ContainerSummaryProof {
        self.prove_plan(Some(expected), Some(expected))
    }

    fn prove_plan(
        self,
        complete: Option<ShallowTypeKey>,
        required: Option<ShallowTypeKey>,
    ) -> ContainerSummaryProof {
        match self {
            Self::Empty => ContainerSummaryProof::Proven,
            Self::Exact(actual) => {
                if complete == Some(actual) {
                    ContainerSummaryProof::Proven
                } else if required.is_some_and(|expected| expected != actual) {
                    ContainerSummaryProof::Mismatch(actual)
                } else {
                    ContainerSummaryProof::Unknown
                }
            }
            Self::Mixed => ContainerSummaryProof::Unknown,
            Self::Unknown => ContainerSummaryProof::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ContainerSummaryProof {
    Proven,
    Mismatch(ShallowTypeKey),
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ContainerContractStamp {
    Unlinked(UnlinkedTypeGuardPlan),
    Linked(TypeGuardPlan),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ShallowTypeKey {
    Primitive(PrimitiveTag),
    Standard(StandardTypeGuard),
    Shape(vela_def::TypeId, vela_common::ShapeId),
    Variant(vela_def::VariantId),
    Host(HostTypeId),
}

impl ShallowTypeKey {
    pub(crate) fn type_name(self) -> &'static str {
        match self {
            Self::Primitive(PrimitiveTag::String) => "String",
            Self::Primitive(PrimitiveTag::Bytes) => "Bytes",
            Self::Primitive(tag) => tag.name(),
            Self::Standard(StandardTypeGuard::Array) => "Array",
            Self::Standard(StandardTypeGuard::Map) => "Map",
            Self::Standard(StandardTypeGuard::Set) => "Set",
            Self::Standard(StandardTypeGuard::Range) => "Range",
            Self::Standard(StandardTypeGuard::Function) => "Function",
            Self::Standard(StandardTypeGuard::Closure) => "Closure",
            Self::Standard(StandardTypeGuard::Iterator) => "Iterator",
            Self::Standard(StandardTypeGuard::Option) => "Option",
            Self::Standard(StandardTypeGuard::Result) => "Result",
            Self::Shape(_, _) => "record",
            Self::Variant(_) => "enum",
            Self::Host(_) => "host",
        }
    }

    pub(crate) fn from_value(value: &Value, heap: &ScriptHeap) -> Option<Self> {
        match value {
            Value::Null => Some(Self::Primitive(PrimitiveTag::Null)),
            Value::Bool(_) => Some(Self::Primitive(PrimitiveTag::Bool)),
            Value::Char(_) => Some(Self::Primitive(PrimitiveTag::Char)),
            Value::I8(_) => Some(Self::Primitive(PrimitiveTag::I8)),
            Value::I16(_) => Some(Self::Primitive(PrimitiveTag::I16)),
            Value::I32(_) => Some(Self::Primitive(PrimitiveTag::I32)),
            Value::I64(_) => Some(Self::Primitive(PrimitiveTag::I64)),
            Value::U8(_) => Some(Self::Primitive(PrimitiveTag::U8)),
            Value::U16(_) => Some(Self::Primitive(PrimitiveTag::U16)),
            Value::U32(_) => Some(Self::Primitive(PrimitiveTag::U32)),
            Value::U64(_) => Some(Self::Primitive(PrimitiveTag::U64)),
            Value::F32(_) => Some(Self::Primitive(PrimitiveTag::F32)),
            Value::F64(_) => Some(Self::Primitive(PrimitiveTag::F64)),
            Value::Range(_) => Some(Self::Standard(StandardTypeGuard::Range)),
            Value::HeapRef(reference) => match heap.get(*reference)? {
                HeapValue::String(_) => Some(Self::Primitive(PrimitiveTag::String)),
                HeapValue::Bytes(_) => Some(Self::Primitive(PrimitiveTag::Bytes)),
                HeapValue::Array(_) => Some(Self::Standard(StandardTypeGuard::Array)),
                HeapValue::Map(_) => Some(Self::Standard(StandardTypeGuard::Map)),
                HeapValue::Set(_) => Some(Self::Standard(StandardTypeGuard::Set)),
                HeapValue::Closure(_) => Some(Self::Standard(StandardTypeGuard::Closure)),
                HeapValue::Iterator(_) => Some(Self::Standard(StandardTypeGuard::Iterator)),
                HeapValue::Record {
                    identity: Some(identity),
                    ..
                } => Some(Self::Shape(identity.type_id, identity.shape_id)),
                HeapValue::Enum {
                    identity: Some(identity),
                    ..
                } => match std_enum_tag(*identity) {
                    Some((StdEnumKind::Option, _)) => {
                        Some(Self::Standard(StandardTypeGuard::Option))
                    }
                    Some((StdEnumKind::Result, _)) => {
                        Some(Self::Standard(StandardTypeGuard::Result))
                    }
                    None => Some(Self::Variant(identity.variant_id)),
                },
                HeapValue::Record { .. } | HeapValue::Enum { .. } => None,
                HeapValue::PathProxy(_) => None,
            },
            Value::HostRef(reference) => Some(Self::Host(reference.type_id)),
            Value::Missing => None,
        }
    }
}

fn unlinked_complete_shallow_key(plan: &UnlinkedTypeGuardPlan) -> Option<ShallowTypeKey> {
    match plan {
        UnlinkedTypeGuardPlan::Primitive(tag) => Some(ShallowTypeKey::Primitive(*tag)),
        UnlinkedTypeGuardPlan::Standard(guard) => Some(ShallowTypeKey::Standard(*guard)),
        UnlinkedTypeGuardPlan::Array { element: None } => {
            Some(ShallowTypeKey::Standard(StandardTypeGuard::Array))
        }
        UnlinkedTypeGuardPlan::Map {
            key: None,
            value: None,
        } => Some(ShallowTypeKey::Standard(StandardTypeGuard::Map)),
        UnlinkedTypeGuardPlan::Set { element: None } => {
            Some(ShallowTypeKey::Standard(StandardTypeGuard::Set))
        }
        UnlinkedTypeGuardPlan::Iterator { item: None } => {
            Some(ShallowTypeKey::Standard(StandardTypeGuard::Iterator))
        }
        UnlinkedTypeGuardPlan::Option { some: None } => {
            Some(ShallowTypeKey::Standard(StandardTypeGuard::Option))
        }
        UnlinkedTypeGuardPlan::Result {
            ok: None,
            err: None,
        } => Some(ShallowTypeKey::Standard(StandardTypeGuard::Result)),
        UnlinkedTypeGuardPlan::HostType { host_type_id, .. } => {
            Some(ShallowTypeKey::Host(*host_type_id))
        }
        _ => None,
    }
}

fn unlinked_required_shallow_key(plan: &UnlinkedTypeGuardPlan) -> Option<ShallowTypeKey> {
    match plan {
        UnlinkedTypeGuardPlan::Primitive(tag) => Some(ShallowTypeKey::Primitive(*tag)),
        UnlinkedTypeGuardPlan::Standard(guard) => Some(ShallowTypeKey::Standard(*guard)),
        UnlinkedTypeGuardPlan::Array { .. } => {
            Some(ShallowTypeKey::Standard(StandardTypeGuard::Array))
        }
        UnlinkedTypeGuardPlan::Map { .. } => Some(ShallowTypeKey::Standard(StandardTypeGuard::Map)),
        UnlinkedTypeGuardPlan::Set { .. } => Some(ShallowTypeKey::Standard(StandardTypeGuard::Set)),
        UnlinkedTypeGuardPlan::Iterator { .. } => {
            Some(ShallowTypeKey::Standard(StandardTypeGuard::Iterator))
        }
        UnlinkedTypeGuardPlan::Option { .. } => {
            Some(ShallowTypeKey::Standard(StandardTypeGuard::Option))
        }
        UnlinkedTypeGuardPlan::Result { .. } => {
            Some(ShallowTypeKey::Standard(StandardTypeGuard::Result))
        }
        UnlinkedTypeGuardPlan::HostType { host_type_id, .. } => {
            Some(ShallowTypeKey::Host(*host_type_id))
        }
        _ => None,
    }
}

fn linked_complete_shallow_key(plan: &TypeGuardPlan) -> Option<ShallowTypeKey> {
    match plan {
        TypeGuardPlan::Primitive(tag) => Some(ShallowTypeKey::Primitive(*tag)),
        TypeGuardPlan::Standard(guard) => Some(ShallowTypeKey::Standard(*guard)),
        TypeGuardPlan::Array { element: None } => {
            Some(ShallowTypeKey::Standard(StandardTypeGuard::Array))
        }
        TypeGuardPlan::Map {
            key: None,
            value: None,
        } => Some(ShallowTypeKey::Standard(StandardTypeGuard::Map)),
        TypeGuardPlan::Set { element: None } => {
            Some(ShallowTypeKey::Standard(StandardTypeGuard::Set))
        }
        TypeGuardPlan::Iterator { item: None } => {
            Some(ShallowTypeKey::Standard(StandardTypeGuard::Iterator))
        }
        TypeGuardPlan::Option { some: None } => {
            Some(ShallowTypeKey::Standard(StandardTypeGuard::Option))
        }
        TypeGuardPlan::Result {
            ok: None,
            err: None,
        } => Some(ShallowTypeKey::Standard(StandardTypeGuard::Result)),
        TypeGuardPlan::HostType { host_type_id, .. } => Some(ShallowTypeKey::Host(*host_type_id)),
        _ => None,
    }
}

fn linked_required_shallow_key(plan: &TypeGuardPlan) -> Option<ShallowTypeKey> {
    match plan {
        TypeGuardPlan::Primitive(tag) => Some(ShallowTypeKey::Primitive(*tag)),
        TypeGuardPlan::Standard(guard) => Some(ShallowTypeKey::Standard(*guard)),
        TypeGuardPlan::Array { .. } => Some(ShallowTypeKey::Standard(StandardTypeGuard::Array)),
        TypeGuardPlan::Map { .. } => Some(ShallowTypeKey::Standard(StandardTypeGuard::Map)),
        TypeGuardPlan::Set { .. } => Some(ShallowTypeKey::Standard(StandardTypeGuard::Set)),
        TypeGuardPlan::Iterator { .. } => {
            Some(ShallowTypeKey::Standard(StandardTypeGuard::Iterator))
        }
        TypeGuardPlan::Option { .. } => Some(ShallowTypeKey::Standard(StandardTypeGuard::Option)),
        TypeGuardPlan::Result { .. } => Some(ShallowTypeKey::Standard(StandardTypeGuard::Result)),
        TypeGuardPlan::HostType { host_type_id, .. } => Some(ShallowTypeKey::Host(*host_type_id)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_exact_to_mixed_is_one_way_for_inserts() {
        let mut summary = ContainerTypeSummary::Empty;

        summary.observe(Some(ShallowTypeKey::Primitive(PrimitiveTag::I64)));
        summary.observe(Some(ShallowTypeKey::Primitive(PrimitiveTag::I64)));
        assert_eq!(
            summary,
            ContainerTypeSummary::Exact(ShallowTypeKey::Primitive(PrimitiveTag::I64))
        );

        summary.observe(Some(ShallowTypeKey::Primitive(PrimitiveTag::String)));
        assert_eq!(summary, ContainerTypeSummary::Mixed);
    }

    #[test]
    fn shallow_summary_proves_simple_but_not_nested_contracts() {
        let summary =
            ContainerTypeSummary::Exact(ShallowTypeKey::Standard(StandardTypeGuard::Array));

        assert_eq!(
            summary.prove_unlinked_plan(&UnlinkedTypeGuardPlan::Array { element: None }),
            ContainerSummaryProof::Proven
        );
        assert_eq!(
            summary.prove_unlinked_plan(&UnlinkedTypeGuardPlan::Array {
                element: Some(Box::new(UnlinkedTypeGuardPlan::Primitive(
                    PrimitiveTag::I64
                ))),
            }),
            ContainerSummaryProof::Unknown
        );
    }

    #[test]
    fn clearing_contracts_resets_map_key_and_value_summaries() {
        let mut contracts = ContainerContracts::default();
        contracts.note_inserted_map_entry(
            Some(ShallowTypeKey::Primitive(PrimitiveTag::I64)),
            Some(ShallowTypeKey::Primitive(PrimitiveTag::String)),
        );

        contracts.note_cleared();

        assert_eq!(contracts.key_summary(), ContainerTypeSummary::Empty);
        assert_eq!(contracts.value_summary(), ContainerTypeSummary::Empty);
    }

    #[test]
    fn map_value_replacement_preserves_key_summary() {
        let mut contracts = ContainerContracts::default();
        contracts.note_inserted_map_entry(
            Some(ShallowTypeKey::Primitive(PrimitiveTag::I64)),
            Some(ShallowTypeKey::Primitive(PrimitiveTag::String)),
        );

        contracts.note_replaced_map_value();

        assert_eq!(
            contracts.key_summary(),
            ContainerTypeSummary::Exact(ShallowTypeKey::Primitive(PrimitiveTag::I64))
        );
        assert_eq!(contracts.value_summary(), ContainerTypeSummary::Unknown);
    }

    #[test]
    fn map_entry_removal_drops_stale_key_summary() {
        let mut contracts = ContainerContracts::default();
        contracts.note_inserted_map_entry(
            Some(ShallowTypeKey::Primitive(PrimitiveTag::I64)),
            Some(ShallowTypeKey::Primitive(PrimitiveTag::String)),
        );

        contracts.note_removed_map_entry();

        assert_eq!(contracts.key_summary(), ContainerTypeSummary::Unknown);
        assert_eq!(contracts.value_summary(), ContainerTypeSummary::Unknown);
    }
}
