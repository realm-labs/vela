use crate::{TypeFact, stdlib::StdlibMethodFact};

const ARRAY_METHOD_NAMES: &[&str] = &[
    "len", "is_empty", "push", "pop", "first", "last", "join", "contains", "distinct", "reverse",
    "slice", "map", "filter", "find", "any", "all", "count", "sum", "group_by", "sort_by",
];
const MAP_METHOD_NAMES: &[&str] = &[
    "len",
    "is_empty",
    "has",
    "get",
    "get_or",
    "set",
    "remove",
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
    "values",
    "union",
    "intersection",
    "difference",
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
    "parse_int",
    "parse_float",
    "parse_bool",
];
const OPTION_METHOD_NAMES: &[&str] = &["map", "and_then", "or_else"];
const RESULT_METHOD_NAMES: &[&str] = &["map", "map_err", "and_then", "or_else"];

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
        TypeFact::Set { element } => set_method_fact((**element).clone(), method),
        TypeFact::String => string_method_fact(method),
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
        TypeFact::Option { .. } | TypeFact::OptionSome { .. } | TypeFact::OptionNone => {
            OPTION_METHOD_NAMES
        }
        TypeFact::Result { .. } | TypeFact::ResultOk { .. } | TypeFact::ResultErr { .. } => {
            RESULT_METHOD_NAMES
        }
        _ => &[],
    }
}

fn array_method_fact(
    element: TypeFact,
    method: &str,
    lambda_return: Option<&TypeFact>,
) -> Option<StdlibMethodFact> {
    let receiver = TypeFact::array(element.clone());
    match method {
        "len" => Some(StdlibMethodFact::new(receiver, "len", TypeFact::Int)),
        "is_empty" => Some(StdlibMethodFact::new(receiver, "is_empty", TypeFact::Bool)),
        "push" => Some(
            StdlibMethodFact::new(receiver, "push", TypeFact::Null)
                .with_params(vec![element.clone()]),
        ),
        "pop" => Some(StdlibMethodFact::new(
            receiver,
            "pop",
            TypeFact::option(element.clone()),
        )),
        "first" => Some(StdlibMethodFact::new(
            receiver,
            "first",
            TypeFact::option(element.clone()),
        )),
        "last" => Some(StdlibMethodFact::new(
            receiver,
            "last",
            TypeFact::option(element),
        )),
        "join" => Some(
            StdlibMethodFact::new(receiver, "join", TypeFact::String)
                .with_params(vec![TypeFact::String]),
        ),
        "contains" => Some(
            StdlibMethodFact::new(receiver, "contains", TypeFact::Bool)
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
                .with_params(vec![TypeFact::Int, TypeFact::Int]),
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
                .with_lambda(vec![element], TypeFact::Bool),
        ),
        "find" => Some(
            StdlibMethodFact::new(receiver, "find", TypeFact::option(element.clone()))
                .with_lambda(vec![element], TypeFact::Bool),
        ),
        "any" => Some(
            StdlibMethodFact::new(receiver, "any", TypeFact::Bool)
                .with_lambda(vec![element], TypeFact::Bool),
        ),
        "all" => Some(
            StdlibMethodFact::new(receiver, "all", TypeFact::Bool)
                .with_lambda(vec![element], TypeFact::Bool),
        ),
        "count" => Some(
            StdlibMethodFact::new(receiver, "count", TypeFact::Int)
                .with_lambda(vec![element], TypeFact::Bool),
        ),
        "sum" => {
            let returns = lambda_return.cloned().unwrap_or(element.clone());
            Some(
                StdlibMethodFact::new(receiver, "sum", numeric_return(&returns))
                    .with_lambda(vec![element], returns),
            )
        }
        "group_by" => Some(
            StdlibMethodFact::new(
                receiver,
                "group_by",
                TypeFact::map(TypeFact::String, TypeFact::array(element.clone())),
            )
            .with_lambda(vec![element], TypeFact::String),
        ),
        "sort_by" => Some(
            StdlibMethodFact::new(receiver, "sort_by", TypeFact::array(element.clone()))
                .with_lambda(vec![element], TypeFact::Any),
        ),
        _ => None,
    }
}

