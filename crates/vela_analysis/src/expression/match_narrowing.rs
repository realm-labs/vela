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
            scrutinee_variant_fact(&scrutinee_fact, &enum_name, &variant),
        );
        bind_variant_pattern_facts(
            &mut narrowed,
            pattern,
            &scrutinee_fact,
            &enum_name,
            &variant,
            facts,
        );
    } else {
        bind_pattern_facts(&mut narrowed, pattern, &scrutinee_fact, facts);
    }
    narrowed
}

fn scrutinee_variant_fact(scrutinee_fact: &TypeFact, enum_name: &str, variant: &str) -> TypeFact {
    match (scrutinee_fact, enum_name.rsplit('.').next(), variant) {
        (TypeFact::Option { some }, Some("Option"), "Some")
        | (TypeFact::OptionSome { some }, Some("Option"), "Some") => {
            TypeFact::option_some((**some).clone())
        }
        (TypeFact::Option { .. } | TypeFact::OptionNone, Some("Option"), "None") => {
            TypeFact::option_none()
        }
        (TypeFact::Result { ok, .. }, Some("Result"), "Ok")
        | (TypeFact::ResultOk { ok }, Some("Result"), "Ok") => TypeFact::result_ok((**ok).clone()),
        (TypeFact::Result { err, .. }, Some("Result"), "Err")
        | (TypeFact::ResultErr { err }, Some("Result"), "Err") => {
            TypeFact::result_err((**err).clone())
        }
        _ => TypeFact::enum_type(enum_name, Some(variant)),
    }
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
            TypeFact::Option { .. } | TypeFact::OptionSome { .. } | TypeFact::OptionNone
                if is_option_variant(variant) =>
            {
                Some(("Option".to_owned(), variant.clone()))
            }
            TypeFact::Result { .. } | TypeFact::ResultOk { .. } | TypeFact::ResultErr { .. }
                if is_result_variant(variant) =>
            {
                Some(("Result".to_owned(), variant.clone()))
            }
            _ => None,
        };
    }

    let owner = owner_path.join(".");
    facts
        .variant_fact(&owner, variant)
        .map(|_| (owner.clone(), variant.clone()))
        .or_else(|| dynamic_enum_variant(scrutinee_fact, &owner, variant))
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
    scrutinee_fact: &TypeFact,
    enum_name: &str,
    variant: &str,
    facts: &RegistryFacts,
) {
    let owner = format!("{enum_name}.{variant}");
    match pattern {
        Pattern::TupleVariant { fields, .. } => {
            bind_tuple_fields(
                scope,
                fields,
                Some((&owner, scrutinee_fact, variant)),
                facts,
            );
        }
        Pattern::RecordVariant { fields, .. } => {
            bind_record_fields(
                scope,
                fields,
                Some((&owner, scrutinee_fact, variant)),
                facts,
            );
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
    owner: Option<(&str, &TypeFact, &str)>,
    facts: &RegistryFacts,
) {
    for (index, pattern) in fields.iter().enumerate() {
        let fact = owner
            .and_then(|(owner, scrutinee_fact, variant)| {
                variant_field_fact(owner, scrutinee_fact, variant, &index.to_string(), facts)
            })
            .unwrap_or(TypeFact::Unknown);
        bind_pattern_facts(scope, pattern, &fact, facts);
    }
}

fn bind_record_fields(
    scope: &mut ExprFactScope,
    fields: &[RecordPatternField],
    owner: Option<(&str, &TypeFact, &str)>,
    facts: &RegistryFacts,
) {
    for field in fields {
        let fact = owner
            .and_then(|(owner, scrutinee_fact, variant)| {
                variant_field_fact(owner, scrutinee_fact, variant, &field.name, facts)
            })
            .unwrap_or(TypeFact::Unknown);
        match &field.pattern {
            Some(pattern) => bind_pattern_facts(scope, pattern, &fact, facts),
            None => scope.insert_path([field.name.clone()], fact),
        }
    }
}

fn dynamic_enum_variant(
    scrutinee_fact: &TypeFact,
    owner: &str,
    variant: &str,
) -> Option<(String, String)> {
    match scrutinee_fact {
        TypeFact::Option { .. } | TypeFact::OptionSome { .. } | TypeFact::OptionNone
            if owner.rsplit('.').next() == Some("Option") && is_option_variant(variant) =>
        {
            Some((owner.to_owned(), variant.to_owned()))
        }
        TypeFact::Result { .. } | TypeFact::ResultOk { .. } | TypeFact::ResultErr { .. }
            if owner.rsplit('.').next() == Some("Result") && is_result_variant(variant) =>
        {
            Some((owner.to_owned(), variant.to_owned()))
        }
        _ => None,
    }
}

fn variant_field_fact(
    owner: &str,
    scrutinee_fact: &TypeFact,
    variant: &str,
    field: &str,
    facts: &RegistryFacts,
) -> Option<TypeFact> {
    facts
        .field_fact(owner, field)
        .cloned()
        .or_else(|| dynamic_variant_field_fact(scrutinee_fact, variant, field))
}

fn dynamic_variant_field_fact(
    scrutinee_fact: &TypeFact,
    variant: &str,
    field: &str,
) -> Option<TypeFact> {
    if field != "0" {
        return None;
    }
    match (scrutinee_fact, variant) {
        (TypeFact::Option { some }, "Some") => Some((**some).clone()),
        (TypeFact::OptionSome { some }, "Some") => Some((**some).clone()),
        (TypeFact::Result { ok, .. }, "Ok") => Some((**ok).clone()),
        (TypeFact::Result { err, .. }, "Err") => Some((**err).clone()),
        (TypeFact::ResultOk { ok }, "Ok") => Some((**ok).clone()),
        (TypeFact::ResultErr { err }, "Err") => Some((**err).clone()),
        _ => None,
    }
}

fn is_option_variant(variant: &str) -> bool {
    matches!(variant, "Some" | "None")
}

fn is_result_variant(variant: &str) -> bool {
    matches!(variant, "Ok" | "Err")
}
