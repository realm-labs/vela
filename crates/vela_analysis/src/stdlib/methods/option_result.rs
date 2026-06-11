use super::*;

#[derive(Clone, Copy)]
pub(super) enum OptionShape {
    Maybe,
    Some,
    None,
}

pub(super) fn option_method_fact(
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
        "is_some" => Some(StdlibMethodFact::new(receiver, "is_some", TypeFact::BOOL)),
        "is_none" => Some(StdlibMethodFact::new(receiver, "is_none", TypeFact::BOOL)),
        "unwrap_or" => Some(
            StdlibMethodFact::new(
                receiver,
                "unwrap_or",
                option_unwrap_or_return(&some, shape, TypeFact::Any),
            )
            .with_params(vec![TypeFact::Any]),
        ),
        "ok_or" => Some(
            StdlibMethodFact::new(
                receiver,
                "ok_or",
                option_ok_or_return(&some, shape, TypeFact::Any),
            )
            .with_params(vec![TypeFact::Any]),
        ),
        "flatten" => option_flatten_return(&some, shape)
            .map(|returns| StdlibMethodFact::new(receiver, "flatten", returns)),
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
        "filter" => {
            let returns = match shape {
                OptionShape::Maybe | OptionShape::Some => TypeFact::option(some.clone()),
                OptionShape::None => TypeFact::option_none(),
            };
            Some(
                StdlibMethodFact::new(receiver, "filter", returns)
                    .with_lambda(vec![some], TypeFact::BOOL),
            )
        }
        _ => None,
    }
}

#[derive(Clone, Copy)]
pub(super) enum ResultShape {
    Maybe,
    Ok,
    Err,
}

