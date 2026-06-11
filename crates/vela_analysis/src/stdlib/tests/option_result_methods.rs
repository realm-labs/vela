use super::*;

#[test]
fn set_lambda_methods_expose_element_parameter_facts() {
    let receiver = TypeFact::set(TypeFact::STRING);

    let mapped = stdlib_method_fact(&receiver, "map", Some(&TypeFact::I64)).expect("map fact");
    assert_eq!(
        mapped.lambda.expect("map lambda").params,
        vec![TypeFact::STRING]
    );
    assert_eq!(mapped.returns, TypeFact::set(TypeFact::I64));

    let filter = stdlib_method_fact(&receiver, "filter", None).expect("filter fact");
    assert_eq!(
        filter.lambda.expect("filter lambda").params,
        vec![TypeFact::STRING]
    );
    assert_eq!(filter.returns, receiver);

    let found = stdlib_method_fact(&receiver, "find", None).expect("find fact");
    assert_eq!(
        found.lambda.expect("find lambda").params,
        vec![TypeFact::STRING]
    );
    assert_eq!(found.returns, TypeFact::option(TypeFact::STRING));

    let any = stdlib_method_fact(&receiver, "any", None).expect("any fact");
    assert_eq!(
        any.lambda.expect("any lambda").params,
        vec![TypeFact::STRING]
    );
    assert_eq!(any.returns, TypeFact::BOOL);

    let all = stdlib_method_fact(&receiver, "all", None).expect("all fact");
    assert_eq!(
        all.lambda.expect("all lambda").params,
        vec![TypeFact::STRING]
    );
    assert_eq!(all.returns, TypeFact::BOOL);

    let count = stdlib_method_fact(&receiver, "count", None).expect("count fact");
    assert_eq!(
        count.lambda.expect("count lambda").params,
        vec![TypeFact::STRING]
    );
    assert_eq!(count.returns, TypeFact::I64);
}

