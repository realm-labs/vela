use crate::type_fact::TypeFact;

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
        assert_eq!(try_fact(TypeFact::option(TypeFact::I64)), TypeFact::I64);
        assert_eq!(
            try_fact(TypeFact::option_some(TypeFact::STRING)),
            TypeFact::STRING
        );
        assert_eq!(
            try_fact(TypeFact::result(TypeFact::host("Reward"), TypeFact::STRING)),
            TypeFact::host("Reward")
        );
        assert_eq!(try_fact(TypeFact::result_ok(TypeFact::F64)), TypeFact::F64);
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
                TypeFact::option(TypeFact::I64),
                TypeFact::result_ok(TypeFact::STRING),
                TypeFact::option_none(),
            ])),
            TypeFact::union([TypeFact::I64, TypeFact::STRING])
        );
    }
}
