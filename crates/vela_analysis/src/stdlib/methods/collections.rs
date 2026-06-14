use super::*;

pub(super) fn array_method_fact(
    element: TypeFact,
    method: &str,
    lambda_return: Option<&TypeFact>,
) -> Option<StdlibMethodFact> {
    let receiver = TypeFact::array(element.clone());
    match method {
        "len" => Some(StdlibMethodFact::new(receiver, "len", TypeFact::I64)),
        "is_empty" => Some(StdlibMethodFact::new(receiver, "is_empty", TypeFact::BOOL)),
        "push" => Some(
            StdlibMethodFact::new(receiver, "push", TypeFact::NULL)
                .with_params(vec![element.clone()]),
        ),
        "pop" => Some(StdlibMethodFact::new(
            receiver,
            "pop",
            TypeFact::option(element.clone()),
        )),
        "insert" => Some(
            StdlibMethodFact::new(receiver, "insert", TypeFact::NULL)
                .with_params(vec![TypeFact::I64, element.clone()]),
        ),
        "extend" => Some(
            StdlibMethodFact::new(receiver, "extend", TypeFact::NULL)
                .with_params(vec![TypeFact::array(element.clone())]),
        ),
        "clear" => Some(StdlibMethodFact::new(receiver, "clear", TypeFact::NULL)),
        "first" => Some(StdlibMethodFact::new(
            receiver,
            "first",
            TypeFact::option(element.clone()),
        )),
        "last" => Some(StdlibMethodFact::new(
            receiver,
            "last",
            TypeFact::option(element.clone()),
        )),
        "remove_at" => Some(
            StdlibMethodFact::new(receiver, "remove_at", TypeFact::option(element.clone()))
                .with_params(vec![TypeFact::I64]),
        ),
        "join" => Some(
            StdlibMethodFact::new(receiver, "join", TypeFact::STRING)
                .with_params(vec![TypeFact::STRING]),
        ),
        "contains" => Some(
            StdlibMethodFact::new(receiver, "contains", TypeFact::BOOL)
                .with_params(vec![element.clone()]),
        ),
        "index_of" => Some(
            StdlibMethodFact::new(receiver, "index_of", TypeFact::option(TypeFact::I64))
                .with_params(vec![element.clone()]),
        ),
        "distinct" => Some(StdlibMethodFact::new(
            receiver,
            "distinct",
            TypeFact::array(element.clone()),
        )),
        "reverse" => Some(StdlibMethodFact::new(
            receiver,
            "reverse",
            TypeFact::array(element.clone()),
        )),
        "slice" => Some(
            StdlibMethodFact::new(receiver, "slice", TypeFact::array(element.clone()))
                .with_params(vec![TypeFact::I64, TypeFact::I64]),
        ),
        "map" => {
            let mapped = lambda_return.cloned().unwrap_or(TypeFact::Any);
            Some(
                StdlibMethodFact::new(receiver, "map", TypeFact::array(mapped.clone()))
                    .with_lambda(vec![element], mapped),
            )
        }
        "filter" => Some(
            StdlibMethodFact::new(receiver, "filter", TypeFact::array(element.clone()))
                .with_lambda(vec![element], TypeFact::BOOL),
        ),
        "find" => Some(
            StdlibMethodFact::new(receiver, "find", TypeFact::option(element.clone()))
                .with_lambda(vec![element], TypeFact::BOOL),
        ),
        "any" => Some(
            StdlibMethodFact::new(receiver, "any", TypeFact::BOOL)
                .with_lambda(vec![element], TypeFact::BOOL),
        ),
        "all" => Some(
            StdlibMethodFact::new(receiver, "all", TypeFact::BOOL)
                .with_lambda(vec![element], TypeFact::BOOL),
        ),
        "count" => Some(
            StdlibMethodFact::new(receiver, "count", TypeFact::I64)
                .with_lambda(vec![element], TypeFact::BOOL),
        ),
        "sum" => {
            let returns = lambda_return.cloned().unwrap_or(element.clone());
            Some(
                StdlibMethodFact::new(receiver, "sum", numeric_return(&returns))
                    .with_lambda(vec![element], returns),
            )
        }
        "group_by" => {
            let key = lambda_return.cloned().unwrap_or(TypeFact::Any);
            Some(
                StdlibMethodFact::new(
                    receiver,
                    "group_by",
                    TypeFact::map(key.clone(), TypeFact::array(element.clone())),
                )
                .with_lambda(vec![element], key),
            )
        }
        "sort" => Some(StdlibMethodFact::new(
            receiver,
            "sort",
            TypeFact::array(element.clone()),
        )),
        "min" => Some(StdlibMethodFact::new(
            receiver,
            "min",
            TypeFact::option(element.clone()),
        )),
        "max" => Some(StdlibMethodFact::new(
            receiver,
            "max",
            TypeFact::option(element.clone()),
        )),
        "sort_by" => Some(
            StdlibMethodFact::new(receiver, "sort_by", TypeFact::array(element.clone()))
                .with_lambda(vec![element], TypeFact::Any),
        ),
        "iter" => Some(StdlibMethodFact::new(
            receiver,
            "iter",
            TypeFact::iterator(element.clone()),
        )),
        "values" => Some(StdlibMethodFact::new(
            receiver,
            "values",
            TypeFact::iterator(element),
        )),
        _ => None,
    }
}

