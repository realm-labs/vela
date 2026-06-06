use vela_common::FunctionId;
use vela_reflect::modules::ModuleDesc;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

use crate::native::{
    EffectSet, FunctionAccess, NativeFunctionDesc, NativeFunctionEntry, NativeFunctionId, TypeHint,
};

pub const TIME_NOW_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0002);
pub const TIME_TICK_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0003);
pub const TIME_ELAPSED_SINCE_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0004);

pub(crate) fn time_module_desc() -> ModuleDesc {
    ModuleDesc::new("time")
        .docs("Deterministic time helpers.")
        .attr("stdlib", "time")
        .attr("domain", "time")
}

pub(crate) fn time_clock_functions(now: i64, tick: i64) -> [NativeFunctionEntry; 3] {
    [
        NativeFunctionEntry::new(
            NativeFunctionDesc::new("time::now", TIME_NOW_FUNCTION_ID)
                .returns(TypeHint::Int)
                .effects(EffectSet::time())
                .access(FunctionAccess::public().reflect_callable(true))
                .docs("Returns the configured deterministic timestamp."),
            move |args| time_value("time::now", now, args),
        ),
        NativeFunctionEntry::new(
            NativeFunctionDesc::new("time::tick", TIME_TICK_FUNCTION_ID)
                .returns(TypeHint::Int)
                .effects(EffectSet::time())
                .access(FunctionAccess::public().reflect_callable(true))
                .docs("Returns the configured deterministic tick."),
            move |args| time_value("time::tick", tick, args),
        ),
        NativeFunctionEntry::new(
            NativeFunctionDesc::new("time::elapsed_since", TIME_ELAPSED_SINCE_FUNCTION_ID)
                .param("start", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::time())
                .access(FunctionAccess::public().reflect_callable(true))
                .docs("Returns deterministic time elapsed since start."),
            move |args| elapsed_since(now, args),
        ),
    ]
}

fn time_value(name: &str, value: i64, args: &[OwnedValue]) -> VmResult<OwnedValue> {
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
                name: "time::elapsed_since".to_owned(),
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
                operation: "time::elapsed_since",
            },
            source_span: None,
            call_stack: Default::default(),
        });
    };

    now.checked_sub(start).map(OwnedValue::Int).ok_or(VmError {
        kind: VmErrorKind::TypeMismatch {
            operation: "time::elapsed_since",
        },
        source_span: None,
        call_stack: Default::default(),
    })
}
