use vela_common::HostMethodId;
use vela_reflect::registry::MethodDesc;

use super::{MethodSpec, ParamSpec, descs};

pub(crate) fn result_method_descs() -> Vec<MethodDesc> {
    descs(RESULT_METHODS, "result")
}

const RESULT_METHODS: &[MethodSpec] = &[
    MethodSpec::new(
        result_id(0),
        "is_ok",
        &[],
        "bool",
        "Returns true when the result contains a success value.",
    ),
    MethodSpec::new(
        result_id(1),
        "is_err",
        &[],
        "bool",
        "Returns true when the result contains an error value.",
    ),
    MethodSpec::new(
        result_id(2),
        "unwrap_or",
        &[ParamSpec::new("default", "any")],
        "any",
        "Returns the success value or a default.",
    ),
    MethodSpec::new(
        result_id(3),
        "to_option",
        &[],
        "Option",
        "Converts Result.Ok to Option.Some and Result.Err to Option.None.",
    ),
    MethodSpec::new(
        result_id(4),
        "to_error_option",
        &[],
        "Option",
        "Converts Result.Err to Option.Some and Result.Ok to Option.None.",
    ),
    MethodSpec::new(
        result_id(5),
        "flatten",
        &[],
        "Result",
        "Flattens a nested dynamic Result value.",
    ),
    MethodSpec::new(
        result_id(6),
        "map",
        &[ParamSpec::new("callback", "function")],
        "Result",
        "Maps a Result.Ok payload with a callback.",
    ),
    MethodSpec::new(
        result_id(7),
        "map_err",
        &[ParamSpec::new("callback", "function")],
        "Result",
        "Maps a Result.Err payload with a callback.",
    ),
    MethodSpec::new(
        result_id(8),
        "and_then",
        &[ParamSpec::new("callback", "function")],
        "Result",
        "Chains a Result.Ok payload through a Result-returning callback.",
    ),
    MethodSpec::new(
        result_id(9),
        "or_else",
        &[ParamSpec::new("callback", "function")],
        "Result",
        "Calls an error-aware fallback callback for Result.Err.",
    ),
];

const fn result_id(offset: u64) -> HostMethodId {
    HostMethodId::new(0xff00_0c00 + offset)
}