pub(super) fn map_method_fact(
    key: TypeFact,
    value: TypeFact,
    method: &str,
    lambda_return: Option<&TypeFact>,
    lambda_param_count: Option<usize>,
) -> Option<StdlibMethodFact> {
    let receiver = TypeFact::map(key.clone(), value.clone());
    match method {
        "len" => Some(StdlibMethodFact::new(receiver, "len", TypeFact::I64)),
        "is_empty" => Some(StdlibMethodFact::new(receiver, "is_empty", TypeFact::BOOL)),
        "has" => Some(
            StdlibMethodFact::new(receiver, "has", TypeFact::BOOL).with_params(vec![key.clone()]),
        ),
        "get" => Some(
            StdlibMethodFact::new(receiver, "get", TypeFact::option(value.clone()))
                .with_params(vec![key.clone()]),
        ),
        "get_or" => Some(
            StdlibMethodFact::new(receiver, "get_or", value.clone())
                .with_params(vec![key.clone(), value.clone()]),
        ),
        "set" => Some(
            StdlibMethodFact::new(receiver, "set", value.clone())
                .with_params(vec![key.clone(), value.clone()]),
        ),
        "remove" => Some(
            StdlibMethodFact::new(receiver, "remove", TypeFact::option(value.clone()))
                .with_params(vec![key.clone()]),
        ),
        "extend" => Some(
            StdlibMethodFact::new(receiver, "extend", TypeFact::NULL)
                .with_params(vec![TypeFact::map(key.clone(), value.clone())]),
        ),
        "clear" => Some(StdlibMethodFact::new(receiver, "clear", TypeFact::NULL)),
        "keys" => Some(StdlibMethodFact::new(
            receiver,
            "keys",
            TypeFact::iterator(key.clone()),
        )),
        "values" => Some(StdlibMethodFact::new(
            receiver,
            "values",
            TypeFact::iterator(value.clone()),
        )),
        "entries" => Some(StdlibMethodFact::new(
            receiver,
            "entries",
            TypeFact::iterator(TypeFact::record("MapEntry")),
        )),
        "merge" => Some(
            StdlibMethodFact::new(receiver, "merge", TypeFact::map(key.clone(), value.clone()))
                .with_params(vec![TypeFact::map(key.clone(), value.clone())]),
        ),
        "map_values" => {
            let mapped = lambda_return.cloned().unwrap_or(TypeFact::Any);
            let lambda_params = map_lambda_params(key.clone(), value, lambda_param_count);
            Some(
                StdlibMethodFact::new(
                    receiver,
                    "map_values",
                    TypeFact::map(key.clone(), mapped.clone()),
                )
                .with_lambda(lambda_params, mapped),
            )
        }
        "filter" => Some(
            StdlibMethodFact::new(
                receiver,
                "filter",
                TypeFact::map(key.clone(), value.clone()),
            )
            .with_lambda(
                map_lambda_params(key.clone(), value.clone(), lambda_param_count),
                TypeFact::BOOL,
            ),
        ),
        "find" => Some(
            StdlibMethodFact::new(
                receiver,
                "find",
                TypeFact::option(TypeFact::record("MapEntry")),
            )
            .with_lambda(
                map_lambda_params(key.clone(), value.clone(), lambda_param_count),
                TypeFact::BOOL,
            ),
        ),
        "any" => Some(
            StdlibMethodFact::new(receiver, "any", TypeFact::BOOL).with_lambda(
                map_lambda_params(key.clone(), value.clone(), lambda_param_count),
                TypeFact::BOOL,
            ),
        ),
        "all" => Some(
            StdlibMethodFact::new(receiver, "all", TypeFact::BOOL).with_lambda(
                map_lambda_params(key.clone(), value.clone(), lambda_param_count),
                TypeFact::BOOL,
            ),
        ),
        "count" => Some(
            StdlibMethodFact::new(receiver, "count", TypeFact::I64).with_lambda(
                map_lambda_params(key, value, lambda_param_count),
                TypeFact::BOOL,
            ),
        ),
        "iter" => Some(StdlibMethodFact::new(
            receiver,
            "iter",
            TypeFact::iterator(value),
        )),
        _ => None,
    }
}

