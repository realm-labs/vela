use vela_common::FunctionId;
use vela_vm::{Value, VmError, VmErrorKind, VmResult};

use crate::{
    EffectSet, FunctionAccess, NativeFunctionDesc, NativeFunctionEntry, NativeFunctionId, TypeHint,
};

pub const CONTEXT_TIME_PERMISSION: &str = "ctx.time";
pub const CTX_NOW_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0002);
pub const CTX_TICK_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0003);

pub(crate) fn context_clock_functions(now: i64, tick: i64) -> [NativeFunctionEntry; 2] {
    [
        NativeFunctionEntry::new(
            NativeFunctionDesc::new("ctx.now", CTX_NOW_FUNCTION_ID)
                .returns(TypeHint::Int)
                .effects(EffectSet::pure())
                .access(
                    FunctionAccess::public()
                        .reflect_callable(true)
                        .require_permission(CONTEXT_TIME_PERMISSION),
                )
                .docs("Returns the configured deterministic context timestamp."),
            move |args| context_value("ctx.now", now, args),
        ),
        NativeFunctionEntry::new(
            NativeFunctionDesc::new("ctx.tick", CTX_TICK_FUNCTION_ID)
                .returns(TypeHint::Int)
                .effects(EffectSet::pure())
                .access(
                    FunctionAccess::public()
                        .reflect_callable(true)
                        .require_permission(CONTEXT_TIME_PERMISSION),
                )
                .docs("Returns the configured deterministic context tick."),
            move |args| context_value("ctx.tick", tick, args),
        ),
    ]
}

fn context_value(name: &str, value: i64, args: &[Value]) -> VmResult<Value> {
    if args.is_empty() {
        return Ok(Value::Int(value));
    }
    Err(VmError {
        kind: VmErrorKind::ArityMismatch {
            name: name.to_owned(),
            expected: 0,
            actual: args.len(),
        },
        source_span: None,
        call_stack: Default::default(),
    })
}
