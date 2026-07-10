use super::*;
use crate::logical_records::{LogicalRecordKind, fixed_record, map_entry};

#[test]
fn reflection_functions_expose_metadata_facts() {
    assert_eq!(
        stdlib_function_fact("reflect::type_of", &[TypeFact::host("Player")])
            .expect("reflect::type_of fact")
            .returns,
        TypeFact::option(logical(LogicalRecordKind::ReflectType))
    );
    assert_eq!(
        stdlib_function_fact("reflect::types", &[])
            .expect("reflect::types fact")
            .returns,
        TypeFact::array(logical(LogicalRecordKind::ReflectType))
    );
    assert_eq!(
        stdlib_function_fact("reflect::attrs", &[TypeFact::host("Player")])
            .expect("reflect::attrs fact")
            .returns,
        TypeFact::map(TypeFact::STRING, TypeFact::STRING)
    );
    assert_eq!(
        stdlib_function_fact("reflect::id", &[TypeFact::host("Player")])
            .expect("reflect::id fact")
            .returns,
        TypeFact::I64
    );
    assert_eq!(
        stdlib_function_fact("reflect::source_span", &[TypeFact::host("Player")])
            .expect("reflect::source_span fact")
            .returns,
        TypeFact::option(logical(LogicalRecordKind::ReflectSourceSpan))
    );
    assert_eq!(
        stdlib_function_fact(
            "reflect::origin",
            &[logical(LogicalRecordKind::ReflectFunction)],
        )
        .expect("reflect::origin fact")
        .returns,
        TypeFact::option(TypeFact::STRING)
    );
    assert_eq!(
        stdlib_function_fact(
            "reflect::required_permissions",
            &[logical(LogicalRecordKind::ReflectFunction)]
        )
        .expect("reflect::required_permissions fact")
        .returns,
        TypeFact::array(TypeFact::STRING)
    );
    assert_eq!(
        stdlib_function_fact(
            "reflect::effects",
            &[logical(LogicalRecordKind::ReflectFunction)],
        )
        .expect("reflect::effects fact")
        .returns,
        logical(LogicalRecordKind::ReflectEffectSet)
    );
    assert_eq!(
        stdlib_function_fact(
            "reflect::owner",
            &[logical(LogicalRecordKind::ReflectMethod)],
        )
        .expect("reflect::owner fact")
        .returns,
        TypeFact::STRING
    );
    assert_eq!(
        stdlib_function_fact(
            "reflect::access",
            &[logical(LogicalRecordKind::ReflectMethod)],
        )
        .expect("reflect::access fact")
        .returns,
        logical(LogicalRecordKind::ReflectMethodAccess)
    );
    assert_eq!(
        stdlib_function_fact(
            "reflect::params",
            &[logical(LogicalRecordKind::ReflectFunction)],
        )
        .expect("reflect::params fact")
        .returns,
        TypeFact::array(logical(LogicalRecordKind::ReflectParam))
    );
    assert_eq!(
        stdlib_function_fact(
            "reflect::returns",
            &[logical(LogicalRecordKind::ReflectFunction)],
        )
        .expect("reflect::returns fact")
        .returns,
        TypeFact::option(TypeFact::STRING)
    );
    assert_eq!(
        stdlib_function_fact(
            "reflect::attr",
            &[TypeFact::host("Player"), TypeFact::STRING]
        )
        .expect("reflect::attr fact")
        .returns,
        TypeFact::option(TypeFact::STRING)
    );
    assert_eq!(
        stdlib_function_fact(
            "reflect::has_attr",
            &[TypeFact::host("Player"), TypeFact::STRING]
        )
        .expect("reflect::has_attr fact")
        .returns,
        TypeFact::BOOL
    );
    assert_eq!(
        stdlib_function_fact("reflect::fields", &[])
            .expect("reflect::fields all fact")
            .returns,
        TypeFact::array(logical(LogicalRecordKind::ReflectField))
    );
    assert_eq!(
        stdlib_function_fact("reflect::fields", &[TypeFact::host("Player")])
            .expect("reflect::fields value fact")
            .returns,
        TypeFact::array(logical(LogicalRecordKind::ReflectField))
    );
    assert_eq!(
        stdlib_function_fact(
            "reflect::method",
            &[TypeFact::host("Player"), TypeFact::STRING]
        )
        .expect("reflect::method fact")
        .returns,
        logical(LogicalRecordKind::ReflectMethod)
    );
    assert_eq!(
        stdlib_function_fact("reflect::functions", &[])
            .expect("reflect::functions fact")
            .returns,
        TypeFact::array(logical(LogicalRecordKind::ReflectFunction))
    );
    assert_eq!(
        stdlib_function_fact(
            "reflect::exports",
            &[logical(LogicalRecordKind::ReflectModule)],
        )
        .expect("reflect::exports module fact")
        .returns,
        TypeFact::array(TypeFact::STRING)
    );
    assert_eq!(
        stdlib_function_fact(
            "reflect::call",
            &[TypeFact::host("Player"), TypeFact::STRING, TypeFact::I64,]
        )
        .expect("reflect::call fact")
        .returns,
        TypeFact::Any
    );
    assert_eq!(
        stdlib_function_fact(
            "reflect::call",
            &[logical(LogicalRecordKind::ReflectFunction)],
        )
        .expect("reflect::call function descriptor fact")
        .returns,
        TypeFact::Any
    );
    assert_eq!(
        stdlib_function_fact(
            "reflect::call",
            &[
                logical(LogicalRecordKind::ReflectFunction),
                TypeFact::I64,
                TypeFact::STRING,
            ]
        )
        .expect("reflect::call function descriptor args fact")
        .returns,
        TypeFact::Any
    );
    assert_eq!(
        stdlib_function_fact(
            "reflect::implements",
            &[
                TypeFact::host("Player"),
                logical(LogicalRecordKind::ReflectTrait),
            ]
        )
        .expect("reflect::implements trait descriptor fact")
        .returns,
        TypeFact::BOOL
    );
    assert_eq!(
        stdlib_function_fact(
            "reflect::variant_is",
            &[
                TypeFact::enum_type("QuestProgress", Some("Active")),
                TypeFact::STRING,
            ]
        )
        .expect("reflect::variant_is fact")
        .returns,
        TypeFact::BOOL
    );
    assert!(stdlib_function_fact("reflect::call", &[TypeFact::host("Player")]).is_none());
    assert!(stdlib_function_fact("reflect::fields", &[TypeFact::Any, TypeFact::Any]).is_none());
}

