use crate::native::{EffectSet, FunctionAccess, NativeFunctionDesc, NativeFunctionId, TypeHint};

use super::ids::{
    MATH_ABS_FUNCTION_ID, MATH_CEIL_FUNCTION_ID, MATH_CLAMP_FUNCTION_ID,
    MATH_DISTANCE2D_FUNCTION_ID, MATH_DISTANCE3D_FUNCTION_ID, MATH_FLOOR_FUNCTION_ID,
    MATH_LERP_FUNCTION_ID, MATH_MAX_FUNCTION_ID, MATH_MIN_FUNCTION_ID,
    MATH_MOVE_TOWARDS_FUNCTION_ID, MATH_POW_FUNCTION_ID, MATH_ROUND_FUNCTION_ID,
    MATH_SIGN_FUNCTION_ID, MATH_SQRT_FUNCTION_ID, OPTION_FLATTEN_FUNCTION_ID,
    OPTION_IS_NONE_FUNCTION_ID, OPTION_IS_SOME_FUNCTION_ID, OPTION_NONE_FUNCTION_ID,
    OPTION_OK_OR_FUNCTION_ID, OPTION_SOME_FUNCTION_ID, OPTION_UNWRAP_OR_FUNCTION_ID,
    RESULT_ERR_FUNCTION_ID, RESULT_FLATTEN_FUNCTION_ID, RESULT_IS_ERR_FUNCTION_ID,
    RESULT_IS_OK_FUNCTION_ID, RESULT_OK_FUNCTION_ID, RESULT_TO_ERROR_OPTION_FUNCTION_ID,
    RESULT_TO_OPTION_FUNCTION_ID, RESULT_UNWRAP_OR_FUNCTION_ID, SET_FROM_ARRAY_FUNCTION_ID,
};

pub(crate) fn standard_native_function_descs() -> Vec<NativeFunctionDesc> {
    let mut descs = Vec::new();
    descs.extend(math_descs());
    descs.extend(option_descs());
    descs.extend(result_descs());
    descs.push(set_from_array_desc());
    descs
}

fn math_descs() -> [NativeFunctionDesc; 14] {
    [
        math_binary(
            "math::max",
            MATH_MAX_FUNCTION_ID,
            "left",
            "right",
            TypeHint::Any,
        )
        .docs("Returns the larger numeric value."),
        math_binary(
            "math::min",
            MATH_MIN_FUNCTION_ID,
            "left",
            "right",
            TypeHint::Any,
        )
        .docs("Returns the smaller numeric value."),
        math_ternary(
            "math::clamp",
            MATH_CLAMP_FUNCTION_ID,
            ["value", "min", "max"],
            TypeHint::Any,
        )
        .docs("Clamps a numeric value between inclusive bounds."),
        math_ternary(
            "math::lerp",
            MATH_LERP_FUNCTION_ID,
            ["start", "end", "t"],
            TypeHint::Float,
        )
        .docs("Linearly interpolates between two numeric values."),
        math_ternary(
            "math::move_towards",
            MATH_MOVE_TOWARDS_FUNCTION_ID,
            ["current", "target", "max_delta"],
            TypeHint::Any,
        )
        .docs("Moves a numeric value toward a target by at most max_delta."),
        math_distance2d(),
        math_distance3d(),
        math_binary(
            "math::pow",
            MATH_POW_FUNCTION_ID,
            "base",
            "exponent",
            TypeHint::Any,
        )
        .docs("Raises a numeric base to a numeric exponent."),
        math_unary("math::sqrt", MATH_SQRT_FUNCTION_ID, TypeHint::Float)
            .docs("Returns the square root as a float."),
        math_unary("math::sign", MATH_SIGN_FUNCTION_ID, TypeHint::Int)
            .docs("Returns -1, 0, or 1 for the numeric sign."),
        math_unary("math::floor", MATH_FLOOR_FUNCTION_ID, TypeHint::Int)
            .docs("Rounds a numeric value down to an integer."),
        math_unary("math::ceil", MATH_CEIL_FUNCTION_ID, TypeHint::Int)
            .docs("Rounds a numeric value up to an integer."),
        math_unary("math::round", MATH_ROUND_FUNCTION_ID, TypeHint::Int)
            .docs("Rounds a numeric value to the nearest integer."),
        math_unary("math::abs", MATH_ABS_FUNCTION_ID, TypeHint::Any)
            .docs("Returns the absolute numeric value."),
    ]
}

fn option_descs() -> [NativeFunctionDesc; 7] {
    [
        option_desc("option::some", OPTION_SOME_FUNCTION_ID)
            .param("value", TypeHint::Any)
            .returns(TypeHint::Any)
            .docs("Wraps a value in Option::Some."),
        option_desc("option::none", OPTION_NONE_FUNCTION_ID)
            .returns(TypeHint::Any)
            .docs("Creates Option::None."),
        option_desc("option::is_some", OPTION_IS_SOME_FUNCTION_ID)
            .param("option", TypeHint::Any)
            .returns(TypeHint::Bool)
            .docs("Returns true when the value is Option::Some."),
        option_desc("option::is_none", OPTION_IS_NONE_FUNCTION_ID)
            .param("option", TypeHint::Any)
            .returns(TypeHint::Bool)
            .docs("Returns true when the value is Option::None."),
        option_desc("option::unwrap_or", OPTION_UNWRAP_OR_FUNCTION_ID)
            .param("option", TypeHint::Any)
            .param("fallback", TypeHint::Any)
            .returns(TypeHint::Any)
            .docs("Returns the Option::Some payload or a fallback value."),
        option_desc("option::ok_or", OPTION_OK_OR_FUNCTION_ID)
            .param("option", TypeHint::Any)
            .param("error", TypeHint::Any)
            .returns(TypeHint::Any)
            .docs("Converts Option::Some to Result::Ok or Option::None to Result::Err."),
        option_desc("option::flatten", OPTION_FLATTEN_FUNCTION_ID)
            .param("option", TypeHint::Any)
            .returns(TypeHint::Any)
            .docs("Flattens a nested Option value by one nesting layer."),
    ]
}

