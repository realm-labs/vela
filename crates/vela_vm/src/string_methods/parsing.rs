use crate::option_result::option_value;
use crate::{ExecutionBudget, HeapExecution, Value, VmResult};

use super::{expect_no_args, string_value};

macro_rules! parse_scalar_method {
    ($name:ident, $method:literal, $ty:ty, $variant:ident) => {
        pub(crate) fn $name(
            receiver: &Value,
            args: &[Value],
            heap: &mut Option<&mut HeapExecution<'_>>,
            budget: &mut Option<&mut ExecutionBudget>,
        ) -> VmResult<Value> {
            parse_option(receiver, args, heap, budget, $method, |value| {
                value.parse::<$ty>().ok().map(Value::$variant)
            })
        }
    };
}

parse_scalar_method!(parse_i8, "parse_i8", i8, I8);
parse_scalar_method!(parse_i16, "parse_i16", i16, I16);
parse_scalar_method!(parse_i32, "parse_i32", i32, I32);
parse_scalar_method!(parse_i64, "parse_i64", i64, I64);
parse_scalar_method!(parse_u8, "parse_u8", u8, U8);
parse_scalar_method!(parse_u16, "parse_u16", u16, U16);
parse_scalar_method!(parse_u32, "parse_u32", u32, U32);
parse_scalar_method!(parse_u64, "parse_u64", u64, U64);

pub(crate) fn parse_f32(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    parse_option(receiver, args, heap, budget, "parse_f32", |value| {
        value
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite())
            .map(Value::F32)
    })
}

pub(crate) fn parse_f64(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    parse_option(receiver, args, heap, budget, "parse_f64", |value| {
        value
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .map(Value::F64)
    })
}

pub(crate) fn parse_bool(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    parse_option(
        receiver,
        args,
        heap,
        budget,
        "parse_bool",
        |value| match value {
            "true" => Some(Value::Bool(true)),
            "false" => Some(Value::Bool(false)),
            _ => None,
        },
    )
}

pub(crate) fn parse_char(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    parse_option(receiver, args, heap, budget, "parse_char", |value| {
        let mut chars = value.chars();
        let first = chars.next()?;
        if chars.next().is_none() {
            Some(Value::Char(first))
        } else {
            None
        }
    })
}

fn parse_option(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    method: &'static str,
    parse: impl FnOnce(&str) -> Option<Value>,
) -> VmResult<Value> {
    expect_no_args(method, args)?;
    let operation = parse_operation(method);
    let value = string_value(receiver, heap.as_deref(), operation)?;
    let payload = parse(value);
    let Some(heap) = heap.as_deref_mut() else {
        return super::type_error(operation);
    };
    option_value(payload, heap, budget.as_deref_mut())
}

fn parse_operation(method: &'static str) -> &'static str {
    match method {
        "parse_i8" => "method parse_i8",
        "parse_i16" => "method parse_i16",
        "parse_i32" => "method parse_i32",
        "parse_i64" => "method parse_i64",
        "parse_u8" => "method parse_u8",
        "parse_u16" => "method parse_u16",
        "parse_u32" => "method parse_u32",
        "parse_u64" => "method parse_u64",
        "parse_f32" => "method parse_f32",
        "parse_f64" => "method parse_f64",
        "parse_bool" => "method parse_bool",
        "parse_char" => "method parse_char",
        _ => "method parse",
    }
}
