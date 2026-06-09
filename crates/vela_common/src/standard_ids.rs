use crate::{FunctionId, HostMethodId};

pub const MATH_MAX_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0100);
pub const MATH_MIN_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0101);
pub const MATH_CLAMP_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0102);
pub const MATH_LERP_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0103);
pub const MATH_MOVE_TOWARDS_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0104);
pub const MATH_DISTANCE2D_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0105);
pub const MATH_DISTANCE3D_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0106);
pub const MATH_POW_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0107);
pub const MATH_SQRT_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0108);
pub const MATH_SIGN_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0109);
pub const MATH_FLOOR_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_010a);
pub const MATH_CEIL_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_010b);
pub const MATH_ROUND_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_010c);
pub const MATH_ABS_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_010d);

pub const OPTION_SOME_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0200);
pub const OPTION_NONE_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0201);
pub const OPTION_IS_SOME_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0202);
pub const OPTION_IS_NONE_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0203);
pub const OPTION_UNWRAP_OR_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0204);
pub const OPTION_OK_OR_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0205);
pub const OPTION_FLATTEN_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0206);

pub const RESULT_OK_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0300);
pub const RESULT_ERR_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0301);
pub const RESULT_IS_OK_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0302);
pub const RESULT_IS_ERR_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0303);
pub const RESULT_UNWRAP_OR_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0304);
pub const RESULT_TO_OPTION_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0305);
pub const RESULT_TO_ERROR_OPTION_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0306);
pub const RESULT_FLATTEN_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0307);

pub const SET_FROM_ARRAY_FUNCTION_ID: FunctionId = FunctionId::new(0xff00_0400);

pub const STRING_LEN_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0700);
pub const STRING_IS_EMPTY_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0701);

pub const ARRAY_LEN_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0800);
pub const ARRAY_IS_EMPTY_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0801);

pub const MAP_LEN_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0900);
pub const MAP_IS_EMPTY_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0901);

pub const SET_LEN_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0a00);
pub const SET_IS_EMPTY_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0a01);

pub const OPTION_IS_SOME_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0b00);
pub const OPTION_IS_NONE_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0b01);

pub const RESULT_IS_OK_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0c00);
pub const RESULT_IS_ERR_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0c01);

pub const RANGE_LEN_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0d00);
pub const RANGE_IS_EMPTY_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0d01);
