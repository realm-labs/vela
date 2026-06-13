use vela_reflect::registry::MethodDesc;

use super::{MethodSpec, descs};

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
        "collect_array",
        &[],
        "array",
        "Consumes the iterator and collects remaining values into an array.",
    ),
];
