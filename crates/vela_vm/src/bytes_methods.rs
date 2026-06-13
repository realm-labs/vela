use crate::heap::HeapValue;
use crate::owned_value::OwnedValue;
use crate::{
    ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult, allocate_heap_value,
    expect_arity, option_result,
};

pub(crate) fn is_bytes(value: &Value, heap: Option<&HeapExecution<'_>>) -> bool {
    match value {
        Value::HeapRef(reference) => matches!(
            heap.and_then(|heap| heap.heap.get(*reference)),
            Some(HeapValue::Bytes(_))
        ),
        _ => false,
    }
}

pub(crate) fn len(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("len", args, 0)?;
    let value = bytes_value(receiver, heap, "method len")?;
    Ok(Value::i64(usize_to_i64(value.len(), "method len")?))
}

pub(crate) fn is_empty(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("is_empty", args, 0)?;
    let value = bytes_value(receiver, heap, "method is_empty")?;
    Ok(Value::Bool(value.is_empty()))
}

pub(crate) fn get(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("get", args, 1)?;
    let value = bytes_value(receiver, heap, "method get")?;
    let index = byte_index(&args[0], value.len(), "method get")?;
    let byte = value
        .get(index)
        .copied()
        .ok_or_else(|| index_out_of_bounds(index, value.len()))?;
    Ok(Value::U8(byte))
}

pub(crate) fn slice(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    let value = bytes_value(receiver, heap.as_deref(), "method slice")?;
    let value = slice_payload(value, args)?;
    make_bytes(value, heap, budget, "method slice")
}

pub(crate) fn read_u32_le(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    read_u32(receiver, args, heap, u32::from_le_bytes, "read_u32_le")
}

pub(crate) fn read_u32_be(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    read_u32(receiver, args, heap, u32::from_be_bytes, "read_u32_be")
}

pub(crate) fn to_hex(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    let value = bytes_value(receiver, heap.as_deref(), "method to_hex")?;
    let text = to_hex_payload(value, args)?;
    make_string(text, heap, budget, "method to_hex")
}

pub(crate) fn slice_payload(value: &[u8], args: &[Value]) -> VmResult<Vec<u8>> {
    expect_arity("slice", args, 2)?;
    let start = byte_index(&args[0], value.len(), "method slice")?;
    let end = byte_index(&args[1], value.len(), "method slice")?;
    if start > end {
        return type_error("method slice range");
    }
    if start > value.len() {
        return Err(index_out_of_bounds(start, value.len()));
    }
    if end > value.len() {
        return Err(index_out_of_bounds(end, value.len()));
    }
    Ok(value[start..end].to_vec())
}

pub(crate) fn to_hex_payload(value: &[u8], args: &[Value]) -> VmResult<String> {
    expect_arity("to_hex", args, 0)?;
    let mut text = String::with_capacity(value.len() * 2);
    for byte in value {
        use std::fmt::Write as _;
        write!(&mut text, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(text)
}

pub(crate) fn from_hex(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_owned_arity("bytes::from_hex", args, 1)?;
    let OwnedValue::String(text) = &args[0] else {
        return type_error("bytes::from_hex");
    };
    match decode_hex(text) {
        Ok(bytes) => Ok(option_result::owned_result_ok(OwnedValue::Bytes(bytes))),
        Err(message) => Ok(option_result::owned_result_err(OwnedValue::String(
            message.to_owned(),
        ))),
    }
}

fn read_u32(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
    read: fn([u8; 4]) -> u32,
    method: &'static str,
) -> VmResult<Value> {
    expect_arity(method, args, 1)?;
    let operation = match method {
        "read_u32_le" => "method read_u32_le",
        "read_u32_be" => "method read_u32_be",
        _ => "method read_u32",
    };
    let value = bytes_value(receiver, heap, operation)?;
    let index = byte_index(&args[0], value.len(), operation)?;
    let end = index
        .checked_add(4)
        .ok_or_else(|| index_out_of_bounds(index, value.len()))?;
    if end > value.len() {
        return Err(index_out_of_bounds(index, value.len()));
    }
    let bytes = <[u8; 4]>::try_from(&value[index..end])
        .map_err(|_| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
    Ok(Value::U32(read(bytes)))
}

pub(crate) fn bytes_value<'a>(
    value: &'a Value,
    heap: Option<&'a HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<&'a [u8]> {
    match value {
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::Bytes(value)) => Ok(value),
            _ => type_error(operation),
        },
        _ => type_error(operation),
    }
}

fn byte_index(value: &Value, len: usize, operation: &'static str) -> VmResult<usize> {
    match value {
        Value::I64(index) if *index >= 0 => Ok(*index as usize),
        Value::I64(index) => Err(VmError::new(VmErrorKind::IndexOutOfBounds {
            index: *index,
            len,
        })),
        _ => type_error(operation),
    }
}

fn make_bytes(
    value: Vec<u8>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let Some(heap) = heap.as_deref_mut() else {
        return type_error(operation);
    };
    allocate_heap_value(HeapValue::Bytes(value), heap, budget.as_deref_mut())
}

fn make_string(
    value: String,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let Some(heap) = heap.as_deref_mut() else {
        return type_error(operation);
    };
    allocate_heap_value(HeapValue::String(value), heap, budget.as_deref_mut())
}

fn decode_hex(text: &str) -> Result<Vec<u8>, &'static str> {
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(2) {
        return Err("hex input must contain an even number of digits");
    }
    bytes
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_digit(pair[0]).ok_or("hex input contains a non-hex digit")?;
            let low = hex_digit(pair[1]).ok_or("hex input contains a non-hex digit")?;
            Ok((high << 4) | low)
        })
        .collect()
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn index_out_of_bounds(index: usize, len: usize) -> VmError {
    VmError::new(VmErrorKind::IndexOutOfBounds {
        index: i64::try_from(index).unwrap_or(i64::MAX),
        len,
    })
}

fn usize_to_i64(value: usize, operation: &'static str) -> VmResult<i64> {
    i64::try_from(value).map_err(|_| VmError::new(VmErrorKind::TypeMismatch { operation }))
}

fn expect_owned_arity(name: &str, args: &[OwnedValue], expected: usize) -> VmResult<()> {
    if args.len() == expected {
        return Ok(());
    }
    Err(VmError::new(VmErrorKind::ArityMismatch {
        name: name.to_owned(),
        expected,
        actual: args.len(),
    }))
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
