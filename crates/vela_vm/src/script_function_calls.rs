use vela_bytecode::CallArgument;

use crate::{CallFrame, SmallStorage, Value, VmResult};

#[inline]
/// Reads one call argument out of the caller frame.
///
/// Shared with prefixed argument construction so a receiver plus its arguments
/// can be gathered without materializing an intermediate vector.
pub(crate) fn script_call_argument_value(
    frame: &CallFrame,
    argument: &CallArgument,
) -> VmResult<Value> {
    match argument {
        CallArgument::Register(register) => frame.read(*register),
        CallArgument::Missing => Ok(Value::Missing),
    }
}

pub(crate) fn script_call_args_from_call_arguments(
    frame: &CallFrame,
    args: &[CallArgument],
) -> VmResult<SmallStorage<Value>> {
    SmallStorage::try_from_slice_map(args, 4, |arg| script_call_argument_value(frame, arg))
}
