use crate::TypeFact;

pub(super) fn try_fact(value: TypeFact) -> TypeFact {
    match value {
        TypeFact::Option { some } | TypeFact::OptionSome { some } => *some,
        TypeFact::OptionNone => TypeFact::Never,
        TypeFact::Result { ok, .. } | TypeFact::ResultOk { ok } => *ok,
        TypeFact::ResultErr { .. } => TypeFact::Never,
        TypeFact::Union(facts) => TypeFact::union(facts.into_iter().map(try_fact)),
        TypeFact::Any => TypeFact::Any,
        TypeFact::Unknown => TypeFact::Unknown,
        _ => TypeFact::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwraps_option_and_result_success_payloads() {
        assert_eq!(try_fact(TypeFact::option(TypeFact::Int)), TypeFact::Int);
        assert_eq!(
            try_fact(TypeFact::option_some(TypeFact::String)),
            TypeFact::String
        );
        assert_eq!(
            try_fact(TypeFact::result(TypeFact::host("Reward"), TypeFact::String)),
            TypeFact::host("Reward")
        );
        assert_eq!(
            try_fact(TypeFact::result_ok(TypeFact::Float)),
            TypeFact::Float
        );
    }

    #[test]
    fn treats_guaranteed_early_return_variants_as_never() {
        assert_eq!(try_fact(TypeFact::option_none()), TypeFact::Never);
        assert_eq!(
            try_fact(TypeFact::result_err(TypeFact::record("Error"))),
            TypeFact::Never
        );
    }

    #[test]
    fn unions_success_payloads_and_drops_early_return_only_paths() {
        assert_eq!(
            try_fact(TypeFact::union([
                TypeFact::option(TypeFact::Int),
                TypeFact::result_ok(TypeFact::String),
                TypeFact::option_none(),
            ])),
            TypeFact::union([TypeFact::Int, TypeFact::String])
        );
    }
}