#[test]
fn option_and_result_map_methods_expose_dynamic_enum_facts() {
    let unwrapped_option = stdlib_method_fact(&TypeFact::option(TypeFact::I64), "unwrap_or", None)
        .expect("option unwrap_or fact");
    assert_eq!(
        unwrapped_option.returns,
        TypeFact::union([TypeFact::I64, TypeFact::Any])
    );
    assert_eq!(unwrapped_option.params, vec![TypeFact::Any]);

    let unwrapped_some =
        stdlib_method_fact(&TypeFact::option_some(TypeFact::I64), "unwrap_or", None)
            .expect("some unwrap_or fact");
    assert_eq!(unwrapped_some.returns, TypeFact::I64);

    let ok_or = stdlib_method_fact(&TypeFact::option(TypeFact::I64), "ok_or", None)
        .expect("option ok_or fact");
    assert_eq!(
        ok_or.returns,
        TypeFact::result(TypeFact::I64, TypeFact::Any)
    );
    assert_eq!(ok_or.params, vec![TypeFact::Any]);

    let none_ok_or =
        stdlib_method_fact(&TypeFact::option_none(), "ok_or", None).expect("none ok_or fact");
    assert_eq!(none_ok_or.returns, TypeFact::result_err(TypeFact::Any));

    let maybe = stdlib_method_fact(
        &TypeFact::option(TypeFact::I64),
        "map",
        Some(&TypeFact::STRING),
    )
    .expect("option map fact");
    assert_eq!(maybe.returns, TypeFact::option(TypeFact::STRING));
    assert_eq!(
        maybe.lambda.expect("option map lambda").params,
        vec![TypeFact::I64]
    );

    let some = stdlib_method_fact(
        &TypeFact::option_some(TypeFact::I64),
        "map",
        Some(&TypeFact::STRING),
    )
    .expect("some map fact");
    assert_eq!(some.returns, TypeFact::option_some(TypeFact::STRING));

    let none = stdlib_method_fact(&TypeFact::option_none(), "map", Some(&TypeFact::STRING))
        .expect("none map fact");
    assert_eq!(none.returns, TypeFact::option_none());

    let chained = stdlib_method_fact(
        &TypeFact::option(TypeFact::I64),
        "and_then",
        Some(&TypeFact::option(TypeFact::STRING)),
    )
    .expect("option and_then fact");
    assert_eq!(chained.returns, TypeFact::option(TypeFact::STRING));
    assert_eq!(
        chained.lambda.expect("option and_then lambda").params,
        vec![TypeFact::I64]
    );

    let chained_some = stdlib_method_fact(
        &TypeFact::option_some(TypeFact::I64),
        "and_then",
        Some(&TypeFact::option_none()),
    )
    .expect("some and_then fact");
    assert_eq!(chained_some.returns, TypeFact::option_none());

    let recovered = stdlib_method_fact(
        &TypeFact::option(TypeFact::I64),
        "or_else",
        Some(&TypeFact::option(TypeFact::STRING)),
    )
    .expect("option or_else fact");
    assert_eq!(
        recovered.returns,
        TypeFact::option(TypeFact::union([TypeFact::I64, TypeFact::STRING]))
    );
    assert_eq!(
        recovered.lambda.expect("option or_else lambda").params,
        Vec::<TypeFact>::new()
    );

    let recovered_some = stdlib_method_fact(
        &TypeFact::option_some(TypeFact::I64),
        "or_else",
        Some(&TypeFact::option(TypeFact::STRING)),
    )
    .expect("some or_else fact");
    assert_eq!(recovered_some.returns, TypeFact::option_some(TypeFact::I64));

    let recovered_none = stdlib_method_fact(
        &TypeFact::option_none(),
        "or_else",
        Some(&TypeFact::option_some(TypeFact::STRING)),
    )
    .expect("none or_else fact");
    assert_eq!(
        recovered_none.returns,
        TypeFact::option_some(TypeFact::STRING)
    );

    let filtered = stdlib_method_fact(&TypeFact::option(TypeFact::I64), "filter", None)
        .expect("option filter fact");
    assert_eq!(filtered.returns, TypeFact::option(TypeFact::I64));
    assert_eq!(
        filtered.lambda.expect("option filter lambda").params,
        vec![TypeFact::I64]
    );

    let filtered_some =
        stdlib_method_fact(&TypeFact::option_some(TypeFact::STRING), "filter", None)
            .expect("some filter fact");
    assert_eq!(filtered_some.returns, TypeFact::option(TypeFact::STRING));

    let filtered_none =
        stdlib_method_fact(&TypeFact::option_none(), "filter", None).expect("none filter fact");
    assert_eq!(filtered_none.returns, TypeFact::option_none());

    let flattened_option = stdlib_method_fact(
        &TypeFact::option(TypeFact::option(TypeFact::STRING)),
        "flatten",
        None,
    )
    .expect("option flatten fact");
    assert_eq!(flattened_option.returns, TypeFact::option(TypeFact::STRING));
    assert!(stdlib_method_fact(&TypeFact::option(TypeFact::STRING), "flatten", None).is_none());

    let result = stdlib_method_fact(
        &TypeFact::result(TypeFact::I64, TypeFact::record("Error")),
        "map",
        Some(&TypeFact::STRING),
    )
    .expect("result map fact");
    assert_eq!(
        result.returns,
        TypeFact::result(TypeFact::STRING, TypeFact::record("Error"))
    );
    assert_eq!(
        result.lambda.expect("result map lambda").params,
        vec![TypeFact::I64]
    );

    let ok = stdlib_method_fact(
        &TypeFact::result_ok(TypeFact::I64),
        "map",
        Some(&TypeFact::STRING),
    )
    .expect("ok map fact");
    assert_eq!(ok.returns, TypeFact::result_ok(TypeFact::STRING));

    let err = stdlib_method_fact(
        &TypeFact::result_err(TypeFact::record("Error")),
        "map",
        Some(&TypeFact::STRING),
    )
    .expect("err map fact");
    assert_eq!(err.returns, TypeFact::result_err(TypeFact::record("Error")));

    let mapped_error = stdlib_method_fact(
        &TypeFact::result(TypeFact::I64, TypeFact::record("Error")),
        "map_err",
        Some(&TypeFact::STRING),
    )
    .expect("result map_err fact");
    assert_eq!(
        mapped_error.returns,
        TypeFact::result(TypeFact::I64, TypeFact::STRING)
    );
    assert_eq!(
        mapped_error.lambda.expect("map_err lambda").params,
        vec![TypeFact::record("Error")]
    );

    let ok_error = stdlib_method_fact(
        &TypeFact::result_ok(TypeFact::I64),
        "map_err",
        Some(&TypeFact::STRING),
    )
    .expect("ok map_err fact");
    assert_eq!(ok_error.returns, TypeFact::result_ok(TypeFact::I64));

    let err_error = stdlib_method_fact(
        &TypeFact::result_err(TypeFact::record("Error")),
        "map_err",
        Some(&TypeFact::STRING),
    )
    .expect("err map_err fact");
    assert_eq!(err_error.returns, TypeFact::result_err(TypeFact::STRING));

    let chained_result = stdlib_method_fact(
        &TypeFact::result(TypeFact::I64, TypeFact::record("Error")),
        "and_then",
        Some(&TypeFact::result(TypeFact::STRING, TypeFact::STRING)),
    )
    .expect("result and_then fact");
    assert_eq!(
        chained_result.returns,
        TypeFact::result(
            TypeFact::STRING,
            TypeFact::union([TypeFact::record("Error"), TypeFact::STRING])
        )
    );
    assert_eq!(
        chained_result
            .lambda
            .expect("result and_then lambda")
            .params,
        vec![TypeFact::I64]
    );

    let chained_ok = stdlib_method_fact(
        &TypeFact::result_ok(TypeFact::I64),
        "and_then",
        Some(&TypeFact::result_err(TypeFact::STRING)),
    )
    .expect("ok and_then fact");
    assert_eq!(chained_ok.returns, TypeFact::result_err(TypeFact::STRING));

    let chained_err = stdlib_method_fact(
        &TypeFact::result_err(TypeFact::record("Error")),
        "and_then",
        Some(&TypeFact::result(TypeFact::STRING, TypeFact::STRING)),
    )
    .expect("err and_then fact");
    assert_eq!(
        chained_err.returns,
        TypeFact::result_err(TypeFact::record("Error"))
    );

    let recovered_result = stdlib_method_fact(
        &TypeFact::result(TypeFact::I64, TypeFact::record("Error")),
        "or_else",
        Some(&TypeFact::result(TypeFact::STRING, TypeFact::STRING)),
    )
    .expect("result or_else fact");
    assert_eq!(
        recovered_result.returns,
        TypeFact::result(
            TypeFact::union([TypeFact::I64, TypeFact::STRING]),
            TypeFact::STRING
        )
    );
    assert_eq!(
        recovered_result
            .lambda
            .expect("result or_else lambda")
            .params,
        vec![TypeFact::record("Error")]
    );

    let recovered_ok = stdlib_method_fact(
        &TypeFact::result_ok(TypeFact::I64),
        "or_else",
        Some(&TypeFact::result(TypeFact::STRING, TypeFact::STRING)),
    )
    .expect("ok or_else fact");
    assert_eq!(recovered_ok.returns, TypeFact::result_ok(TypeFact::I64));

    let recovered_err = stdlib_method_fact(
        &TypeFact::result_err(TypeFact::record("Error")),
        "or_else",
        Some(&TypeFact::result_ok(TypeFact::STRING)),
    )
    .expect("err or_else fact");
    assert_eq!(recovered_err.returns, TypeFact::result_ok(TypeFact::STRING));

    let result_is_ok = stdlib_method_fact(
        &TypeFact::result(TypeFact::I64, TypeFact::record("Error")),
        "is_ok",
        None,
    )
    .expect("result is_ok fact");
    assert_eq!(result_is_ok.returns, TypeFact::BOOL);

    let unwrapped_result = stdlib_method_fact(
        &TypeFact::result(TypeFact::I64, TypeFact::record("Error")),
        "unwrap_or",
        None,
    )
    .expect("result unwrap_or fact");
    assert_eq!(
        unwrapped_result.returns,
        TypeFact::union([TypeFact::I64, TypeFact::Any])
    );
    assert_eq!(unwrapped_result.params, vec![TypeFact::Any]);

    let ok_to_option = stdlib_method_fact(&TypeFact::result_ok(TypeFact::I64), "to_option", None)
        .expect("ok to_option fact");
    assert_eq!(ok_to_option.returns, TypeFact::option_some(TypeFact::I64));

    let err_to_option = stdlib_method_fact(
        &TypeFact::result_err(TypeFact::record("Error")),
        "to_option",
        None,
    )
    .expect("err to_option fact");
    assert_eq!(err_to_option.returns, TypeFact::option_none());

    let maybe_to_error_option = stdlib_method_fact(
        &TypeFact::result(TypeFact::I64, TypeFact::record("Error")),
        "to_error_option",
        None,
    )
    .expect("maybe to_error_option fact");
    assert_eq!(
        maybe_to_error_option.returns,
        TypeFact::option(TypeFact::record("Error"))
    );

    let err_to_error_option = stdlib_method_fact(
        &TypeFact::result_err(TypeFact::record("Error")),
        "to_error_option",
        None,
    )
    .expect("err to_error_option fact");
    assert_eq!(
        err_to_error_option.returns,
        TypeFact::option_some(TypeFact::record("Error"))
    );

    let flattened_result = stdlib_method_fact(
        &TypeFact::result(
            TypeFact::result(TypeFact::STRING, TypeFact::record("InnerError")),
            TypeFact::record("OuterError"),
        ),
        "flatten",
        None,
    )
    .expect("result flatten fact");
    assert_eq!(
        flattened_result.returns,
        TypeFact::result(
            TypeFact::STRING,
            TypeFact::union([
                TypeFact::record("OuterError"),
                TypeFact::record("InnerError")
            ])
        )
    );
    assert!(
        stdlib_method_fact(
            &TypeFact::result(TypeFact::STRING, TypeFact::record("Error")),
            "flatten",
            None
        )
        .is_none()
    );
}