fn map_lambda_params(
    key: TypeFact,
    value: TypeFact,
    lambda_param_count: Option<usize>,
) -> Vec<TypeFact> {
    match lambda_param_count {
        Some(0) => Vec::new(),
        Some(1) => vec![value],
        _ => vec![key, value],
    }
}

pub(super) fn set_method_fact(
    element: TypeFact,
    method: &str,
    lambda_return: Option<&TypeFact>,
) -> Option<StdlibMethodFact> {
    let receiver = TypeFact::set(element.clone());
    match method {
        "len" => Some(StdlibMethodFact::new(receiver, "len", TypeFact::I64)),
        "is_empty" => Some(StdlibMethodFact::new(receiver, "is_empty", TypeFact::BOOL)),
        "has" => Some(
            StdlibMethodFact::new(receiver, "has", TypeFact::BOOL)
                .with_params(vec![element.clone()]),
        ),
        "add" => Some(
            StdlibMethodFact::new(receiver, "add", TypeFact::BOOL)
                .with_params(vec![element.clone()]),
        ),
        "remove" => Some(
            StdlibMethodFact::new(receiver, "remove", TypeFact::BOOL)
                .with_params(vec![element.clone()]),
        ),
        "extend" => Some(
            StdlibMethodFact::new(receiver, "extend", TypeFact::NULL)
                .with_params(vec![TypeFact::set(element.clone())]),
        ),
        "clear" => Some(StdlibMethodFact::new(receiver, "clear", TypeFact::NULL)),
        "values" => Some(StdlibMethodFact::new(
            receiver,
            "values",
            TypeFact::iterator(element.clone()),
        )),
        "map" => {
            let mapped = lambda_return.cloned().unwrap_or(TypeFact::Any);
            Some(
                StdlibMethodFact::new(receiver, "map", TypeFact::set(mapped.clone()))
                    .with_lambda(vec![element], mapped),
            )
        }
        "filter" => Some(
            StdlibMethodFact::new(receiver, "filter", TypeFact::set(element.clone()))
                .with_lambda(vec![element], TypeFact::BOOL),
        ),
        "find" => Some(
            StdlibMethodFact::new(receiver, "find", TypeFact::option(element.clone()))
                .with_lambda(vec![element], TypeFact::BOOL),
        ),
        "any" => Some(
            StdlibMethodFact::new(receiver, "any", TypeFact::BOOL)
                .with_lambda(vec![element], TypeFact::BOOL),
        ),
        "all" => Some(
            StdlibMethodFact::new(receiver, "all", TypeFact::BOOL)
                .with_lambda(vec![element], TypeFact::BOOL),
        ),
        "count" => Some(
            StdlibMethodFact::new(receiver, "count", TypeFact::I64)
                .with_lambda(vec![element], TypeFact::BOOL),
        ),
        "union" | "intersection" | "difference" | "symmetric_difference" => Some(
            StdlibMethodFact::new(
                receiver,
                match method {
                    "union" => "union",
                    "intersection" => "intersection",
                    "difference" => "difference",
                    _ => "symmetric_difference",
                },
                TypeFact::set(element.clone()),
            )
            .with_params(vec![TypeFact::set(element)]),
        ),
        "is_subset" | "is_superset" | "is_disjoint" => Some(
            StdlibMethodFact::new(
                receiver,
                match method {
                    "is_subset" => "is_subset",
                    "is_superset" => "is_superset",
                    _ => "is_disjoint",
                },
                TypeFact::BOOL,
            )
            .with_params(vec![TypeFact::set(element)]),
        ),
        "iter" => Some(StdlibMethodFact::new(
            receiver,
            "iter",
            TypeFact::iterator(element),
        )),
        _ => None,
    }
}

