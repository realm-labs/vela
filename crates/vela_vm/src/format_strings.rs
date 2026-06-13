use vela_bytecode::{Constant, ConstantId, FormatStringPart, Register};
use vela_common::Span;

use crate::heap::HeapValue;
use crate::{
    CallFrame, ExecutionBudget, HeapExecution, VmError, VmErrorKind, VmResult, allocate_heap_value,
    value_to_owned,
};

pub(crate) fn make_format_string(
    frame: &mut CallFrame,
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
    dst: Register,
    constants: &[Constant],
    parts: &[FormatStringPart],
    source_span: Option<Span>,
) -> VmResult<()> {
    let Some(heap) = heap else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "format string heap",
        })
        .with_source_span_if_absent(source_span));
    };
    let text = render_format_string(frame, heap, constants, parts, source_span)?;
    let value = allocate_heap_value(HeapValue::String(text), heap, budget)?;
    frame.write(dst, value)
}

fn render_format_string(
    frame: &CallFrame,
    heap: &HeapExecution<'_>,
    constants: &[Constant],
    parts: &[FormatStringPart],
    source_span: Option<Span>,
) -> VmResult<String> {
    let mut output = String::new();
    for part in parts {
        match part {
            FormatStringPart::Text(constant) => {
                output.push_str(text_constant(constants, *constant, source_span)?);
            }
            FormatStringPart::Value(register) => {
                let value = frame.read(*register)?;
                let owned = value_to_owned(&value, Some(heap))
                    .map_err(|error| error.with_source_span_if_absent(source_span))?;
                output.push_str(&owned.display_text());
            }
        }
    }
    Ok(output)
}

fn text_constant(
    constants: &[Constant],
    constant: ConstantId,
    source_span: Option<Span>,
) -> VmResult<&str> {
    match constants.get(constant.0) {
        Some(Constant::String(value)) => Ok(value),
        Some(_) => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "format string text constant",
        })
        .with_source_span_if_absent(source_span)),
        None => Err(VmError::new(VmErrorKind::ConstantOutOfBounds {
            constant: constant.0,
        })
        .with_source_span_if_absent(source_span)),
    }
}
