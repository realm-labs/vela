use vela_common::HostMethodId;
use vela_reflect::{MethodDesc, MethodParamDesc};

use super::ids::{
    STRING_CHAR_AT_METHOD_ID, STRING_CONTAINS_METHOD_ID, STRING_ENDS_WITH_METHOD_ID,
    STRING_FIND_METHOD_ID, STRING_IS_EMPTY_METHOD_ID, STRING_LEN_METHOD_ID,
    STRING_PARSE_BOOL_METHOD_ID, STRING_PARSE_FLOAT_METHOD_ID, STRING_PARSE_INT_METHOD_ID,
    STRING_REPEAT_METHOD_ID, STRING_REPLACE_METHOD_ID, STRING_SLICE_METHOD_ID,
    STRING_SPLIT_LINES_METHOD_ID, STRING_SPLIT_METHOD_ID, STRING_SPLIT_ONCE_METHOD_ID,
    STRING_SPLIT_WHITESPACE_METHOD_ID, STRING_STARTS_WITH_METHOD_ID, STRING_STRIP_PREFIX_METHOD_ID,
    STRING_STRIP_SUFFIX_METHOD_ID, STRING_TO_LOWER_METHOD_ID, STRING_TO_UPPER_METHOD_ID,
    STRING_TRIM_END_METHOD_ID, STRING_TRIM_METHOD_ID, STRING_TRIM_START_METHOD_ID,
};

pub(crate) fn string_method_descs() -> Vec<MethodDesc> {
    vec![
        method(
            STRING_LEN_METHOD_ID,
            "len",
            &[],
            "int",
            "Returns the string length in characters.",
        ),
        method(
            STRING_IS_EMPTY_METHOD_ID,
            "is_empty",
            &[],
            "bool",
            "Returns true when the string has no bytes.",
        ),
        method(
            STRING_CONTAINS_METHOD_ID,
            "contains",
            &[("needle", "string")],
            "bool",
            "Returns true when the string contains the needle.",
        ),
        method(
            STRING_FIND_METHOD_ID,
            "find",
            &[("needle", "string")],
            "Option",
            "Returns the byte index of the first match, or Option.None.",
        ),
        method(
            STRING_STARTS_WITH_METHOD_ID,
            "starts_with",
            &[("prefix", "string")],
            "bool",
            "Returns true when the string starts with the prefix.",
        ),
        method(
            STRING_ENDS_WITH_METHOD_ID,
            "ends_with",
            &[("suffix", "string")],
            "bool",
            "Returns true when the string ends with the suffix.",
        ),
        method(
            STRING_STRIP_PREFIX_METHOD_ID,
            "strip_prefix",
            &[("prefix", "string")],
            "Option",
            "Returns the string without the prefix, or Option.None.",
        ),
        method(
            STRING_STRIP_SUFFIX_METHOD_ID,
            "strip_suffix",
            &[("suffix", "string")],
            "Option",
            "Returns the string without the suffix, or Option.None.",
        ),
        method(
            STRING_TO_UPPER_METHOD_ID,
            "to_upper",
            &[],
            "string",
            "Returns an uppercase copy.",
        ),
        method(
            STRING_TO_LOWER_METHOD_ID,
            "to_lower",
            &[],
            "string",
            "Returns a lowercase copy.",
        ),
        method(
            STRING_TRIM_METHOD_ID,
            "trim",
            &[],
            "string",
            "Returns a copy with leading and trailing whitespace removed.",
        ),
        method(
            STRING_TRIM_START_METHOD_ID,
            "trim_start",
            &[],
            "string",
            "Returns a copy with leading whitespace removed.",
        ),
        method(
            STRING_TRIM_END_METHOD_ID,
            "trim_end",
            &[],
            "string",
            "Returns a copy with trailing whitespace removed.",
        ),
        method(
            STRING_REPLACE_METHOD_ID,
            "replace",
            &[("old", "string"), ("new", "string")],
            "string",
            "Returns a copy with all matches replaced.",
        ),
        method(
            STRING_REPEAT_METHOD_ID,
            "repeat",
            &[("count", "int")],
            "string",
            "Returns the string repeated count times.",
        ),
        method(
            STRING_SLICE_METHOD_ID,
            "slice",
            &[("start", "int"), ("end", "int")],
            "string",
            "Returns the substring in the character range.",
        ),
        method(
            STRING_SPLIT_METHOD_ID,
            "split",
            &[("separator", "string")],
            "array",
            "Returns string segments split by the separator.",
        ),
        method(
            STRING_SPLIT_ONCE_METHOD_ID,
            "split_once",
            &[("separator", "string")],
            "Option",
            "Returns the first split pair, or Option.None.",
        ),
        method(
            STRING_SPLIT_LINES_METHOD_ID,
            "split_lines",
            &[],
            "array",
            "Returns the string split into lines.",
        ),
        method(
            STRING_SPLIT_WHITESPACE_METHOD_ID,
            "split_whitespace",
            &[],
            "array",
            "Returns the string split on whitespace.",
        ),
        method(
            STRING_CHAR_AT_METHOD_ID,
            "char_at",
            &[("index", "int")],
            "Option",
            "Returns the character at the character index, or Option.None.",
        ),
        method(
            STRING_PARSE_INT_METHOD_ID,
            "parse_int",
            &[],
            "Option",
            "Parses the string as an integer, or Option.None.",
        ),
        method(
            STRING_PARSE_FLOAT_METHOD_ID,
            "parse_float",
            &[],
            "Option",
            "Parses the string as a float, or Option.None.",
        ),
        method(
            STRING_PARSE_BOOL_METHOD_ID,
            "parse_bool",
            &[],
            "Option",
            "Parses the string as a boolean, or Option.None.",
        ),
    ]
}

fn method(
    id: HostMethodId,
    name: &'static str,
    params: &[(&'static str, &'static str)],
    return_type: &'static str,
    docs: &'static str,
) -> MethodDesc {
    let mut desc = MethodDesc::new(id, name)
        .return_type(return_type)
        .attr("stdlib", "string")
        .docs(docs);
    for (name, type_hint) in params {
        desc = desc.param(MethodParamDesc::new(*name).type_hint(*type_hint));
    }
    desc
}