pub(super) fn string_method_fact(method: &str) -> Option<StdlibMethodFact> {
    let receiver = TypeFact::STRING;
    match method {
        "len" => Some(StdlibMethodFact::new(receiver, "len", TypeFact::I64)),
        "is_empty" => Some(StdlibMethodFact::new(receiver, "is_empty", TypeFact::BOOL)),
        "contains" => Some(
            StdlibMethodFact::new(receiver, "contains", TypeFact::BOOL)
                .with_params(vec![TypeFact::STRING]),
        ),
        "find" => Some(
            StdlibMethodFact::new(receiver, "find", TypeFact::option(TypeFact::I64))
                .with_params(vec![TypeFact::STRING]),
        ),
        "starts_with" => Some(
            StdlibMethodFact::new(receiver, "starts_with", TypeFact::BOOL)
                .with_params(vec![TypeFact::STRING]),
        ),
        "ends_with" => Some(
            StdlibMethodFact::new(receiver, "ends_with", TypeFact::BOOL)
                .with_params(vec![TypeFact::STRING]),
        ),
        "strip_prefix" => Some(
            StdlibMethodFact::new(receiver, "strip_prefix", TypeFact::option(TypeFact::STRING))
                .with_params(vec![TypeFact::STRING]),
        ),
        "strip_suffix" => Some(
            StdlibMethodFact::new(receiver, "strip_suffix", TypeFact::option(TypeFact::STRING))
                .with_params(vec![TypeFact::STRING]),
        ),
        "to_upper" => Some(StdlibMethodFact::new(
            receiver,
            "to_upper",
            TypeFact::STRING,
        )),
        "to_lower" => Some(StdlibMethodFact::new(
            receiver,
            "to_lower",
            TypeFact::STRING,
        )),
        "trim" | "trim_start" | "trim_end" => Some(StdlibMethodFact::new(
            receiver,
            match method {
                "trim_start" => "trim_start",
                "trim_end" => "trim_end",
                _ => "trim",
            },
            TypeFact::STRING,
        )),
        "replace" => Some(
            StdlibMethodFact::new(receiver, "replace", TypeFact::STRING)
                .with_params(vec![TypeFact::STRING, TypeFact::STRING]),
        ),
        "repeat" => Some(
            StdlibMethodFact::new(receiver, "repeat", TypeFact::STRING)
                .with_params(vec![TypeFact::I64]),
        ),
        "slice" => Some(
            StdlibMethodFact::new(receiver, "slice", TypeFact::STRING)
                .with_params(vec![TypeFact::I64, TypeFact::I64]),
        ),
        "split" => Some(
            StdlibMethodFact::new(receiver, "split", TypeFact::array(TypeFact::STRING))
                .with_params(vec![TypeFact::STRING]),
        ),
        "split_once" => Some(
            StdlibMethodFact::new(
                receiver,
                "split_once",
                TypeFact::option(TypeFact::array(TypeFact::STRING)),
            )
            .with_params(vec![TypeFact::STRING]),
        ),
        "split_lines" => Some(StdlibMethodFact::new(
            receiver,
            "split_lines",
            TypeFact::array(TypeFact::STRING),
        )),
        "split_whitespace" => Some(StdlibMethodFact::new(
            receiver,
            "split_whitespace",
            TypeFact::array(TypeFact::STRING),
        )),
        "parse_i8" => Some(parse_fact(receiver, "parse_i8", TypeFact::I8)),
        "parse_i16" => Some(parse_fact(receiver, "parse_i16", TypeFact::I16)),
        "parse_i32" => Some(parse_fact(receiver, "parse_i32", TypeFact::I32)),
        "parse_i64" => Some(parse_fact(receiver, "parse_i64", TypeFact::I64)),
        "parse_u8" => Some(parse_fact(receiver, "parse_u8", TypeFact::U8)),
        "parse_u16" => Some(parse_fact(receiver, "parse_u16", TypeFact::U16)),
        "parse_u32" => Some(parse_fact(receiver, "parse_u32", TypeFact::U32)),
        "parse_u64" => Some(parse_fact(receiver, "parse_u64", TypeFact::U64)),
        "parse_f32" => Some(parse_fact(receiver, "parse_f32", TypeFact::F32)),
        "parse_f64" => Some(parse_fact(receiver, "parse_f64", TypeFact::F64)),
        "parse_bool" => Some(parse_fact(receiver, "parse_bool", TypeFact::BOOL)),
        "parse_char" => Some(parse_fact(receiver, "parse_char", TypeFact::CHAR)),
        "chars" => Some(StdlibMethodFact::new(
            receiver,
            "chars",
            TypeFact::iterator(TypeFact::CHAR),
        )),
        "bytes" => Some(StdlibMethodFact::new(
            receiver,
            "bytes",
            TypeFact::iterator(TypeFact::U8),
        )),
        _ => None,
    }
}

