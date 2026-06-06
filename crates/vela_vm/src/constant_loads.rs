use crate::heap::HeapValue;
use crate::{HeapExecution, Value};

pub(crate) fn loaded_string_constant(
    current: Option<&Value>,
    constant: &str,
    heap: Option<&HeapExecution<'_>>,
) -> Option<Value> {
    let Value::HeapRef(reference) = current? else {
        return None;
    };
    match heap?.heap.get(*reference)? {
        HeapValue::String(value) if value == constant => Some(Value::HeapRef(*reference)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::heap::{HeapValue, ScriptHeap};
    use crate::{HeapExecution, Value};

    use super::*;

    #[test]
    fn loaded_string_constant_reuses_matching_heap_string() {
        let mut heap = ScriptHeap::new();
        let tick = Value::HeapRef(heap.allocate(HeapValue::String("tick".to_owned())));
        let other = Value::HeapRef(heap.allocate(HeapValue::String("other".to_owned())));
        let array = Value::HeapRef(heap.allocate(HeapValue::Array(Vec::new())));
        let heap = HeapExecution::new(&mut heap);

        assert_eq!(
            loaded_string_constant(Some(&tick), "tick", Some(&heap)),
            Some(tick)
        );
        assert_eq!(
            loaded_string_constant(Some(&other), "tick", Some(&heap)),
            None
        );
        assert_eq!(
            loaded_string_constant(Some(&array), "tick", Some(&heap)),
            None
        );
        assert_eq!(loaded_string_constant(Some(&tick), "tick", None), None);
        assert_eq!(
            loaded_string_constant(Some(&Value::Null), "tick", Some(&heap)),
            None
        );
    }
}