fn map_method_fact(
    key: TypeFact,
    value: TypeFact,
    method: &str,
    lambda_return: Option<&TypeFact>,
) -> Option<StdlibMethodFact> {
    let receiver = TypeFact::map(key.clone(), value.clone());
    match method {
        "len" => Some(StdlibMethodFact::new(receiver, "len", TypeFact::Int)),
        "is_empty" => Some(StdlibMethodFact::new(receiver, "is_empty", TypeFact::Bool)),
        "has" => Some(
            StdlibMethodFact::new(receiver, "has", TypeFact::Bool).with_params(vec![key.clone()]),
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
        "keys" => Some(StdlibMethodFact::new(
            receiver,
            "keys",
            TypeFact::array(key.clone()),
        )),
        "values" => Some(StdlibMethodFact::new(
            receiver,
            "values",
            TypeFact::array(value.clone()),
        )),
        "entries" => Some(StdlibMethodFact::new(
            receiver,
            "entries",
            TypeFact::array(TypeFact::record("MapEntry")),
        )),
        "merge" => Some(
            StdlibMethodFact::new(receiver, "merge", TypeFact::map(key.clone(), value.clone()))
                .with_params(vec![TypeFact::map(key.clone(), value.clone())]),
        ),
        "map_values" => {
            let mapped = lambda_return.cloned().unwrap_or(TypeFact::Any);
            Some(
                StdlibMethodFact::new(
                    receiver,
                    "map_values",
                    TypeFact::map(key.clone(), mapped.clone()),
                )
                .with_lambda(vec![key, value], mapped),
            )
        }
        "filter" => Some(
            StdlibMethodFact::new(
                receiver,
                "filter",
                TypeFact::map(key.clone(), value.clone()),
            )
            .with_lambda(vec![key, value], TypeFact::Bool),
        ),
        "find" => Some(
            StdlibMethodFact::new(
                receiver,
                "find",
                TypeFact::option(TypeFact::record("MapEntry")),
            )
            .with_lambda(vec![key.clone(), value.clone()], TypeFact::Bool),
        ),
        "any" => Some(
            StdlibMethodFact::new(receiver, "any", TypeFact::Bool)
                .with_lambda(vec![key.clone(), value.clone()], TypeFact::Bool),
        ),
        "all" => Some(
            StdlibMethodFact::new(receiver, "all", TypeFact::Bool)
                .with_lambda(vec![key.clone(), value.clone()], TypeFact::Bool),
        ),
        "count" => Some(
            StdlibMethodFact::new(receiver, "count", TypeFact::Int)
                .with_lambda(vec![key, value], TypeFact::Bool),
        ),
        _ => None,
    }
}

fn set_method_fact(element: TypeFact, method: &str) -> Option<StdlibMethodFact> {
    let receiver = TypeFact::set(element.clone());
    match method {
        "len" => Some(StdlibMethodFact::new(receiver, "len", TypeFact::Int)),
        "is_empty" => Some(StdlibMethodFact::new(receiver, "is_empty", TypeFact::Bool)),
        "has" => Some(
            StdlibMethodFact::new(receiver, "has", TypeFact::Bool)
                .with_params(vec![element.clone()]),
        ),
        "add" => Some(
            StdlibMethodFact::new(receiver, "add", TypeFact::Bool)
                .with_params(vec![element.clone()]),
        ),
        "remove" => Some(
            StdlibMethodFact::new(receiver, "remove", TypeFact::Bool)
                .with_params(vec![element.clone()]),
        ),
        "values" => Some(StdlibMethodFact::new(
            receiver,
            "values",
            TypeFact::array(element.clone()),
        )),
        "union" | "intersection" | "difference" => Some(
            StdlibMethodFact::new(
                receiver,
                match method {
                    "union" => "union",
                    "intersection" => "intersection",
                    _ => "difference",
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
                TypeFact::Bool,
            )
            .with_params(vec![TypeFact::set(element)]),
        ),
        _ => None,
    }
}

fn string_method_fact(method: &str) -> Option<StdlibMethodFact> {
    let receiver = TypeFact::String;
    match method {
        "len" => Some(StdlibMethodFact::new(receiver, "len", TypeFact::Int)),
        "is_empty" => Some(StdlibMethodFact::new(receiver, "is_empty", TypeFact::Bool)),
        "contains" => Some(
            StdlibMethodFact::new(receiver, "contains", TypeFact::Bool)
                .with_params(vec![TypeFact::String]),
        ),
        "find" => Some(
            StdlibMethodFact::new(receiver, "find", TypeFact::option(TypeFact::Int))
                .with_params(vec![TypeFact::String]),
        ),
        "starts_with" => Some(
            StdlibMethodFact::new(receiver, "starts_with", TypeFact::Bool)
                .with_params(vec![TypeFact::String]),
        ),
        "ends_with" => Some(
            StdlibMethodFact::new(receiver, "ends_with", TypeFact::Bool)
                .with_params(vec![TypeFact::String]),
        ),
        "strip_prefix" => Some(
            StdlibMethodFact::new(receiver, "strip_prefix", TypeFact::option(TypeFact::String))
                .with_params(vec![TypeFact::String]),
        ),
        "strip_suffix" => Some(
            StdlibMethodFact::new(receiver, "strip_suffix", TypeFact::option(TypeFact::String))
                .with_params(vec![TypeFact::String]),
        ),
        "to_upper" => Some(StdlibMethodFact::new(
            receiver,
            "to_upper",
            TypeFact::String,
        )),
        "to_lower" => Some(StdlibMethodFact::new(
            receiver,
            "to_lower",
            TypeFact::String,
        )),
        "trim" | "trim_start" | "trim_end" => Some(StdlibMethodFact::new(
            receiver,
            match method {
                "trim_start" => "trim_start",
                "trim_end" => "trim_end",
                _ => "trim",
            },
            TypeFact::String,
        )),
        "replace" => Some(
            StdlibMethodFact::new(receiver, "replace", TypeFact::String)
                .with_params(vec![TypeFact::String, TypeFact::String]),
        ),
        "repeat" => Some(
            StdlibMethodFact::new(receiver, "repeat", TypeFact::String)
                .with_params(vec![TypeFact::Int]),
        ),
        "slice" => Some(
            StdlibMethodFact::new(receiver, "slice", TypeFact::String)
                .with_params(vec![TypeFact::Int, TypeFact::Int]),
        ),
        "split" => Some(
            StdlibMethodFact::new(receiver, "split", TypeFact::array(TypeFact::String))
                .with_params(vec![TypeFact::String]),
        ),
        "parse_int" => Some(StdlibMethodFact::new(
            receiver,
            "parse_int",
            TypeFact::option(TypeFact::Int),
        )),
        "parse_float" => Some(StdlibMethodFact::new(
            receiver,
            "parse_float",
            TypeFact::option(TypeFact::Float),
        )),
        "parse_bool" => Some(StdlibMethodFact::new(
            receiver,
            "parse_bool",
            TypeFact::option(TypeFact::Bool),
        )),
        _ => None,
    }
}

#[derive(Clone, Copy)]
enum OptionShape {
    Maybe,
    Some,
    None,
}

fn option_method_fact(
    some: TypeFact,
    shape: OptionShape,
    method: &str,
    lambda_return: Option<&TypeFact>,
) -> Option<StdlibMethodFact> {
    let receiver = match shape {
        OptionShape::Maybe => TypeFact::option(some.clone()),
        OptionShape::Some => TypeFact::option_some(some.clone()),
        OptionShape::None => TypeFact::option_none(),
    };
    match method {
        "map" => {
            let mapped = lambda_return.cloned().unwrap_or(TypeFact::Any);
            let returns = match shape {
                OptionShape::Maybe => TypeFact::option(mapped.clone()),
                OptionShape::Some => TypeFact::option_some(mapped.clone()),
                OptionShape::None => TypeFact::option_none(),
            };
            Some(StdlibMethodFact::new(receiver, "map", returns).with_lambda(vec![some], mapped))
        }
        "and_then" => {
            let chained = option_chain_lambda_return(lambda_return);
            let returns = option_chain_return(shape, &chained);
            Some(
                StdlibMethodFact::new(receiver, "and_then", returns)
                    .with_lambda(vec![some], chained),
            )
        }
        "or_else" => {
            let fallback = option_chain_lambda_return(lambda_return);
            let returns = option_or_else_return(some.clone(), shape, &fallback);
            Some(StdlibMethodFact::new(receiver, "or_else", returns).with_lambda(vec![], fallback))
        }
        _ => None,
    }
}

#[derive(Clone, Copy)]
enum ResultShape {
    Maybe,
    Ok,
    Err,
}

fn result_method_fact(
    ok: TypeFact,
    err: TypeFact,
    shape: ResultShape,
    method: &str,
    lambda_return: Option<&TypeFact>,
) -> Option<StdlibMethodFact> {
    let mapped = lambda_return.cloned().unwrap_or(TypeFact::Any);
    let receiver = match shape {
        ResultShape::Maybe => TypeFact::result(ok.clone(), err.clone()),
        ResultShape::Ok => TypeFact::result_ok(ok.clone()),
        ResultShape::Err => TypeFact::result_err(err.clone()),
    };
    match method {
        "map" => {
            let returns = match shape {
                ResultShape::Maybe => TypeFact::result(mapped.clone(), err),
                ResultShape::Ok => TypeFact::result_ok(mapped.clone()),
                ResultShape::Err => TypeFact::result_err(err),
            };
            Some(StdlibMethodFact::new(receiver, "map", returns).with_lambda(vec![ok], mapped))
        }
        "map_err" => {
            let returns = match shape {
                ResultShape::Maybe => TypeFact::result(ok, mapped.clone()),
                ResultShape::Ok => TypeFact::result_ok(ok),
                ResultShape::Err => TypeFact::result_err(mapped.clone()),
            };
            Some(StdlibMethodFact::new(receiver, "map_err", returns).with_lambda(vec![err], mapped))
        }
        "and_then" => {
            let chained = result_chain_lambda_return(lambda_return);
            let returns = result_chain_return(err.clone(), shape, lambda_return);
            Some(
                StdlibMethodFact::new(receiver, "and_then", returns).with_lambda(vec![ok], chained),
            )
        }
        "or_else" => {
            let fallback = result_chain_lambda_return(lambda_return);
            let returns = result_or_else_return(ok.clone(), shape, lambda_return);
            Some(
                StdlibMethodFact::new(receiver, "or_else", returns)
                    .with_lambda(vec![err], fallback),
            )
        }
        _ => None,
    }
}

fn option_chain_lambda_return(lambda_return: Option<&TypeFact>) -> TypeFact {
    lambda_return
        .and_then(option_like_fact)
        .unwrap_or_else(|| TypeFact::option(TypeFact::Any))
}

fn option_chain_return(shape: OptionShape, chained: &TypeFact) -> TypeFact {
    match shape {
        OptionShape::Some => chained.clone(),
        OptionShape::None => TypeFact::option_none(),
        OptionShape::Maybe => option_maybe_return(chained),
    }
}

fn option_maybe_return(chained: &TypeFact) -> TypeFact {
    match chained {
        TypeFact::Option { some } | TypeFact::OptionSome { some } => {
            TypeFact::option((**some).clone())
        }
        TypeFact::OptionNone => TypeFact::option_none(),
        _ => TypeFact::option(TypeFact::Any),
    }
}

fn option_or_else_return(some: TypeFact, shape: OptionShape, fallback: &TypeFact) -> TypeFact {
    match shape {
        OptionShape::Some => TypeFact::option_some(some),
        OptionShape::None => fallback.clone(),
        OptionShape::Maybe => option_or_else_maybe_return(some, fallback),
    }
}

fn option_or_else_maybe_return(some: TypeFact, fallback: &TypeFact) -> TypeFact {
    match fallback {
        TypeFact::Option {
            some: fallback_some,
        }
        | TypeFact::OptionSome {
            some: fallback_some,
        } => TypeFact::option(TypeFact::union([some, (**fallback_some).clone()])),
        TypeFact::OptionNone => TypeFact::option(some),
        _ => TypeFact::option(TypeFact::Any),
    }
}

fn option_like_fact(fact: &TypeFact) -> Option<TypeFact> {
    match fact {
        TypeFact::Option { .. } | TypeFact::OptionSome { .. } | TypeFact::OptionNone => {
            Some(fact.clone())
        }
        _ => None,
    }
}

fn result_chain_lambda_return(lambda_return: Option<&TypeFact>) -> TypeFact {
    lambda_return
        .and_then(result_like_fact)
        .unwrap_or_else(|| TypeFact::result(TypeFact::Any, TypeFact::Any))
}

fn result_chain_return(
    passthrough_err: TypeFact,
    shape: ResultShape,
    lambda_return: Option<&TypeFact>,
) -> TypeFact {
    match shape {
        ResultShape::Ok => result_chain_lambda_return(lambda_return),
        ResultShape::Err => TypeFact::result_err(passthrough_err),
        ResultShape::Maybe => lambda_return
            .and_then(|fact| result_maybe_return(passthrough_err.clone(), fact))
            .unwrap_or_else(|| TypeFact::result(TypeFact::Any, TypeFact::Any)),
    }
}

fn result_or_else_return(
    passthrough_ok: TypeFact,
    shape: ResultShape,
    lambda_return: Option<&TypeFact>,
) -> TypeFact {
    match shape {
        ResultShape::Ok => TypeFact::result_ok(passthrough_ok),
        ResultShape::Err => result_chain_lambda_return(lambda_return),
        ResultShape::Maybe => lambda_return
            .and_then(|fact| result_or_else_maybe_return(passthrough_ok.clone(), fact))
            .unwrap_or_else(|| TypeFact::result(TypeFact::Any, TypeFact::Any)),
    }
}

fn result_or_else_maybe_return(passthrough_ok: TypeFact, fallback: &TypeFact) -> Option<TypeFact> {
    match fallback {
        TypeFact::Result { ok, err } => Some(TypeFact::result(
            TypeFact::union([passthrough_ok, (**ok).clone()]),
            (**err).clone(),
        )),
        TypeFact::ResultOk { ok } => Some(TypeFact::result_ok(TypeFact::union([
            passthrough_ok,
            (**ok).clone(),
        ]))),
        TypeFact::ResultErr { err } => Some(TypeFact::result(passthrough_ok, (**err).clone())),
        _ => None,
    }
}

fn result_maybe_return(passthrough_err: TypeFact, chained: &TypeFact) -> Option<TypeFact> {
    match chained {
        TypeFact::Result { ok, err } => Some(TypeFact::result(
            (**ok).clone(),
            TypeFact::union([passthrough_err, (**err).clone()]),
        )),
        TypeFact::ResultOk { ok } => Some(TypeFact::result((**ok).clone(), passthrough_err)),
        TypeFact::ResultErr { err } => Some(TypeFact::result_err(TypeFact::union([
            passthrough_err,
            (**err).clone(),
        ]))),
        _ => None,
    }
}

fn result_like_fact(fact: &TypeFact) -> Option<TypeFact> {
    match fact {
        TypeFact::Result { .. } | TypeFact::ResultOk { .. } | TypeFact::ResultErr { .. } => {
            Some(fact.clone())
        }
        _ => None,
    }
}

fn numeric_return(value: &TypeFact) -> TypeFact {
    match value {
        TypeFact::Float => TypeFact::Float,
        TypeFact::Int => TypeFact::Int,
        _ => TypeFact::Union(vec![TypeFact::Int, TypeFact::Float]),
    }
}
