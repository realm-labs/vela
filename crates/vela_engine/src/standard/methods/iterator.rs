use vela_reflect::registry::MethodDesc;

use super::{MethodSpec, ParamSpec, descs};

pub(crate) fn iterator_method_descs() -> Vec<MethodDesc> {
    descs("Iterator", ITERATOR_METHODS, "iterator")
}

const ITERATOR_METHODS: &[MethodSpec] = &[
    MethodSpec::new(
        "next",
        &[],
        "Option",
        "Advances the iterator and returns the next value.",
    ),
    MethodSpec::new(
        "count",
        &[],
        "i64",
        "Consumes the iterator and returns the remaining item count.",
    ),
    MethodSpec::new(
        "any",
        &[ParamSpec::new("callback", "function")],
        "bool",
        "Consumes the iterator until a callback returns true.",
    ),
    MethodSpec::new(
        "all",
        &[ParamSpec::new("callback", "function")],
        "bool",
        "Consumes the iterator until a callback returns false.",
    ),
    MethodSpec::new(
        "find",
        &[ParamSpec::new("callback", "function")],
        "Option",
        "Consumes the iterator until a callback returns true.",
    ),
    MethodSpec::new(
        "map",
        &[ParamSpec::new("callback", "function")],
        "iterator",
        "Returns a lazy iterator that maps each value through a callback.",
    ),
    MethodSpec::new(
        "filter",
        &[ParamSpec::new("callback", "function")],
        "iterator",
        "Returns a lazy iterator that yields values accepted by a callback.",
    ),
    MethodSpec::new(
        "take",
        &[ParamSpec::new("count", "i64")],
        "iterator",
        "Returns a lazy iterator over at most count remaining values.",
    ),
    MethodSpec::new(
        "skip",
        &[ParamSpec::new("count", "i64")],
        "iterator",
        "Returns a lazy iterator after skipping count remaining values.",
    ),
    MethodSpec::new(
        "collect_array",
        &[],
        "array",
        "Consumes the iterator and collects remaining values into an array.",
    ),
];
