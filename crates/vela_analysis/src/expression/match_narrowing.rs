use vela_syntax::{Expr, ExprKind, Pattern, RecordPatternField};

use super::{ExprFactScope, type_fact_from_expr_with_registry};
use crate::{RegistryFacts, TypeFact};

pub(super) fn narrowed_by_match_pattern(
    scope: &ExprFactScope,
    scrutinee: &Expr,
    pattern: &Pattern,
    facts: &RegistryFacts,
) -> ExprFactScope {
    let mut narrowed = scope.clone();
    let scrutinee_fact = type_fact_from_expr_with_registry(scrutinee, scope, facts);
    let Some(scrutinee_path) = expr_path(scrutinee) else {
        bind_pattern_facts(&mut narrowed, pattern, &scrutinee_fact, facts);
        return narrowed;
    };

    if let Some((enum_name, variant)) = pattern_variant(pattern, &scrutinee_fact, facts) {
        narrowed.paths.insert(
            scrutinee_path.to_vec(),
            TypeFact::enum_type(enum_name.clone(), Some(variant.clone())),
        );
        bind_variant_pattern_facts(&mut narrowed, pattern, &enum_name, &variant, facts);
    } else {
        bind_pattern_facts(&mut narrowed, pattern, &scrutinee_fact, facts);
    }
    narrowed
}

fn expr_path(expr: &Expr) -> Option<&[String]> {
    match &expr.kind {
        ExprKind::Path(path) => Some(path.as_slice()),
        _ => None,
    }
}

fn pattern_variant(
    pattern: &Pattern,
    scrutinee_fact: &TypeFact,
    facts: &RegistryFacts,
) -> Option<(String, String)> {
    let (path, variant) = match pattern {
        Pattern::Path(path) => pattern_path_variant(path, scrutinee_fact, facts)?,
        Pattern::TupleVariant { path, .. } | Pattern::RecordVariant { path, .. } => {
            pattern_path_variant(path, scrutinee_fact, facts)?
        }
        Pattern::Wildcard | Pattern::Literal(_) | Pattern::Binding(_) => return None,
    };
    Some((path, variant))
}

fn pattern_path_variant(
    path: &[String],
    scrutinee_fact: &TypeFact,
    facts: &RegistryFacts,
) -> Option<(String, String)> {
    let (variant, owner_path) = path.split_last()?;
    if owner_path.is_empty() {
        return match scrutinee_fact {
            TypeFact::Enum { name, .. } if facts.variant_fact(name, variant).is_some() => {
                Some((name.clone(), variant.clone()))
            }
            _ => None,
        };
    }

    let owner = owner_path.join(".");
    facts
        .variant_fact(&owner, variant)
        .map(|_| (owner, variant.clone()))
}

fn bind_pattern_facts(
    scope: &mut ExprFactScope,
    pattern: &Pattern,
    scrutinee_fact: &TypeFact,
    facts: &RegistryFacts,
) {
    match pattern {
        Pattern::Binding(name) => scope.insert_path([name.clone()], scrutinee_fact.clone()),
        Pattern::TupleVariant { fields, .. } => {
            bind_tuple_fields(scope, fields, None, facts);
        }
        Pattern::RecordVariant { fields, .. } => {
            bind_record_fields(scope, fields, None, facts);
        }
        Pattern::Wildcard | Pattern::Literal(_) | Pattern::Path(_) => {}
    }
}

fn bind_variant_pattern_facts(
    scope: &mut ExprFactScope,
    pattern: &Pattern,
    enum_name: &str,
    variant: &str,
    facts: &RegistryFacts,
) {
    let owner = format!("{enum_name}.{variant}");
    match pattern {
        Pattern::TupleVariant { fields, .. } => {
            bind_tuple_fields(scope, fields, Some(&owner), facts);
        }
        Pattern::RecordVariant { fields, .. } => {
            bind_record_fields(scope, fields, Some(&owner), facts);
        }
        Pattern::Binding(name) => {
            scope.insert_path(
                [name.clone()],
                TypeFact::enum_type(enum_name, Some(variant)),
            );
        }
        Pattern::Wildcard | Pattern::Literal(_) | Pattern::Path(_) => {}
    }
}

fn bind_tuple_fields(
    scope: &mut ExprFactScope,
    fields: &[Pattern],
    owner: Option<&str>,
    facts: &RegistryFacts,
) {
    for (index, pattern) in fields.iter().enumerate() {
        let fact = owner
            .and_then(|owner| facts.field_fact(owner, &index.to_string()).cloned())
            .unwrap_or(TypeFact::Unknown);
        bind_pattern_facts(scope, pattern, &fact, facts);
    }
}

fn bind_record_fields(
    scope: &mut ExprFactScope,
    fields: &[RecordPatternField],
    owner: Option<&str>,
    facts: &RegistryFacts,
) {
    for field in fields {
        let fact = owner
            .and_then(|owner| facts.field_fact(owner, &field.name).cloned())
            .unwrap_or(TypeFact::Unknown);
        match &field.pattern {
            Some(pattern) => bind_pattern_facts(scope, pattern, &fact, facts),
            None => scope.insert_path([field.name.clone()], fact),
        }
    }
}
