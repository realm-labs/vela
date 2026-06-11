use vela_common::ScalarValue;

use crate::owned_value::OwnedValue;
use crate::{VmError, VmErrorKind, VmResult, option_result};

pub(crate) fn i64_from_i32(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("i64::from_i32", args, 1)?;
    let OwnedValue::Scalar(ScalarValue::I32(value)) = &args[0] else {
        return type_error("i64::from_i32");
    };
    Ok(OwnedValue::Scalar(ScalarValue::I64(i64::from(*value))))
}

pub(crate) fn u64_from_u32(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("u64::from_u32", args, 1)?;
    let OwnedValue::Scalar(ScalarValue::U32(value)) = &args[0] else {
        return type_error("u64::from_u32");
    };
    Ok(OwnedValue::Scalar(ScalarValue::U64(u64::from(*value))))
}

pub(crate) fn f64_from_f32(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("f64::from_f32", args, 1)?;
    let OwnedValue::Scalar(ScalarValue::F32(value)) = &args[0] else {
        return type_error("f64::from_f32");
    };
    Ok(OwnedValue::Scalar(ScalarValue::F64(f64::from(*value))))
}

pub(crate) fn i8_try_from_i64(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("i8::try_from_i64", args, 1)?;
    let OwnedValue::Scalar(ScalarValue::I64(value)) = &args[0] else {
        return type_error("i8::try_from_i64");
    };
    match i8::try_from(*value) {
        Ok(value) => Ok(ok_scalar(ScalarValue::I8(value))),
        Err(_) => Ok(err_string("i64 value is outside i8 range")),
    }
}

pub(crate) fn u8_try_from_u64(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("u8::try_from_u64", args, 1)?;
    let OwnedValue::Scalar(ScalarValue::U64(value)) = &args[0] else {
        return type_error("u8::try_from_u64");
    };
    match u8::try_from(*value) {
        Ok(value) => Ok(ok_scalar(ScalarValue::U8(value))),
        Err(_) => Ok(err_string("u64 value is outside u8 range")),
    }
}

pub(crate) fn f32_try_from_f64(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("f32::try_from_f64", args, 1)?;
    let OwnedValue::Scalar(ScalarValue::F64(value)) = &args[0] else {
        return type_error("f32::try_from_f64");
    };
    if !value.is_finite() || value.abs() > f64::from(f32::MAX) {
        return Ok(err_string("f64 value is outside finite f32 range"));
    }
    Ok(ok_scalar(ScalarValue::F32(*value as f32)))
}

pub(crate) fn u8_wrapping_add(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    let (lhs, rhs) = expect_u8_pair("u8::wrapping_add", args)?;
    Ok(OwnedValue::Scalar(ScalarValue::U8(lhs.wrapping_add(rhs))))
}

pub(crate) fn u32_wrapping_mul(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("u32::wrapping_mul", args, 2)?;
    let OwnedValue::Scalar(ScalarValue::U32(lhs)) = &args[0] else {
        return type_error("u32::wrapping_mul");
    };
    let OwnedValue::Scalar(ScalarValue::U32(rhs)) = &args[1] else {
        return type_error("u32::wrapping_mul");
    };
    Ok(OwnedValue::Scalar(ScalarValue::U32(lhs.wrapping_mul(*rhs))))
}

pub(crate) fn i8_wrapping_add(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("i8::wrapping_add", args, 2)?;
    let OwnedValue::Scalar(ScalarValue::I8(lhs)) = &args[0] else {
        return type_error("i8::wrapping_add");
    };
    let OwnedValue::Scalar(ScalarValue::I8(rhs)) = &args[1] else {
        return type_error("i8::wrapping_add");
    };
    Ok(OwnedValue::Scalar(ScalarValue::I8(lhs.wrapping_add(*rhs))))
}

pub(crate) fn u8_bit_and(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    let (lhs, rhs) = expect_u8_pair("u8::bit_and", args)?;
    Ok(OwnedValue::Scalar(ScalarValue::U8(lhs & rhs)))
}

pub(crate) fn u8_bit_or(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    let (lhs, rhs) = expect_u8_pair("u8::bit_or", args)?;
    Ok(OwnedValue::Scalar(ScalarValue::U8(lhs | rhs)))
}

pub(crate) fn u8_bit_xor(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    let (lhs, rhs) = expect_u8_pair("u8::bit_xor", args)?;
    Ok(OwnedValue::Scalar(ScalarValue::U8(lhs ^ rhs)))
}

pub(crate) fn u8_shift_left(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    let (value, bits) = expect_u8_u32("u8::shift_left", args)?;
    Ok(OwnedValue::Scalar(ScalarValue::U8(
        value.checked_shl(bits).unwrap_or(0),
    )))
}

pub(crate) fn u8_shift_right(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    let (value, bits) = expect_u8_u32("u8::shift_right", args)?;
    Ok(OwnedValue::Scalar(ScalarValue::U8(
        value.checked_shr(bits).unwrap_or(0),
    )))
}

pub(crate) fn u8_rotate_left(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    let (value, bits) = expect_u8_u32("u8::rotate_left", args)?;
    Ok(OwnedValue::Scalar(ScalarValue::U8(value.rotate_left(bits))))
}

pub(crate) fn u8_rotate_right(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    let (value, bits) = expect_u8_u32("u8::rotate_right", args)?;
    Ok(OwnedValue::Scalar(ScalarValue::U8(
        value.rotate_right(bits),
    )))
}

fn ok_scalar(value: ScalarValue) -> OwnedValue {
    option_result::owned_result_ok(OwnedValue::Scalar(value))
}

fn err_string(message: &str) -> OwnedValue {
    option_result::owned_result_err(OwnedValue::String(message.to_owned()))
}

fn expect_arity(name: &str, args: &[OwnedValue], expected: usize) -> VmResult<()> {
    if args.len() == expected {
        return Ok(());
    }
    Err(VmError::new(VmErrorKind::ArityMismatch {
        name: name.to_owned(),
        expected,
        actual: args.len(),
    }))
}

fn expect_u8_pair(operation: &'static str, args: &[OwnedValue]) -> VmResult<(u8, u8)> {
    expect_arity(operation, args, 2)?;
    let OwnedValue::Scalar(ScalarValue::U8(lhs)) = &args[0] else {
        return type_error(operation);
    };
    let OwnedValue::Scalar(ScalarValue::U8(rhs)) = &args[1] else {
        return type_error(operation);
    };
    Ok((*lhs, *rhs))
}

fn expect_u8_u32(operation: &'static str, args: &[OwnedValue]) -> VmResult<(u8, u32)> {
    expect_arity(operation, args, 2)?;
    let OwnedValue::Scalar(ScalarValue::U8(value)) = &args[0] else {
        return type_error(operation);
    };
    let OwnedValue::Scalar(ScalarValue::U32(bits)) = &args[1] else {
        return type_error(operation);
    };
    Ok((*value, *bits))
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
