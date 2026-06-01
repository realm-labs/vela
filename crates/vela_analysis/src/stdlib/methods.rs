use crate::stdlib::StdlibMethodFact;
use crate::type_fact::TypeFact;

mod collections;
mod option_result;

use collections::{
    array_method_fact, map_method_fact, range_method_fact, set_method_fact, string_method_fact,
};
use option_result::{OptionShape, ResultShape, option_method_fact, result_method_fact};

const ARRAY_METHOD_NAMES: &[&str] = &[
    "len",
    "is_empty",
    "push",
    "pop",
    "insert",
    "extend",
    "clear",
    "first",
    "last",
    "remove_at",
    "join",
    "contains",
    "index_of",
    "distinct",
    "reverse",
    "slice",
    "map",
    "filter",
    "find",
    "any",
    "all",
    "count",
    "sum",
    "group_by",
    "sort",
    "min",
    "max",
    "sort_by",
];
const MAP_METHOD_NAMES: &[&str] = &[
    "len",
    "is_empty",
    "has",
    "get",
    "get_or",
    "set",
    "remove",
    "extend",
    "clear",
    "keys",
    "values",
    "entries",
    "merge",
    "map_values",
    "filter",
    "find",
    "any",
    "all",
    "count",
];
const SET_METHOD_NAMES: &[&str] = &[
    "len",
    "is_empty",
    "has",
    "add",
    "remove",
    "extend",
    "clear",
    "values",
    "map",
    "filter",
    "find",
    "any",
    "all",
    "count",
    "union",
    "intersection",
    "difference",
    "symmetric_difference",
    "is_subset",
    "is_superset",
    "is_disjoint",
];
const STRING_METHOD_NAMES: &[&str] = &[
    "len",
    "is_empty",
    "contains",
    "find",
    "starts_with",
    "ends_with",
    "strip_prefix",
    "strip_suffix",
    "to_upper",
    "to_lower",
    "trim",
    "trim_start",
    "trim_end",
    "replace",
    "repeat",
    "slice",
    "split",
    "split_once",
    "split_lines",
    "split_whitespace",
    "char_at",
    "parse_int",
    "parse_float",
    "parse_bool",
];
const RANGE_METHOD_NAMES: &[&str] = &["len", "is_empty"];
const OPTION_METHOD_NAMES: &[&str] = &[
    "is_some",
    "is_none",
    "unwrap_or",
    "ok_or",
    "flatten",
    "map",
    "and_then",
    "or_else",
    "filter",
];
const RESULT_METHOD_NAMES: &[&str] = &[
    "is_ok",
    "is_err",
    "unwrap_or",
    "to_option",
    "to_error_option",
    "flatten",
    "map",
    "map_err",
    "and_then",
    "or_else",
];

pub(super) fn method_fact(
    receiver: &TypeFact,
    method: &str,
    lambda_return: Option<&TypeFact>,
) -> Option<StdlibMethodFact> {
    match receiver {
        TypeFact::Array { element } => {
            array_method_fact((**element).clone(), method, lambda_return)
        }
        TypeFact::Map { key, value } => {
            map_method_fact((**key).clone(), (**value).clone(), method, lambda_return)
        }
        TypeFact::Set { element } => set_method_fact((**element).clone(), method, lambda_return),
        TypeFact::String => string_method_fact(method),
        TypeFact::Range => range_method_fact(method),
        TypeFact::Option { some } => {
            option_method_fact((**some).clone(), OptionShape::Maybe, method, lambda_return)
        }
        TypeFact::OptionSome { some } => {
            option_method_fact((**some).clone(), OptionShape::Some, method, lambda_return)
        }
        TypeFact::OptionNone => {
            option_method_fact(TypeFact::Never, OptionShape::None, method, lambda_return)
        }
        TypeFact::Result { ok, err } => result_method_fact(
            (**ok).clone(),
            (**err).clone(),
            ResultShape::Maybe,
            method,
            lambda_return,
        ),
        TypeFact::ResultOk { ok } => result_method_fact(
            (**ok).clone(),
            TypeFact::Any,
            ResultShape::Ok,
            method,
            lambda_return,
        ),
        TypeFact::ResultErr { err } => result_method_fact(
            TypeFact::Never,
            (**err).clone(),
            ResultShape::Err,
            method,
            lambda_return,
        ),
        _ => None,
    }
}

pub(super) fn method_facts(
    receiver: &TypeFact,
    lambda_return: Option<&TypeFact>,
) -> Vec<StdlibMethodFact> {
    method_names(receiver)
        .iter()
        .filter_map(|method| method_fact(receiver, method, lambda_return))
        .collect()
}

fn method_names(receiver: &TypeFact) -> &'static [&'static str] {
    match receiver {
        TypeFact::Array { .. } => ARRAY_METHOD_NAMES,
        TypeFact::Map { .. } => MAP_METHOD_NAMES,
        TypeFact::Set { .. } => SET_METHOD_NAMES,
        TypeFact::String => STRING_METHOD_NAMES,
        TypeFact::Range => RANGE_METHOD_NAMES,
        TypeFact::Option { .. } | TypeFact::OptionSome { .. } | TypeFact::OptionNone => {
            OPTION_METHOD_NAMES
        }
        TypeFact::Result { .. } | TypeFact::ResultOk { .. } | TypeFact::ResultErr { .. } => {
            RESULT_METHOD_NAMES
        }
        _ => &[],
    }
}

fn value_or_fallback(value: TypeFact, fallback: TypeFact) -> TypeFact {
    if value == fallback {
        value
    } else {
        TypeFact::union([value, fallback])
    }
}

fn numeric_return(value: &TypeFact) -> TypeFact {
    match value {
        TypeFact::Float => TypeFact::Float,
        TypeFact::Int => TypeFact::Int,
        _ => TypeFact::Union(vec![TypeFact::Int, TypeFact::Float]),
    }
}
