use vela_common::FunctionId;

use crate::{EffectSet, FunctionAccess, NativeFunctionDesc, NativeFunctionId, TypeHint};

pub const MATH_MAX_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0100);
pub const MATH_MIN_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0101);
pub const MATH_CLAMP_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0102);
pub const MATH_LERP_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0103);
pub const MATH_MOVE_TOWARDS_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0104);
pub const MATH_DISTANCE2D_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0105);
pub const MATH_DISTANCE3D_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0106);
pub const MATH_POW_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0107);
pub const MATH_SQRT_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0108);
pub const MATH_SIGN_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0109);
pub const MATH_FLOOR_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_010a);
pub const MATH_CEIL_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_010b);
pub const MATH_ROUND_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_010c);
pub const MATH_ABS_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_010d);

pub(crate) fn standard_native_function_descs() -> Vec<NativeFunctionDesc> {
    vec![
        math_binary(
            "math.max",
            MATH_MAX_FUNCTION_ID,
            "left",
            "right",
            TypeHint::Any,
        ),
        math_binary(
            "math.min",
            MATH_MIN_FUNCTION_ID,
            "left",
            "right",
            TypeHint::Any,
        ),
        math_ternary(
            "math.clamp",
            MATH_CLAMP_FUNCTION_ID,
            ["value", "min", "max"],
            TypeHint::Any,
        ),
        math_ternary(
            "math.lerp",
            MATH_LERP_FUNCTION_ID,
            ["start", "end", "t"],
            TypeHint::Float,
        ),
        math_ternary(
            "math.move_towards",
            MATH_MOVE_TOWARDS_FUNCTION_ID,
            ["current", "target", "max_delta"],
            TypeHint::Any,
        ),
        math_distance2d(),
        math_distance3d(),
        math_binary(
            "math.pow",
            MATH_POW_FUNCTION_ID,
            "base",
            "exponent",
            TypeHint::Any,
        ),
        math_unary("math.sqrt", MATH_SQRT_FUNCTION_ID, TypeHint::Float),
        math_unary("math.sign", MATH_SIGN_FUNCTION_ID, TypeHint::Int),
        math_unary("math.floor", MATH_FLOOR_FUNCTION_ID, TypeHint::Int),
        math_unary("math.ceil", MATH_CEIL_FUNCTION_ID, TypeHint::Int),
        math_unary("math.round", MATH_ROUND_FUNCTION_ID, TypeHint::Int),
        math_unary("math.abs", MATH_ABS_FUNCTION_ID, TypeHint::Any),
    ]
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
    math_desc("math.distance2d", MATH_DISTANCE2D_FUNCTION_ID)
        .param("x1", TypeHint::Any)
        .param("y1", TypeHint::Any)
        .param("x2", TypeHint::Any)
        .param("y2", TypeHint::Any)
        .returns(TypeHint::Float)
}

fn math_distance3d() -> NativeFunctionDesc {
    math_desc("math.distance3d", MATH_DISTANCE3D_FUNCTION_ID)
        .param("x1", TypeHint::Any)
        .param("y1", TypeHint::Any)
        .param("z1", TypeHint::Any)
        .param("x2", TypeHint::Any)
        .param("y2", TypeHint::Any)
        .param("z2", TypeHint::Any)
        .returns(TypeHint::Float)
}

fn math_desc(name: &'static str, id: NativeFunctionId) -> NativeFunctionDesc {
    NativeFunctionDesc::new(name, id)
        .effects(EffectSet::pure())
        .access(FunctionAccess::public().reflect_callable(true))
        .docs("Deterministic math standard-library helper.")
        .attr("stdlib", "math")
}
