use vela_common::FunctionId;
use vela_reflect::modules::ModuleDesc;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

use crate::native::{
    EffectSet, FunctionAccess, NativeFunctionDesc, NativeFunctionEntry, NativeFunctionId, TypeHint,
};

pub const CONTEXT_TIME_PERMISSION: &str = "ctx.time";
pub const CTX_NOW_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0002);
pub const CTX_TICK_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0003);
pub const CTX_ELAPSED_SINCE_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0004);

pub(crate) fn context_module_desc() -> ModuleDesc {
    ModuleDesc::new("ctx")
        .docs("Deterministic context helpers.")
        .attr("stdlib", "context")
        .attr("domain", "context")
}

pub(crate) fn context_clock_functions(now: i64, tick: i64) -> [NativeFunctionEntry; 3] {
    [
        NativeFunctionEntry::new(
            NativeFunctionDesc::new("ctx::now", CTX_NOW_FUNCTION_ID)
                .returns(TypeHint::Int)
                .effects(EffectSet::pure())
                .access(
                    FunctionAccess::public()
                        .reflect_callable(true)
                        .require_permission(CONTEXT_TIME_PERMISSION),
                )
                .docs("Returns the configured deterministic context timestamp."),
            move |args| context_value("ctx::now", now, args),
        ),
        NativeFunctionEntry::new(
            NativeFunctionDesc::new("ctx::tick", CTX_TICK_FUNCTION_ID)
                .returns(TypeHint::Int)
                .effects(EffectSet::pure())
                .access(
                    FunctionAccess::public()
                        .reflect_callable(true)
                        .require_permission(CONTEXT_TIME_PERMISSION),
                )
                .docs("Returns the configured deterministic context tick."),
            move |args| context_value("ctx::tick", tick, args),
        ),
        NativeFunctionEntry::new(
            NativeFunctionDesc::new("ctx::elapsed_since", CTX_ELAPSED_SINCE_FUNCTION_ID)
                .param("start", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::pure())
                .access(
                    FunctionAccess::public()
                        .reflect_callable(true)
                        .require_permission(CONTEXT_TIME_PERMISSION),
                )
                .docs("Returns deterministic context time elapsed since start."),
            move |args| elapsed_since(now, args),
        ),
    ]
}

fn context_value(name: &str, value: i64, args: &[OwnedValue]) -> VmResult<OwnedValue> {
    if args.is_empty() {
        return Ok(OwnedValue::Int(value));
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

fn elapsed_since(now: i64, args: &[OwnedValue]) -> VmResult<OwnedValue> {
    if args.len() != 1 {
        return Err(VmError {
            kind: VmErrorKind::ArityMismatch {
                name: "ctx::elapsed_since".to_owned(),
                expected: 1,
                actual: args.len(),
            },
            source_span: None,
            call_stack: Default::default(),
        });
    }

    let OwnedValue::Int(start) = args[0] else {
        return Err(VmError {
            kind: VmErrorKind::TypeMismatch {
                operation: "ctx::elapsed_since",
            },
            source_span: None,
            call_stack: Default::default(),
        });
    };

    now.checked_sub(start).map(OwnedValue::Int).ok_or(VmError {
        kind: VmErrorKind::TypeMismatch {
            operation: "ctx::elapsed_since",
        },
        source_span: None,
        call_stack: Default::default(),
    })
}