#[test]
fn stdlib_function_facts_reject_unknown_names_and_wrong_arity() {
    assert!(stdlib_function_fact("option::some", &[]).is_none());
    assert!(stdlib_function_fact("game::spawn", &[TypeFact::STRING]).is_none());
}

#[test]
fn stdlib_method_facts_enumerate_receiver_api_surface() {
    let array_facts = stdlib_method_facts(&TypeFact::array(TypeFact::I64), None);
    assert!(array_facts.iter().any(|fact| {
        fact.method == "values" && fact.returns == TypeFact::iterator(TypeFact::I64)
    }));

    let map = TypeFact::map(TypeFact::STRING, TypeFact::I64);
    let facts = stdlib_method_facts(&map, Some(&TypeFact::BOOL));

    assert!(facts.iter().any(|fact| {
        fact.method == "map_values"
            && fact.returns == TypeFact::map(TypeFact::STRING, TypeFact::BOOL)
    }));
    assert!(facts.iter().any(|fact| {
        fact.method == "entries"
            && fact.returns == TypeFact::iterator(map_entry(TypeFact::STRING, TypeFact::I64))
    }));
    assert!(facts.iter().any(|fact| {
        fact.method == "filter"
            && fact
                .lambda
                .as_ref()
                .is_some_and(|lambda| lambda.params == vec![TypeFact::STRING, TypeFact::I64])
    }));
    let iterator_facts =
        stdlib_method_facts(&TypeFact::iterator(TypeFact::I64), Some(&TypeFact::STRING));
    assert!(iterator_facts.iter().any(|fact| {
        fact.method == "map" && fact.returns == TypeFact::iterator(TypeFact::STRING)
    }));
    assert!(iterator_facts.iter().any(|fact| {
        fact.method == "collect_array" && fact.returns == TypeFact::array(TypeFact::I64)
    }));
    let option_facts = stdlib_method_facts(&TypeFact::option(TypeFact::I64), None);
    assert!(
        option_facts
            .iter()
            .any(|fact| fact.method == "unwrap_or" && fact.params == vec![TypeFact::Any])
    );
    assert!(option_facts.iter().any(|fact| fact.method == "ok_or"
        && fact.returns == TypeFact::result(TypeFact::I64, TypeFact::Any)));
    assert!(option_facts.iter().any(|fact| {
        fact.method == "map"
            && fact
                .lambda
                .as_ref()
                .is_some_and(|lambda| lambda.params == vec![TypeFact::I64])
    }));
    let nested_option_facts =
        stdlib_method_facts(&TypeFact::option(TypeFact::option(TypeFact::I64)), None);
    assert!(nested_option_facts.iter().any(|fact| {
        fact.method == "flatten" && fact.returns == TypeFact::option(TypeFact::I64)
    }));
    let result_facts =
        stdlib_method_facts(&TypeFact::result(TypeFact::I64, TypeFact::STRING), None);
    assert!(
        result_facts
            .iter()
            .any(|fact| fact.method == "unwrap_or" && fact.params == vec![TypeFact::Any])
    );
    assert!(
        result_facts
            .iter()
            .any(|fact| fact.method == "to_option"
                && fact.returns == TypeFact::option(TypeFact::I64))
    );
    assert!(
        result_facts
            .iter()
            .any(|fact| fact.method == "to_error_option"
                && fact.returns == TypeFact::option(TypeFact::STRING))
    );
    assert!(result_facts.iter().any(|fact| {
        fact.method == "map_err"
            && fact
                .lambda
                .as_ref()
                .is_some_and(|lambda| lambda.params == vec![TypeFact::STRING])
    }));
    let nested_result_facts = stdlib_method_facts(
        &TypeFact::result(
            TypeFact::result(TypeFact::I64, TypeFact::STRING),
            TypeFact::record("OuterError"),
        ),
        None,
    );
    assert!(nested_result_facts.iter().any(|fact| {
        fact.method == "flatten"
            && fact.returns
                == TypeFact::result(
                    TypeFact::I64,
                    TypeFact::union([TypeFact::record("OuterError"), TypeFact::STRING]),
                )
    }));
    assert!(
        stdlib_method_facts(
            &TypeFact::Host {
                name: "Player".into()
            },
            None
        )
        .is_empty()
    );
}

fn logical(kind: LogicalRecordKind) -> TypeFact {
    fixed_record(kind)
}
