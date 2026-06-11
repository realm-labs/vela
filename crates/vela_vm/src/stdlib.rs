use crate::Vm;
use crate::VmResult;
use crate::owned_value::OwnedValue;
use vela_stdlib_runtime::{
    StdFunctionImplementation, StdFunctionRuntimeBinding, stdlib_function_runtime_bindings,
};

type StdNativeFunction = fn(&[OwnedValue]) -> VmResult<OwnedValue>;

pub(crate) fn register(vm: &mut Vm) {
    for binding in stdlib_function_runtime_bindings() {
        register_binding(vm, binding);
    }
}

fn register_binding(vm: &mut Vm, binding: StdFunctionRuntimeBinding) {
    vm.register_native_with_id(binding.id, native_function(binding.implementation));
}

fn native_function(implementation: StdFunctionImplementation) -> StdNativeFunction {
    match implementation {
        StdFunctionImplementation::MathMax => crate::math_stdlib::scalar::math_max,
        StdFunctionImplementation::MathMin => crate::math_stdlib::scalar::math_min,
        StdFunctionImplementation::MathClamp => crate::math_stdlib::scalar::math_clamp,
        StdFunctionImplementation::MathLerp => crate::math_stdlib::movement::math_lerp,
        StdFunctionImplementation::MathMoveTowards => {
            crate::math_stdlib::movement::math_move_towards
        }
        StdFunctionImplementation::MathDistance2d => crate::math_stdlib::distance::math_distance2d,
        StdFunctionImplementation::MathDistance3d => crate::math_stdlib::distance::math_distance3d,
        StdFunctionImplementation::MathPow => crate::math_stdlib::power::math_pow,
        StdFunctionImplementation::MathSqrt => crate::math_stdlib::root::math_sqrt,
        StdFunctionImplementation::MathSign => crate::math_stdlib::scalar::math_sign,
        StdFunctionImplementation::MathFloor => crate::math_stdlib::scalar::math_floor,
        StdFunctionImplementation::MathCeil => crate::math_stdlib::scalar::math_ceil,
        StdFunctionImplementation::MathRound => crate::math_stdlib::scalar::math_round,
        StdFunctionImplementation::MathAbs => crate::math_stdlib::scalar::math_abs,
        StdFunctionImplementation::OptionSome => crate::option_result::option_some,
        StdFunctionImplementation::OptionNone => crate::option_result::option_none,
        StdFunctionImplementation::OptionIsSome => crate::option_result::option_is_some,
        StdFunctionImplementation::OptionIsNone => crate::option_result::option_is_none,
        StdFunctionImplementation::OptionUnwrapOr => crate::option_result::option_unwrap_or,
        StdFunctionImplementation::OptionOkOr => crate::option_result::option_ok_or,
        StdFunctionImplementation::OptionFlatten => crate::option_result::option_flatten,
        StdFunctionImplementation::ResultOk => crate::option_result::result_ok,
        StdFunctionImplementation::ResultErr => crate::option_result::result_err,
        StdFunctionImplementation::ResultIsOk => crate::option_result::result_is_ok,
        StdFunctionImplementation::ResultIsErr => crate::option_result::result_is_err,
        StdFunctionImplementation::ResultUnwrapOr => crate::option_result::result_unwrap_or,
        StdFunctionImplementation::ResultToOption => crate::option_result::result_to_option,
        StdFunctionImplementation::ResultToErrorOption => {
            crate::option_result::result_to_error_option
        }
        StdFunctionImplementation::ResultFlatten => crate::option_result::result_flatten,
        StdFunctionImplementation::SetFromArray => crate::set_methods::from_array,
        StdFunctionImplementation::BytesFromHex => crate::bytes_methods::from_hex,
        StdFunctionImplementation::I32FromI16 => crate::numeric_conversions::i32_from_i16,
        StdFunctionImplementation::I64FromI32 => crate::numeric_conversions::i64_from_i32,
        StdFunctionImplementation::U32FromU16 => crate::numeric_conversions::u32_from_u16,
        StdFunctionImplementation::U64FromU32 => crate::numeric_conversions::u64_from_u32,
        StdFunctionImplementation::F64FromF32 => crate::numeric_conversions::f64_from_f32,
        StdFunctionImplementation::I16TryFromI64 => crate::numeric_conversions::i16_try_from_i64,
        StdFunctionImplementation::I8TryFromI64 => crate::numeric_conversions::i8_try_from_i64,
        StdFunctionImplementation::U16TryFromU64 => crate::numeric_conversions::u16_try_from_u64,
        StdFunctionImplementation::U8TryFromU64 => crate::numeric_conversions::u8_try_from_u64,
        StdFunctionImplementation::F32TryFromF64 => crate::numeric_conversions::f32_try_from_f64,
        StdFunctionImplementation::U8WrappingAdd => crate::numeric_conversions::u8_wrapping_add,
        StdFunctionImplementation::U32WrappingMul => crate::numeric_conversions::u32_wrapping_mul,
        StdFunctionImplementation::I8WrappingAdd => crate::numeric_conversions::i8_wrapping_add,
        StdFunctionImplementation::U8BitAnd => crate::numeric_conversions::u8_bit_and,
        StdFunctionImplementation::U8BitOr => crate::numeric_conversions::u8_bit_or,
        StdFunctionImplementation::U8BitXor => crate::numeric_conversions::u8_bit_xor,
        StdFunctionImplementation::U8ShiftLeft => crate::numeric_conversions::u8_shift_left,
        StdFunctionImplementation::U8ShiftRight => crate::numeric_conversions::u8_shift_right,
        StdFunctionImplementation::U8RotateLeft => crate::numeric_conversions::u8_rotate_left,
        StdFunctionImplementation::U8RotateRight => crate::numeric_conversions::u8_rotate_right,
    }
}

#[cfg(test)]
mod tests;
