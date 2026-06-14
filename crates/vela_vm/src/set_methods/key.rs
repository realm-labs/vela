use crate::value_key::ValueKey;
use crate::{HeapExecution, Value, VmResult};

pub(super) type SetKey = ValueKey;

impl SetKey {}

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