fn parse_fact(receiver: TypeFact, name: &'static str, returns: TypeFact) -> StdlibMethodFact {
    StdlibMethodFact::new(receiver, name, TypeFact::option(returns))
}

pub(super) fn bytes_method_fact(method: &str) -> Option<StdlibMethodFact> {
    let receiver = TypeFact::BYTES;
    match method {
        "len" => Some(StdlibMethodFact::new(receiver, "len", TypeFact::I64)),
        "is_empty" => Some(StdlibMethodFact::new(receiver, "is_empty", TypeFact::BOOL)),
        "slice" => Some(
            StdlibMethodFact::new(receiver, "slice", TypeFact::BYTES)
                .with_params(vec![TypeFact::I64, TypeFact::I64]),
        ),
        "get" => Some(
            StdlibMethodFact::new(receiver, "get", TypeFact::U8).with_params(vec![TypeFact::I64]),
        ),
        "read_u32_le" => Some(
            StdlibMethodFact::new(receiver, "read_u32_le", TypeFact::U32)
                .with_params(vec![TypeFact::I64]),
        ),
        "read_u32_be" => Some(
            StdlibMethodFact::new(receiver, "read_u32_be", TypeFact::U32)
                .with_params(vec![TypeFact::I64]),
        ),
        "to_hex" => Some(StdlibMethodFact::new(receiver, "to_hex", TypeFact::STRING)),
        "iter" => Some(StdlibMethodFact::new(
            receiver,
            "iter",
            TypeFact::iterator(TypeFact::U8),
        )),
        "values" => Some(StdlibMethodFact::new(
            receiver,
            "values",
            TypeFact::iterator(TypeFact::U8),
        )),
        _ => None,
    }
}

