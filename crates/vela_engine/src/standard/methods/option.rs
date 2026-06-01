use vela_common::HostMethodId;
use vela_reflect::MethodDesc;

use super::{MethodSpec, ParamSpec, descs};

pub(crate) fn option_method_descs() -> Vec<MethodDesc> {
    descs(OPTION_METHODS, "option")
}

const OPTION_METHODS: &[MethodSpec] = &[
    MethodSpec::new(
        option_id(0),
        "is_some",
        &[],
        "bool",
        "Returns true when the option contains a value.",
    ),
    MethodSpec::new(
        option_id(1),
        "is_none",
        &[],
        "bool",
        "Returns true when the option is empty.",
    ),
    MethodSpec::new(
        option_id(2),
        "unwrap_or",
        &[ParamSpec::new("default", "any")],
        "any",
        "Returns the contained value or a default.",
    ),
    MethodSpec::new(
        option_id(3),
        "ok_or",
        &[ParamSpec::new("error", "any")],
        "Result",
        "Converts Option.None to Result.Err with an error value.",
    ),
    MethodSpec::new(
        option_id(4),
        "flatten",
        &[],
        "Option",
        "Flattens a nested dynamic Option value.",
    ),
    MethodSpec::new(
        option_id(5),
        "map",
        &[ParamSpec::new("callback", "function")],
        "Option",
        "Maps an Option.Some payload with a callback.",
    ),
    MethodSpec::new(
        option_id(6),
        "and_then",
        &[ParamSpec::new("callback", "function")],
        "Option",
        "Chains an Option.Some payload through an Option-returning callback.",
    ),
    MethodSpec::new(
        option_id(7),
        "or_else",
        &[ParamSpec::new("callback", "function")],
        "Option",
        "Calls a fallback callback when the option is empty.",
    ),
    MethodSpec::new(
        option_id(8),
        "filter",
        &[ParamSpec::new("predicate", "function")],
        "Option",
        "Keeps an Option.Some payload accepted by a predicate.",
    ),
];

const fn option_id(offset: u32) -> HostMethodId {
    HostMethodId::new(0xff00_0b00 + offset)
}
