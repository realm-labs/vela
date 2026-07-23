use vela_bytecode::{Constant, ConstantId, FormatStringPart, Register};
use vela_common::Span;

use crate::heap::HeapValue;
use crate::{
    CallFrame, ExecutionBudget, HeapExecution, HostExecution, VmError, VmErrorKind, VmResult,
    allocate_heap_value, value_to_owned,
};

pub(crate) struct FormatStringRequest<'a> {
    pub(crate) dst: Register,
    pub(crate) constants: &'a [Constant],
    pub(crate) parts: &'a [FormatStringPart],
    pub(crate) source_span: Option<Span>,
}

pub(crate) fn make_format_string(
    frame: &mut CallFrame,
    heap: Option<&mut HeapExecution<'_>>,
    host: Option<&HostExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
    request: FormatStringRequest<'_>,
) -> VmResult<()> {
    let Some(heap) = heap else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "format string heap",
        })
        .with_source_span_if_absent(request.source_span));
    };
    let text = render_format_string(
        frame,
        heap,
        host,
        request.constants,
        request.parts,
        request.source_span,
    )?;
    let value = allocate_heap_value(HeapValue::String(text), heap, budget)?;
    frame.write(request.dst, value)
}

fn render_format_string(
    frame: &CallFrame,
    heap: &HeapExecution<'_>,
    host: Option<&HostExecution<'_>>,
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
                let owned = value_to_owned(
                    &value,
                    Some(heap),
                    host.map(|host| {
                        &*host.adapter as &(dyn vela_host::adapter::ScriptStateAdapter + Send)
                    }),
                )
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
