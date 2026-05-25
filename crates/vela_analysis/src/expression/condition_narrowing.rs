use vela_syntax::{BinaryOp, Expr, ExprKind, Literal, UnaryOp};

use super::ExprFactScope;
use crate::TypeFact;

pub(super) fn narrowed_by_condition(
    scope: &ExprFactScope,
    condition: &Expr,
    truthy: bool,
) -> ExprFactScope {
    let mut narrowed = scope.clone();
    if let Some((path, expects_null)) = null_check(condition, truthy)
        && let Some(fact) = scope.path_fact(path)
    {
        narrowed.paths.insert(
            path.to_vec(),
            if expects_null {
                fact.only_null()
            } else {
                fact.without_null()
            },
        );
    }
    if let Some((path, fact)) = option_result_predicate(condition, scope, truthy) {
        narrowed.paths.insert(path.to_vec(), fact);
    }
    narrowed
}

fn null_check(condition: &Expr, truthy: bool) -> Option<(&[String], bool)> {
    match &condition.kind {
        ExprKind::Unary {
            op: UnaryOp::Not,
            expr,
        } => null_check(expr, !truthy),
        ExprKind::Binary { op, left, right } => {
            let equality_expects_null = match op {
                BinaryOp::Equal => truthy,
                BinaryOp::NotEqual => !truthy,
                _ => return None,
            };

            if let Some(path) = path_if_null_check(left, right) {
                return Some((path, equality_expects_null));
            }
            path_if_null_check(right, left).map(|path| (path, equality_expects_null))
        }
        _ => None,
    }
}

fn path_if_null_check<'a>(path_expr: &'a Expr, null_expr: &Expr) -> Option<&'a [String]> {
    let ExprKind::Path(path) = &path_expr.kind else {
        return None;
    };
    if matches!(null_expr.kind, ExprKind::Literal(Literal::Null)) {
        Some(path.as_slice())
    } else {
        None
    }
}

fn option_result_predicate<'a>(
    condition: &'a Expr,
    scope: &ExprFactScope,
    truthy: bool,
) -> Option<(&'a [String], TypeFact)> {
    match &condition.kind {
        ExprKind::Unary {
            op: UnaryOp::Not,
            expr,
        } => option_result_predicate(expr, scope, !truthy),
        ExprKind::Call { callee, args } => {
            let ExprKind::Path(callee_path) = &callee.kind else {
                return None;
            };
            let [arg] = args.as_slice() else {
                return None;
            };
            let ExprKind::Path(arg_path) = &arg.value.kind else {
                return None;
            };
            let fact = scope.path_fact(arg_path)?;
            let variant = predicate_variant(&callee_path.join("."), truthy)?;
            let narrowed = narrowed_variant_fact(fact, variant)?;
            Some((arg_path.as_slice(), narrowed))
        }
        _ => None,
    }
}

fn predicate_variant(name: &str, truthy: bool) -> Option<DynamicVariant> {
    match (name, truthy) {
        ("option.is_some", true) | ("option.is_none", false) => Some(DynamicVariant::OptionSome),
        ("option.is_some", false) | ("option.is_none", true) => Some(DynamicVariant::OptionNone),
        ("result.is_ok", true) | ("result.is_err", false) => Some(DynamicVariant::ResultOk),
        ("result.is_ok", false) | ("result.is_err", true) => Some(DynamicVariant::ResultErr),
        _ => None,
    }
}

fn narrowed_variant_fact(fact: &TypeFact, variant: DynamicVariant) -> Option<TypeFact> {
    match (fact, variant) {
        (TypeFact::Option { some }, DynamicVariant::OptionSome) => {
            Some(TypeFact::option_some((**some).clone()))
        }
        (TypeFact::Option { .. }, DynamicVariant::OptionNone) => Some(TypeFact::option_none()),
        (TypeFact::OptionSome { some }, DynamicVariant::OptionSome) => {
            Some(TypeFact::option_some((**some).clone()))
        }
        (TypeFact::OptionNone, DynamicVariant::OptionNone) => Some(TypeFact::option_none()),
        (TypeFact::Result { ok, .. }, DynamicVariant::ResultOk) => {
            Some(TypeFact::result_ok((**ok).clone()))
        }
        (TypeFact::Result { err, .. }, DynamicVariant::ResultErr) => {
            Some(TypeFact::result_err((**err).clone()))
        }
        (TypeFact::ResultOk { ok }, DynamicVariant::ResultOk) => {
            Some(TypeFact::result_ok((**ok).clone()))
        }
        (TypeFact::ResultErr { err }, DynamicVariant::ResultErr) => {
            Some(TypeFact::result_err((**err).clone()))
        }
        _ => None,
    }
}

#[derive(Clone, Copy)]
enum DynamicVariant {
    OptionSome,
    OptionNone,
    ResultOk,
    ResultErr,
}
