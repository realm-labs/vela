use vela_reflect::registry::MethodDesc;

use super::{MethodSpec, ParamSpec, descs};

pub(crate) fn option_method_descs() -> Vec<MethodDesc> {
    descs("Option", OPTION_METHODS, "option")
}

const OPTION_METHODS: &[MethodSpec] = &[
    MethodSpec::new(
        "is_some",
        &[],
        "bool",
        "Returns true when the option contains a value.",
    ),
    MethodSpec::new(
        "is_none",
        &[],
        "bool",
        "Returns true when the option is empty.",
    ),
    MethodSpec::new(
        "unwrap_or",
        &[ParamSpec::new("default", "any")],
        "any",
        "Returns the contained value or a default.",
    ),
    MethodSpec::new(
        "ok_or",
        &[ParamSpec::new("error", "any")],
        "Result",
        "Converts Option::None to Result::Err with an error value.",
    ),
    MethodSpec::new(
        "flatten",
        &[],
        "Option",
        "Flattens a nested dynamic Option value.",
    ),
    MethodSpec::new(
        "map",
        &[ParamSpec::new("callback", "function")],
        "Option",
        "Maps an Option::Some payload with a callback.",
    ),
    MethodSpec::new(
        "and_then",
        &[ParamSpec::new("callback", "function")],
        "Option",
        "Chains an Option::Some payload through an Option-returning callback.",
    ),
    MethodSpec::new(
        "or_else",
        &[ParamSpec::new("callback", "function")],
        "Option",
        "Calls a fallback callback when the option is empty.",
    ),
    MethodSpec::new(
        "filter",
        &[ParamSpec::new("predicate", "function")],
        "Option",
        "Keeps an Option::Some payload accepted by a predicate.",
    ),
];
