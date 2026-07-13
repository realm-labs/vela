use vela_bytecode::CallArgument;

use crate::{CallFrame, SmallStorage, Value, VmResult};

#[inline]
pub(crate) fn script_call_args_from_call_arguments(
    frame: &CallFrame,
    args: &[CallArgument],
) -> VmResult<SmallStorage<Value>> {
    SmallStorage::try_from_slice_map(args, 4, |arg| match arg {
        CallArgument::Register(register) => Ok(frame.read(*register)?),
        CallArgument::Missing => Ok(Value::Missing),
    })
}
