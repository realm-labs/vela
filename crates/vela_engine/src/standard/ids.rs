use vela_common::{FieldId, HostMethodId, TypeId, VariantId};

pub use vela_common::standard_ids::{
    ARRAY_CLEAR_METHOD_ID, ARRAY_CONTAINS_METHOD_ID, ARRAY_IS_EMPTY_METHOD_ID, ARRAY_LEN_METHOD_ID,
    ARRAY_POP_METHOD_ID, ARRAY_PUSH_METHOD_ID, MAP_HAS_METHOD_ID, MAP_IS_EMPTY_METHOD_ID,
    MAP_LEN_METHOD_ID, MATH_ABS_FUNCTION_ID, MATH_CEIL_FUNCTION_ID, MATH_CLAMP_FUNCTION_ID,
    MATH_DISTANCE2D_FUNCTION_ID, MATH_DISTANCE3D_FUNCTION_ID, MATH_FLOOR_FUNCTION_ID,
    MATH_LERP_FUNCTION_ID, MATH_MAX_FUNCTION_ID, MATH_MIN_FUNCTION_ID,
    MATH_MOVE_TOWARDS_FUNCTION_ID, MATH_POW_FUNCTION_ID, MATH_ROUND_FUNCTION_ID,
    MATH_SIGN_FUNCTION_ID, MATH_SQRT_FUNCTION_ID, OPTION_FLATTEN_FUNCTION_ID,
    OPTION_IS_NONE_FUNCTION_ID, OPTION_IS_NONE_METHOD_ID, OPTION_IS_SOME_FUNCTION_ID,
    OPTION_IS_SOME_METHOD_ID, OPTION_NONE_FUNCTION_ID, OPTION_OK_OR_FUNCTION_ID,
    OPTION_SOME_FUNCTION_ID, OPTION_UNWRAP_OR_FUNCTION_ID, RANGE_IS_EMPTY_METHOD_ID,
    RANGE_LEN_METHOD_ID, RESULT_ERR_FUNCTION_ID, RESULT_FLATTEN_FUNCTION_ID,
    RESULT_IS_ERR_FUNCTION_ID, RESULT_IS_ERR_METHOD_ID, RESULT_IS_OK_FUNCTION_ID,
    RESULT_IS_OK_METHOD_ID, RESULT_OK_FUNCTION_ID, RESULT_TO_ERROR_OPTION_FUNCTION_ID,
    RESULT_TO_OPTION_FUNCTION_ID, RESULT_UNWRAP_OR_FUNCTION_ID, SET_FROM_ARRAY_FUNCTION_ID,
    SET_HAS_METHOD_ID, SET_IS_DISJOINT_METHOD_ID, SET_IS_EMPTY_METHOD_ID, SET_IS_SUBSET_METHOD_ID,
    SET_IS_SUPERSET_METHOD_ID, SET_LEN_METHOD_ID, STRING_IS_EMPTY_METHOD_ID, STRING_LEN_METHOD_ID,
};

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
pub const RANGE_TYPE_ID: TypeId = TypeId::new(0xff00_050a);
pub const OPTION_TYPE_ID: TypeId = TypeId::new(0xff00_0600);
pub const RESULT_TYPE_ID: TypeId = TypeId::new(0xff00_0601);

pub(crate) const OPTION_SOME_VARIANT_ID: VariantId = VariantId::new(0xff00_0602);
pub(crate) const OPTION_NONE_VARIANT_ID: VariantId = VariantId::new(0xff00_0603);
pub(crate) const RESULT_OK_VARIANT_ID: VariantId = VariantId::new(0xff00_0604);
pub(crate) const RESULT_ERR_VARIANT_ID: VariantId = VariantId::new(0xff00_0605);
pub(crate) const OPTION_SOME_FIELD_ID: FieldId = FieldId::new(0xff00_0606);
pub(crate) const RESULT_OK_FIELD_ID: FieldId = FieldId::new(0xff00_0607);
pub(crate) const RESULT_ERR_FIELD_ID: FieldId = FieldId::new(0xff00_0608);

pub(crate) const STRING_CONTAINS_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0702);
pub(crate) const STRING_FIND_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0703);
pub(crate) const STRING_STARTS_WITH_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0704);
pub(crate) const STRING_ENDS_WITH_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0705);
pub(crate) const STRING_STRIP_PREFIX_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0706);
pub(crate) const STRING_STRIP_SUFFIX_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0707);
pub(crate) const STRING_TO_UPPER_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0708);
pub(crate) const STRING_TO_LOWER_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0709);
pub(crate) const STRING_TRIM_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_070a);
pub(crate) const STRING_TRIM_START_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_070b);
pub(crate) const STRING_TRIM_END_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_070c);
pub(crate) const STRING_REPLACE_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_070d);
pub(crate) const STRING_REPEAT_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_070e);
pub(crate) const STRING_SLICE_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_070f);
pub(crate) const STRING_SPLIT_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0710);
pub(crate) const STRING_SPLIT_ONCE_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0711);
pub(crate) const STRING_SPLIT_LINES_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0712);
pub(crate) const STRING_SPLIT_WHITESPACE_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0713);
pub(crate) const STRING_CHAR_AT_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0714);
pub(crate) const STRING_PARSE_INT_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0715);
pub(crate) const STRING_PARSE_FLOAT_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0716);
pub(crate) const STRING_PARSE_BOOL_METHOD_ID: HostMethodId = HostMethodId::new(0xff00_0717);
