use vela_bytecode::{HostPathSegment, Register};
use vela_common::{FieldId, Span, SymbolInterner};
use vela_host::path::HostPath;
use vela_host::tx::PatchTx;
use vela_host::value::HostValue;

use crate::host_paths::host_path_from_segments;
use crate::host_values::value_to_host;
use crate::{
    CallFrame, ExecutionBudget, HeapExecution, HostExecution, VmError, VmErrorKind, VmResult,
    expect_host_ref,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostNumericPatch {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

pub(crate) struct HostPatchRuntime<'a, 'host, 'heap> {
    pub(crate) frame: &'a CallFrame,
    pub(crate) heap: Option<&'a HeapExecution<'heap>>,
    pub(crate) budget: Option<&'a ExecutionBudget>,
    pub(crate) host: Option<&'a mut HostExecution<'host>>,
    pub(crate) source_span: Option<Span>,
}

impl HostNumericPatch {
    fn field_operation(self) -> &'static str {
        match self {
            Self::Add => "add_host_field",
            Self::Sub => "sub_host_field",
            Self::Mul => "mul_host_field",
            Self::Div => "div_host_field",
            Self::Rem => "rem_host_field",
        }
    }

    fn path_operation(self) -> &'static str {
        match self {
            Self::Add => "add_host_path",
            Self::Sub => "sub_host_path",
            Self::Mul => "mul_host_path",
            Self::Div => "div_host_path",
            Self::Rem => "rem_host_path",
        }
    }

    fn apply(
        self,
        tx: &mut PatchTx,
        path: HostPath,
        value: HostValue,
        base_value: HostValue,
        source_span: Option<Span>,
    ) -> VmResult<()> {
        match self {
            Self::Add => tx.add_path(path, value, base_value, source_span),
            Self::Sub => tx.sub_path(path, value, base_value, source_span),
            Self::Mul => tx.mul_path(path, value, base_value, source_span),
            Self::Div => tx.div_path(path, value, base_value, source_span),
            Self::Rem => tx.rem_path(path, value, base_value, source_span),
        }?;
        Ok(())
    }
}

pub(crate) fn apply_host_field_numeric_patch(
    runtime: HostPatchRuntime<'_, '_, '_>,
    root: Register,
    field: FieldId,
    rhs: Register,
    patch: HostNumericPatch,
) -> VmResult<()> {
    let root = expect_host_ref(runtime.frame.read(root)?, patch.field_operation())?;
    let value = value_to_host(
        runtime.frame.read(rhs)?,
        patch.field_operation(),
        runtime.heap,
    )?;
    let path = HostPath::new(root).field(field);
    apply_host_numeric_patch(path, value, patch, runtime)
}

pub(crate) fn apply_host_path_numeric_patch(
    runtime: HostPatchRuntime<'_, '_, '_>,
    root: Register,
    segments: &[HostPathSegment],
    rhs: Register,
    patch: HostNumericPatch,
    symbols: &mut SymbolInterner,
) -> VmResult<()> {
    let root = expect_host_ref(runtime.frame.read(root)?, patch.path_operation())?;
    let value = value_to_host(
        runtime.frame.read(rhs)?,
        patch.path_operation(),
        runtime.heap,
    )?;
    let path = host_path_from_segments(root, segments, runtime.frame, runtime.heap, symbols)?;
    apply_host_numeric_patch(path, value, patch, runtime)
}

fn apply_host_numeric_patch(
    path: HostPath,
    value: HostValue,
    patch: HostNumericPatch,
    runtime: HostPatchRuntime<'_, '_, '_>,
) -> VmResult<()> {
    let host = runtime.host.ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "host context",
        })
    })?;
    let base_value = host
        .tx
        .read_path_at(host.adapter, &path, runtime.source_span)?;
    if let Some(budget) = runtime.budget {
        budget.reserve_patch(host.tx.patches().len())?;
    }
    patch.apply(host.tx, path, value, base_value, runtime.source_span)
}
