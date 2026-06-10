use vela_reflect::registry::MethodDesc;

use super::{MethodSpec, ParamSpec, descs};

pub(crate) fn result_method_descs() -> Vec<MethodDesc> {
    descs("Result", RESULT_METHODS, "result")
}

const RESULT_METHODS: &[MethodSpec] = &[
    MethodSpec::new(
        "is_ok",
        &[],
        "bool",
        "Returns true when the result contains a success value.",
    ),
    MethodSpec::new(
        "is_err",
        &[],
        "bool",
        "Returns true when the result contains an error value.",
    ),
    MethodSpec::new(
        "unwrap_or",
        &[ParamSpec::new("default", "any")],
        "any",
        "Returns the success value or a default.",
    ),
    MethodSpec::new(
        "to_option",
        &[],
        "Option",
        "Converts Result::Ok to Option::Some and Result::Err to Option::None.",
    ),
    MethodSpec::new(
        "to_error_option",
        &[],
        "Option",
        "Converts Result::Err to Option::Some and Result::Ok to Option::None.",
    ),
    MethodSpec::new(
        "flatten",
        &[],
        "Result",
        "Flattens a nested dynamic Result value.",
    ),
    MethodSpec::new(
        "map",
        &[ParamSpec::new("callback", "function")],
        "Result",
        "Maps a Result::Ok payload with a callback.",
    ),
    MethodSpec::new(
        "map_err",
        &[ParamSpec::new("callback", "function")],
        "Result",
        "Maps a Result::Err payload with a callback.",
    ),
    MethodSpec::new(
        "and_then",
        &[ParamSpec::new("callback", "function")],
        "Result",
        "Chains a Result::Ok payload through a Result-returning callback.",
    ),
    MethodSpec::new(
        "or_else",
        &[ParamSpec::new("callback", "function")],
        "Result",
        "Calls an error-aware fallback callback for Result::Err.",
    ),
];
