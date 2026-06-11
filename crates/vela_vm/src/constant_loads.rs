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

pub(crate) fn loaded_bytes_constant(
    current: Option<&Value>,
    constant: &[u8],
    heap: Option<&HeapExecution<'_>>,
) -> Option<Value> {
    let Value::HeapRef(reference) = current? else {
        return None;
    };
    match heap?.heap.get(*reference)? {
        HeapValue::Bytes(value) if value == constant => Some(Value::HeapRef(*reference)),
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

    #[test]
    fn loaded_bytes_constant_reuses_matching_heap_bytes() {
        let mut heap = ScriptHeap::new();
        let bytes = Value::HeapRef(heap.allocate(HeapValue::Bytes(vec![1, 2, 3])));
        let other = Value::HeapRef(heap.allocate(HeapValue::Bytes(vec![4])));
        let string = Value::HeapRef(heap.allocate(HeapValue::String("123".to_owned())));
        let heap = HeapExecution::new(&mut heap);

        assert_eq!(
            loaded_bytes_constant(Some(&bytes), &[1, 2, 3], Some(&heap)),
            Some(bytes)
        );
        assert_eq!(
            loaded_bytes_constant(Some(&other), &[1, 2, 3], Some(&heap)),
            None
        );
        assert_eq!(
            loaded_bytes_constant(Some(&string), &[1, 2, 3], Some(&heap)),
            None
        );
        assert_eq!(loaded_bytes_constant(Some(&bytes), &[1, 2, 3], None), None);
        assert_eq!(
            loaded_bytes_constant(Some(&Value::Null), &[1, 2, 3], Some(&heap)),
            None
        );
    }
}
