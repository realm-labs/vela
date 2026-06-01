use vela_common::{FieldId, FunctionId, TypeId, VariantId};

use crate::NativeFunctionId;

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

pub const OPTION_SOME_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0200);
pub const OPTION_NONE_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0201);
pub const OPTION_IS_SOME_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0202);
pub const OPTION_IS_NONE_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0203);
pub const OPTION_UNWRAP_OR_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0204);
pub const OPTION_OK_OR_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0205);
pub const OPTION_FLATTEN_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0206);

pub const RESULT_OK_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0300);
pub const RESULT_ERR_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0301);
pub const RESULT_IS_OK_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0302);
pub const RESULT_IS_ERR_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0303);
pub const RESULT_UNWRAP_OR_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0304);
pub const RESULT_TO_OPTION_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0305);
pub const RESULT_TO_ERROR_OPTION_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0306);
pub const RESULT_FLATTEN_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0307);

pub const SET_FROM_ARRAY_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0400);

pub const NULL_TYPE_ID: TypeId = TypeId::new(0xff00_0500);
pub const BOOL_TYPE_ID: TypeId = TypeId::new(0xff00_0501);
pub const INT_TYPE_ID: TypeId = TypeId::new(0xff00_0502);
pub const FLOAT_TYPE_ID: TypeId = TypeId::new(0xff00_0503);
pub const STRING_TYPE_ID: TypeId = TypeId::new(0xff00_0504);
pub const ARRAY_TYPE_ID: TypeId = TypeId::new(0xff00_0505);
pub const MAP_TYPE_ID: TypeId = TypeId::new(0xff00_0506);
pub const SET_TYPE_ID: TypeId = TypeId::new(0xff00_0507);
pub const FUNCTION_TYPE_ID: TypeId = TypeId::new(0xff00_0508);
pub const CLOSURE_TYPE_ID: TypeId = TypeId::new(0xff00_0509);
pub const OPTION_TYPE_ID: TypeId = TypeId::new(0xff00_0600);
pub const RESULT_TYPE_ID: TypeId = TypeId::new(0xff00_0601);

pub(crate) const OPTION_SOME_VARIANT_ID: VariantId = VariantId::new(0xff00_0602);
pub(crate) const OPTION_NONE_VARIANT_ID: VariantId = VariantId::new(0xff00_0603);
pub(crate) const RESULT_OK_VARIANT_ID: VariantId = VariantId::new(0xff00_0604);
pub(crate) const RESULT_ERR_VARIANT_ID: VariantId = VariantId::new(0xff00_0605);
pub(crate) const OPTION_SOME_FIELD_ID: FieldId = FieldId::new(0xff00_0606);
pub(crate) const RESULT_OK_FIELD_ID: FieldId = FieldId::new(0xff00_0607);
pub(crate) const RESULT_ERR_FIELD_ID: FieldId = FieldId::new(0xff00_0608);
