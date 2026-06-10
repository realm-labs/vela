use crate::{HostMethodId, stable_id};
use vela_def::FunctionId;

const fn std_function(module: &str, name: &str) -> FunctionId {
    FunctionId::new(stable_id("std_function", module, name) as u128)
}

const fn std_method(owner: &str, name: &str) -> HostMethodId {
    HostMethodId::new(stable_id("std_method", owner, name))
}

pub const MATH_MAX_FUNCTION_ID: FunctionId = std_function("math", "max");
pub const MATH_MIN_FUNCTION_ID: FunctionId = std_function("math", "min");
pub const MATH_CLAMP_FUNCTION_ID: FunctionId = std_function("math", "clamp");
pub const MATH_LERP_FUNCTION_ID: FunctionId = std_function("math", "lerp");
pub const MATH_MOVE_TOWARDS_FUNCTION_ID: FunctionId = std_function("math", "move_towards");
pub const MATH_DISTANCE2D_FUNCTION_ID: FunctionId = std_function("math", "distance2d");
pub const MATH_DISTANCE3D_FUNCTION_ID: FunctionId = std_function("math", "distance3d");
pub const MATH_POW_FUNCTION_ID: FunctionId = std_function("math", "pow");
pub const MATH_SQRT_FUNCTION_ID: FunctionId = std_function("math", "sqrt");
pub const MATH_SIGN_FUNCTION_ID: FunctionId = std_function("math", "sign");
pub const MATH_FLOOR_FUNCTION_ID: FunctionId = std_function("math", "floor");
pub const MATH_CEIL_FUNCTION_ID: FunctionId = std_function("math", "ceil");
pub const MATH_ROUND_FUNCTION_ID: FunctionId = std_function("math", "round");
pub const MATH_ABS_FUNCTION_ID: FunctionId = std_function("math", "abs");

pub const OPTION_SOME_FUNCTION_ID: FunctionId = std_function("option", "some");
pub const OPTION_NONE_FUNCTION_ID: FunctionId = std_function("option", "none");
pub const OPTION_IS_SOME_FUNCTION_ID: FunctionId = std_function("option", "is_some");
pub const OPTION_IS_NONE_FUNCTION_ID: FunctionId = std_function("option", "is_none");
pub const OPTION_UNWRAP_OR_FUNCTION_ID: FunctionId = std_function("option", "unwrap_or");
pub const OPTION_OK_OR_FUNCTION_ID: FunctionId = std_function("option", "ok_or");
pub const OPTION_FLATTEN_FUNCTION_ID: FunctionId = std_function("option", "flatten");

pub const RESULT_OK_FUNCTION_ID: FunctionId = std_function("result", "ok");
pub const RESULT_ERR_FUNCTION_ID: FunctionId = std_function("result", "err");
pub const RESULT_IS_OK_FUNCTION_ID: FunctionId = std_function("result", "is_ok");
pub const RESULT_IS_ERR_FUNCTION_ID: FunctionId = std_function("result", "is_err");
pub const RESULT_UNWRAP_OR_FUNCTION_ID: FunctionId = std_function("result", "unwrap_or");
pub const RESULT_TO_OPTION_FUNCTION_ID: FunctionId = std_function("result", "to_option");
pub const RESULT_TO_ERROR_OPTION_FUNCTION_ID: FunctionId =
    std_function("result", "to_error_option");
pub const RESULT_FLATTEN_FUNCTION_ID: FunctionId = std_function("result", "flatten");

pub const SET_FROM_ARRAY_FUNCTION_ID: FunctionId = std_function("set", "from_array");

pub const STRING_LEN_METHOD_ID: HostMethodId = std_method("String", "len");
pub const STRING_IS_EMPTY_METHOD_ID: HostMethodId = std_method("String", "is_empty");
pub const STRING_CONTAINS_METHOD_ID: HostMethodId = std_method("String", "contains");
pub const STRING_FIND_METHOD_ID: HostMethodId = std_method("String", "find");
pub const STRING_STARTS_WITH_METHOD_ID: HostMethodId = std_method("String", "starts_with");
pub const STRING_ENDS_WITH_METHOD_ID: HostMethodId = std_method("String", "ends_with");
pub const STRING_STRIP_PREFIX_METHOD_ID: HostMethodId = std_method("String", "strip_prefix");
pub const STRING_STRIP_SUFFIX_METHOD_ID: HostMethodId = std_method("String", "strip_suffix");
pub const STRING_TO_UPPER_METHOD_ID: HostMethodId = std_method("String", "to_upper");
pub const STRING_TO_LOWER_METHOD_ID: HostMethodId = std_method("String", "to_lower");
pub const STRING_TRIM_METHOD_ID: HostMethodId = std_method("String", "trim");
pub const STRING_TRIM_START_METHOD_ID: HostMethodId = std_method("String", "trim_start");
pub const STRING_TRIM_END_METHOD_ID: HostMethodId = std_method("String", "trim_end");
pub const STRING_REPLACE_METHOD_ID: HostMethodId = std_method("String", "replace");
pub const STRING_REPEAT_METHOD_ID: HostMethodId = std_method("String", "repeat");
pub const STRING_SLICE_METHOD_ID: HostMethodId = std_method("String", "slice");
pub const STRING_SPLIT_METHOD_ID: HostMethodId = std_method("String", "split");
pub const STRING_SPLIT_ONCE_METHOD_ID: HostMethodId = std_method("String", "split_once");
pub const STRING_SPLIT_LINES_METHOD_ID: HostMethodId = std_method("String", "split_lines");
pub const STRING_SPLIT_WHITESPACE_METHOD_ID: HostMethodId =
    std_method("String", "split_whitespace");
