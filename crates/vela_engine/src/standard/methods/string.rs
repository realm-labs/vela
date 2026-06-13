use vela_reflect::registry::MethodDesc;

use super::{MethodSpec, ParamSpec, descs};

pub(crate) fn string_method_descs() -> Vec<MethodDesc> {
    descs("String", STRING_METHODS, "string")
}

const STRING_METHODS: &[MethodSpec] = &[
    MethodSpec::new("len", &[], "i64", "Returns the string length in bytes."),
    MethodSpec::new(
        "is_empty",
        &[],
        "bool",
        "Returns true when the string has no characters.",
    ),
    MethodSpec::new(
        "contains",
        &[ParamSpec::new("needle", "string")],
        "bool",
        "Returns true when the string contains the needle.",
    ),
    MethodSpec::new(
        "find",
        &[ParamSpec::new("needle", "string")],
        "Option",
        "Returns the byte index of the first match, or Option::None.",
    ),
    MethodSpec::new(
        "starts_with",
        &[ParamSpec::new("prefix", "string")],
        "bool",
        "Returns true when the string starts with the prefix.",
    ),
    MethodSpec::new(
        "ends_with",
        &[ParamSpec::new("suffix", "string")],
        "bool",
        "Returns true when the string ends with the suffix.",
    ),
    MethodSpec::new(
        "strip_prefix",
        &[ParamSpec::new("prefix", "string")],
        "Option",
        "Returns the string without the prefix, or Option::None.",
    ),
    MethodSpec::new(
        "strip_suffix",
        &[ParamSpec::new("suffix", "string")],
        "Option",
        "Returns the string without the suffix, or Option::None.",
    ),
    MethodSpec::new("to_upper", &[], "string", "Returns an uppercase copy."),
    MethodSpec::new("to_lower", &[], "string", "Returns a lowercase copy."),
    MethodSpec::new(
        "trim",
        &[],
        "string",
        "Returns a copy with leading and trailing whitespace removed.",
    ),
    MethodSpec::new(
        "trim_start",
        &[],
        "string",
        "Returns a copy with leading whitespace removed.",
    ),
    MethodSpec::new(
        "trim_end",
        &[],
        "string",
        "Returns a copy with trailing whitespace removed.",
    ),
    MethodSpec::new(
        "replace",
        &[
            ParamSpec::new("old", "string"),
            ParamSpec::new("new", "string"),
        ],
        "string",
        "Returns a copy with all matches replaced.",
    ),
    MethodSpec::new(
        "repeat",
        &[ParamSpec::new("count", "i64")],
        "string",
        "Returns the string repeated count times.",
    ),
    MethodSpec::new(
        "slice",
        &[ParamSpec::new("start", "i64"), ParamSpec::new("end", "i64")],
        "string",
        "Returns the substring in the byte range.",
    ),
    MethodSpec::new(
        "split",
        &[ParamSpec::new("separator", "string")],
        "array",
        "Returns string segments split by the separator.",
    ),
    MethodSpec::new(
        "split_once",
        &[ParamSpec::new("separator", "string")],
        "Option",
        "Returns the first split pair, or Option::None.",
    ),
    MethodSpec::new(
        "split_lines",
        &[],
        "array",
        "Returns the string split into lines.",
    ),
    MethodSpec::new(
        "split_whitespace",
        &[],
        "array",
        "Returns the string split on whitespace.",
    ),
    MethodSpec::new(
        "parse_int",
        &[],
        "Option",
        "Parses the string as an integer, or Option::None.",
    ),
    MethodSpec::new(
        "parse_float",
        &[],
        "Option",
        "Parses the string as a float, or Option::None.",
    ),
    MethodSpec::new(
        "parse_bool",
        &[],
        "Option",
        "Parses the string as a boolean, or Option::None.",
    ),
];
