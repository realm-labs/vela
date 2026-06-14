use crate::value_key::ValueKey;
use crate::{HeapExecution, Value, VmResult};

pub(super) type SetKey = ValueKey;

impl SetKey {
    #[allow(dead_code)]
    pub(super) fn matches_value(
        &self,
        value: &Value,
        heap: Option<&HeapExecution<'_>>,
        operation: &'static str,
    ) -> VmResult<bool> {
        Self::from_value(value, heap, operation).map(|key| self == &key)
    }

    pub(super) fn matches_slot(
        &self,
        slot: &Value,
        heap: &HeapExecution<'_>,
        operation: &'static str,
    ) -> VmResult<bool> {
        Self::from_value(slot, Some(heap), operation).map(|key| self == &key)
    }
}

pub(super) fn set_keys(
    values: &[Value],
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<Vec<SetKey>> {
    values
        .iter()
        .map(|value| SetKey::from_value(value, heap, operation))
        .collect()
}

pub(super) fn slot_key(slot: &Value, heap: &HeapExecution<'_>) -> VmResult<SetKey> {
    SetKey::from_value(slot, Some(heap), "method set")
}