pub const STRING_CHAR_AT_METHOD_ID: HostMethodId = std_method("String", "char_at");
pub const STRING_PARSE_INT_METHOD_ID: HostMethodId = std_method("String", "parse_int");
pub const STRING_PARSE_FLOAT_METHOD_ID: HostMethodId = std_method("String", "parse_float");
pub const STRING_PARSE_BOOL_METHOD_ID: HostMethodId = std_method("String", "parse_bool");

pub const ARRAY_LEN_METHOD_ID: HostMethodId = std_method("Array", "len");
pub const ARRAY_IS_EMPTY_METHOD_ID: HostMethodId = std_method("Array", "is_empty");
pub const ARRAY_PUSH_METHOD_ID: HostMethodId = std_method("Array", "push");
pub const ARRAY_POP_METHOD_ID: HostMethodId = std_method("Array", "pop");
pub const ARRAY_CLEAR_METHOD_ID: HostMethodId = std_method("Array", "clear");
pub const ARRAY_FIRST_METHOD_ID: HostMethodId = std_method("Array", "first");
pub const ARRAY_LAST_METHOD_ID: HostMethodId = std_method("Array", "last");
pub const ARRAY_JOIN_METHOD_ID: HostMethodId = std_method("Array", "join");
pub const ARRAY_CONTAINS_METHOD_ID: HostMethodId = std_method("Array", "contains");
pub const ARRAY_INDEX_OF_METHOD_ID: HostMethodId = std_method("Array", "index_of");
pub const ARRAY_DISTINCT_METHOD_ID: HostMethodId = std_method("Array", "distinct");
pub const ARRAY_REVERSE_METHOD_ID: HostMethodId = std_method("Array", "reverse");
pub const ARRAY_SLICE_METHOD_ID: HostMethodId = std_method("Array", "slice");

pub const MAP_LEN_METHOD_ID: HostMethodId = std_method("Map", "len");
pub const MAP_IS_EMPTY_METHOD_ID: HostMethodId = std_method("Map", "is_empty");
pub const MAP_HAS_METHOD_ID: HostMethodId = std_method("Map", "has");
pub const MAP_SET_METHOD_ID: HostMethodId = std_method("Map", "set");
pub const MAP_REMOVE_METHOD_ID: HostMethodId = std_method("Map", "remove");
pub const MAP_CLEAR_METHOD_ID: HostMethodId = std_method("Map", "clear");

pub const SET_LEN_METHOD_ID: HostMethodId = std_method("Set", "len");
pub const SET_IS_EMPTY_METHOD_ID: HostMethodId = std_method("Set", "is_empty");
pub const SET_HAS_METHOD_ID: HostMethodId = std_method("Set", "has");
pub const SET_ADD_METHOD_ID: HostMethodId = std_method("Set", "add");
pub const SET_REMOVE_METHOD_ID: HostMethodId = std_method("Set", "remove");
pub const SET_CLEAR_METHOD_ID: HostMethodId = std_method("Set", "clear");
pub const SET_IS_SUBSET_METHOD_ID: HostMethodId = std_method("Set", "is_subset");
pub const SET_IS_SUPERSET_METHOD_ID: HostMethodId = std_method("Set", "is_superset");
pub const SET_IS_DISJOINT_METHOD_ID: HostMethodId = std_method("Set", "is_disjoint");

pub const OPTION_IS_SOME_METHOD_ID: HostMethodId = std_method("Option", "is_some");
pub const OPTION_IS_NONE_METHOD_ID: HostMethodId = std_method("Option", "is_none");

pub const RESULT_IS_OK_METHOD_ID: HostMethodId = std_method("Result", "is_ok");
pub const RESULT_IS_ERR_METHOD_ID: HostMethodId = std_method("Result", "is_err");

pub const RANGE_LEN_METHOD_ID: HostMethodId = std_method("Range", "len");
pub const RANGE_IS_EMPTY_METHOD_ID: HostMethodId = std_method("Range", "is_empty");