pub(super) fn result_method_fact(
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
        "is_ok" => Some(StdlibMethodFact::new(receiver, "is_ok", TypeFact::BOOL)),
        "is_err" => Some(StdlibMethodFact::new(receiver, "is_err", TypeFact::BOOL)),
        "unwrap_or" => Some(
            StdlibMethodFact::new(
                receiver,
                "unwrap_or",
                result_unwrap_or_return(&ok, shape, TypeFact::Any),
            )
            .with_params(vec![TypeFact::Any]),
        ),
        "to_option" => Some(StdlibMethodFact::new(
            receiver,
            "to_option",
            result_to_option_return(&ok, shape),
        )),
        "to_error_option" => Some(StdlibMethodFact::new(
            receiver,
            "to_error_option",
            result_to_error_option_return(&err, shape),
        )),
        "flatten" => result_flatten_return(&ok, &err, shape)
            .map(|returns| StdlibMethodFact::new(receiver, "flatten", returns)),
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

pub(super) fn option_chain_lambda_return(lambda_return: Option<&TypeFact>) -> TypeFact {
    lambda_return
        .and_then(option_like_fact)
        .unwrap_or_else(|| TypeFact::option(TypeFact::Any))
}

pub(super) fn option_chain_return(shape: OptionShape, chained: &TypeFact) -> TypeFact {
    match shape {
        OptionShape::Some => chained.clone(),
        OptionShape::None => TypeFact::option_none(),
        OptionShape::Maybe => option_maybe_return(chained),
    }
}

pub(super) fn option_maybe_return(chained: &TypeFact) -> TypeFact {
    match chained {
        TypeFact::Option { some } | TypeFact::OptionSome { some } => {
            TypeFact::option((**some).clone())
        }
        TypeFact::OptionNone => TypeFact::option_none(),
        _ => TypeFact::option(TypeFact::Any),
    }
}

pub(super) fn option_or_else_return(
    some: TypeFact,
    shape: OptionShape,
    fallback: &TypeFact,
) -> TypeFact {
    match shape {
        OptionShape::Some => TypeFact::option_some(some),
        OptionShape::None => fallback.clone(),
        OptionShape::Maybe => option_or_else_maybe_return(some, fallback),
    }
}

pub(super) fn option_or_else_maybe_return(some: TypeFact, fallback: &TypeFact) -> TypeFact {
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

pub(super) fn option_unwrap_or_return(
    some: &TypeFact,
    shape: OptionShape,
    fallback: TypeFact,
) -> TypeFact {
    match shape {
        OptionShape::Some => some.clone(),
        OptionShape::None => fallback,
        OptionShape::Maybe => value_or_fallback(some.clone(), fallback),
    }
}

pub(super) fn option_ok_or_return(some: &TypeFact, shape: OptionShape, err: TypeFact) -> TypeFact {
    match shape {
        OptionShape::Some => TypeFact::result_ok(some.clone()),
        OptionShape::None => TypeFact::result_err(err),
        OptionShape::Maybe => TypeFact::result(some.clone(), err),
    }
}

pub(super) fn option_flatten_return(some: &TypeFact, shape: OptionShape) -> Option<TypeFact> {
    match shape {
        OptionShape::Some => option_like_fact(some),
        OptionShape::None => Some(TypeFact::option_none()),
        OptionShape::Maybe => option_maybe_flatten_return(some),
    }
}

pub(super) fn option_maybe_flatten_return(some: &TypeFact) -> Option<TypeFact> {
    match some {
        TypeFact::Option { some } | TypeFact::OptionSome { some } => {
            Some(TypeFact::option((**some).clone()))
        }
        TypeFact::OptionNone => Some(TypeFact::option_none()),
        TypeFact::Any | TypeFact::Unknown => Some(TypeFact::option(TypeFact::Any)),
        _ => None,
    }
}

pub(super) fn option_like_fact(fact: &TypeFact) -> Option<TypeFact> {
    match fact {
        TypeFact::Option { .. } | TypeFact::OptionSome { .. } | TypeFact::OptionNone => {
            Some(fact.clone())
        }
        TypeFact::Any | TypeFact::Unknown => Some(TypeFact::option(TypeFact::Any)),
        _ => None,
    }
}

pub(super) fn result_chain_lambda_return(lambda_return: Option<&TypeFact>) -> TypeFact {
    lambda_return
        .and_then(result_like_fact)
        .unwrap_or_else(|| TypeFact::result(TypeFact::Any, TypeFact::Any))
}

pub(super) fn result_unwrap_or_return(
    ok: &TypeFact,
    shape: ResultShape,
    fallback: TypeFact,
) -> TypeFact {
    match shape {
        ResultShape::Ok => ok.clone(),
        ResultShape::Err => fallback,
        ResultShape::Maybe => value_or_fallback(ok.clone(), fallback),
    }
}

pub(super) fn result_to_option_return(ok: &TypeFact, shape: ResultShape) -> TypeFact {
    match shape {
        ResultShape::Ok => TypeFact::option_some(ok.clone()),
        ResultShape::Err => TypeFact::option_none(),
        ResultShape::Maybe => TypeFact::option(ok.clone()),
    }
}

pub(super) fn result_to_error_option_return(err: &TypeFact, shape: ResultShape) -> TypeFact {
    match shape {
        ResultShape::Ok => TypeFact::option_none(),
        ResultShape::Err => TypeFact::option_some(err.clone()),
        ResultShape::Maybe => TypeFact::option(err.clone()),
    }
}

pub(super) fn result_flatten_return(
    ok: &TypeFact,
    err: &TypeFact,
    shape: ResultShape,
) -> Option<TypeFact> {
    match shape {
        ResultShape::Ok => result_like_fact(ok),
        ResultShape::Err => Some(TypeFact::result_err(err.clone())),
        ResultShape::Maybe => result_maybe_flatten_return(ok, err),
    }
}

pub(super) fn result_maybe_flatten_return(ok: &TypeFact, err: &TypeFact) -> Option<TypeFact> {
    match ok {
        TypeFact::Result {
            ok: inner_ok,
            err: inner_err,
        } => Some(TypeFact::result(
            (**inner_ok).clone(),
            TypeFact::union([err.clone(), (**inner_err).clone()]),
        )),
        TypeFact::ResultOk { ok: inner_ok } => {
            Some(TypeFact::result((**inner_ok).clone(), err.clone()))
        }
        TypeFact::ResultErr { err: inner_err } => Some(TypeFact::result_err(TypeFact::union([
            err.clone(),
            (**inner_err).clone(),
        ]))),
        TypeFact::Any | TypeFact::Unknown => Some(TypeFact::result(TypeFact::Any, TypeFact::Any)),
        _ => None,
    }
}

pub(super) fn result_chain_return(
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

pub(super) fn result_or_else_return(
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

pub(super) fn result_or_else_maybe_return(
    passthrough_ok: TypeFact,
    fallback: &TypeFact,
) -> Option<TypeFact> {
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

pub(super) fn result_maybe_return(
    passthrough_err: TypeFact,
    chained: &TypeFact,
) -> Option<TypeFact> {
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

pub(super) fn result_like_fact(fact: &TypeFact) -> Option<TypeFact> {
    match fact {
        TypeFact::Result { .. } | TypeFact::ResultOk { .. } | TypeFact::ResultErr { .. } => {
            Some(fact.clone())
        }
        TypeFact::Any | TypeFact::Unknown => Some(TypeFact::result(TypeFact::Any, TypeFact::Any)),
        _ => None,
    }
}