pub(super) fn char_method_fact(method: &str) -> Option<StdlibMethodFact> {
    let receiver = TypeFact::CHAR;
    match method {
        "to_string" => Some(StdlibMethodFact::new(
            receiver,
            "to_string",
            TypeFact::STRING,
        )),
        "is_whitespace" | "is_ascii" | "is_ascii_digit" => Some(StdlibMethodFact::new(
            receiver,
            match method {
                "is_whitespace" => "is_whitespace",
                "is_ascii_digit" => "is_ascii_digit",
                _ => "is_ascii",
            },
            TypeFact::BOOL,
        )),
        _ => None,
    }
}

pub(super) fn range_method_fact(method: &str) -> Option<StdlibMethodFact> {
    let receiver = TypeFact::Range;
    match method {
        "len" => Some(StdlibMethodFact::new(receiver, "len", TypeFact::I64)),
        "is_empty" => Some(StdlibMethodFact::new(receiver, "is_empty", TypeFact::BOOL)),
        "iter" => Some(StdlibMethodFact::new(
            receiver,
            "iter",
            TypeFact::iterator(TypeFact::I64),
        )),
        _ => None,
    }
}

pub(super) fn iterator_method_fact(
    item: TypeFact,
    method: &str,
    lambda_return: Option<&TypeFact>,
) -> Option<StdlibMethodFact> {
    let receiver = TypeFact::iterator(item.clone());
    match method {
        "next" => Some(StdlibMethodFact::new(
            receiver,
            "next",
            TypeFact::option(item.clone()),
        )),
        "count" => Some(StdlibMethodFact::new(receiver, "count", TypeFact::I64)),
        "any" => Some(
            StdlibMethodFact::new(receiver, "any", TypeFact::BOOL)
                .with_lambda(vec![item.clone()], TypeFact::BOOL),
        ),
        "all" => Some(
            StdlibMethodFact::new(receiver, "all", TypeFact::BOOL)
                .with_lambda(vec![item.clone()], TypeFact::BOOL),
        ),
        "find" => Some(
            StdlibMethodFact::new(receiver, "find", TypeFact::option(item.clone()))
                .with_lambda(vec![item.clone()], TypeFact::BOOL),
        ),
        "map" => {
            let mapped = lambda_return.cloned().unwrap_or(TypeFact::Any);
            Some(
                StdlibMethodFact::new(receiver, "map", TypeFact::iterator(mapped.clone()))
                    .with_lambda(vec![item], mapped),
            )
        }
        "filter" => Some(
            StdlibMethodFact::new(receiver, "filter", TypeFact::iterator(item.clone()))
                .with_lambda(vec![item], TypeFact::BOOL),
        ),
        "take" => Some(
            StdlibMethodFact::new(receiver, "take", TypeFact::iterator(item.clone()))
                .with_params(vec![TypeFact::I64]),
        ),
        "skip" => Some(
            StdlibMethodFact::new(receiver, "skip", TypeFact::iterator(item.clone()))
                .with_params(vec![TypeFact::I64]),
        ),
        "collect_array" => Some(StdlibMethodFact::new(
            receiver,
            "collect_array",
            TypeFact::array(item),
        )),
        "collect_set" => Some(StdlibMethodFact::new(
            receiver,
            "collect_set",
            TypeFact::set(item),
        )),
        "collect_map" => Some(StdlibMethodFact::new(
            receiver,
            "collect_map",
            TypeFact::map(TypeFact::STRING, TypeFact::Any),
        )),
        _ => None,
    }
}