fn result_descs() -> [NativeFunctionDesc; 8] {
    [
        result_desc("result::ok", RESULT_OK_FUNCTION_ID)
            .param("value", TypeHint::Any)
            .returns(TypeHint::Any)
            .docs("Wraps a success value in Result::Ok."),
        result_desc("result::err", RESULT_ERR_FUNCTION_ID)
            .param("error", TypeHint::Any)
            .returns(TypeHint::Any)
            .docs("Wraps an error value in Result::Err."),
        result_desc("result::is_ok", RESULT_IS_OK_FUNCTION_ID)
            .param("result", TypeHint::Any)
            .returns(TypeHint::Bool)
            .docs("Returns true when the value is Result::Ok."),
        result_desc("result::is_err", RESULT_IS_ERR_FUNCTION_ID)
            .param("result", TypeHint::Any)
            .returns(TypeHint::Bool)
            .docs("Returns true when the value is Result::Err."),
        result_desc("result::unwrap_or", RESULT_UNWRAP_OR_FUNCTION_ID)
            .param("result", TypeHint::Any)
            .param("fallback", TypeHint::Any)
            .returns(TypeHint::Any)
            .docs("Returns the Result::Ok payload or a fallback value."),
        result_desc("result::to_option", RESULT_TO_OPTION_FUNCTION_ID)
            .param("result", TypeHint::Any)
            .returns(TypeHint::Any)
            .docs("Converts Result::Ok to Option::Some and Result::Err to Option::None."),
        result_desc(
            "result::to_error_option",
            RESULT_TO_ERROR_OPTION_FUNCTION_ID,
        )
        .param("result", TypeHint::Any)
        .returns(TypeHint::Any)
        .docs("Converts Result::Err to Option::Some and Result::Ok to Option::None."),
        result_desc("result::flatten", RESULT_FLATTEN_FUNCTION_ID)
            .param("result", TypeHint::Any)
            .returns(TypeHint::Any)
            .docs("Flattens a nested Result value by one nesting layer."),
    ]
}

fn set_from_array_desc() -> NativeFunctionDesc {
    stdlib_desc("set::from_array", SET_FROM_ARRAY_FUNCTION_ID, "set")
        .param("values", TypeHint::Array)
        .returns(TypeHint::Set)
        .docs("Builds a set from array values.")
}

fn math_unary(name: &'static str, id: NativeFunctionId, returns: TypeHint) -> NativeFunctionDesc {
    math_desc(name, id)
        .param("value", TypeHint::Any)
        .returns(returns)
}

fn math_binary(
    name: &'static str,
    id: NativeFunctionId,
    first: &'static str,
    second: &'static str,
    returns: TypeHint,
) -> NativeFunctionDesc {
    math_desc(name, id)
        .param(first, TypeHint::Any)
        .param(second, TypeHint::Any)
        .returns(returns)
}

fn math_ternary(
    name: &'static str,
    id: NativeFunctionId,
    params: [&'static str; 3],
    returns: TypeHint,
) -> NativeFunctionDesc {
    math_desc(name, id)
        .param(params[0], TypeHint::Any)
        .param(params[1], TypeHint::Any)
        .param(params[2], TypeHint::Any)
        .returns(returns)
}

fn math_distance2d() -> NativeFunctionDesc {
    math_desc("math::distance2d", MATH_DISTANCE2D_FUNCTION_ID)
        .param("x1", TypeHint::Any)
        .param("y1", TypeHint::Any)
        .param("x2", TypeHint::Any)
        .param("y2", TypeHint::Any)
        .returns(TypeHint::Float)
        .docs("Returns the 2D distance between two points.")
}

fn math_distance3d() -> NativeFunctionDesc {
    math_desc("math::distance3d", MATH_DISTANCE3D_FUNCTION_ID)
        .param("x1", TypeHint::Any)
        .param("y1", TypeHint::Any)
        .param("z1", TypeHint::Any)
        .param("x2", TypeHint::Any)
        .param("y2", TypeHint::Any)
        .param("z2", TypeHint::Any)
        .returns(TypeHint::Float)
        .docs("Returns the 3D distance between two points.")
}

fn math_desc(name: &'static str, id: NativeFunctionId) -> NativeFunctionDesc {
    stdlib_desc(name, id, "math")
}

fn option_desc(name: &'static str, id: NativeFunctionId) -> NativeFunctionDesc {
    stdlib_desc(name, id, "option")
}

fn result_desc(name: &'static str, id: NativeFunctionId) -> NativeFunctionDesc {
    stdlib_desc(name, id, "result")
}

fn stdlib_desc(
    name: &'static str,
    id: NativeFunctionId,
    namespace: &'static str,
) -> NativeFunctionDesc {
    NativeFunctionDesc::new(name, id)
        .effects(EffectSet::pure())
        .access(FunctionAccess::public().reflect_callable(true))
        .attr("stdlib", namespace)
}
