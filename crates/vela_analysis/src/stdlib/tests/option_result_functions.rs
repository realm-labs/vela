use super::*;

#[test]
fn unknown_or_unsupported_receiver_methods_have_no_stdlib_fact() {
    assert!(stdlib_method_fact(&TypeFact::I64, "len", None).is_none());
    assert!(stdlib_method_fact(&TypeFact::STRING, "map", None).is_none());
}

#[test]
fn option_and_result_functions_expose_dynamic_enum_facts() {
    let some = stdlib_function_fact("option::some", &[TypeFact::STRING]).expect("some fact");
    assert_eq!(some.returns, TypeFact::option(TypeFact::STRING));

    let unwrapped = stdlib_function_fact(
        "option::unwrap_or",
        &[TypeFact::option(TypeFact::STRING), TypeFact::STRING],
    )
    .expect("unwrap_or fact");
    assert_eq!(unwrapped.returns, TypeFact::STRING);
    let none_unwrapped = stdlib_function_fact(
        "option::unwrap_or",
        &[TypeFact::option_none(), TypeFact::STRING],
    )
    .expect("none unwrap_or fact");
    assert_eq!(none_unwrapped.returns, TypeFact::STRING);
    let ok_or = stdlib_function_fact(
        "option::ok_or",
        &[
            TypeFact::option(TypeFact::STRING),
            TypeFact::record("ParseError"),
        ],
    )
    .expect("ok_or fact");
    assert_eq!(
        ok_or.returns,
        TypeFact::result(TypeFact::STRING, TypeFact::record("ParseError"))
    );
    let none_ok_or = stdlib_function_fact(
        "option::ok_or",
        &[TypeFact::option_none(), TypeFact::STRING],
    )
    .expect("none ok_or fact");
    assert_eq!(none_ok_or.returns, TypeFact::result_err(TypeFact::STRING));
    let flattened_option = stdlib_function_fact(
        "option::flatten",
        &[TypeFact::option(TypeFact::option(TypeFact::I64))],
    )
    .expect("option flatten fact");
    assert_eq!(flattened_option.returns, TypeFact::option(TypeFact::I64));
    assert!(stdlib_function_fact("option::flatten", &[TypeFact::option(TypeFact::I64)]).is_none());

    let ok = stdlib_function_fact("result::ok", &[TypeFact::I64]).expect("ok fact");
    assert_eq!(ok.returns, TypeFact::result(TypeFact::I64, TypeFact::Any));

    let narrowed_ok_unwrapped = stdlib_function_fact(
        "result::unwrap_or",
        &[TypeFact::result_ok(TypeFact::I64), TypeFact::F64],
    )
    .expect("narrowed result unwrap_or fact");
    assert_eq!(narrowed_ok_unwrapped.returns, TypeFact::I64);

    let result_unwrapped = stdlib_function_fact(
        "result::unwrap_or",
        &[
            TypeFact::result(TypeFact::I64, TypeFact::STRING),
            TypeFact::F64,
        ],
    )
    .expect("result unwrap_or fact");
    assert_eq!(
        result_unwrapped.returns,
        TypeFact::Union(vec![TypeFact::I64, TypeFact::F64])
    );
    let to_option = stdlib_function_fact(
        "result::to_option",
        &[TypeFact::result(TypeFact::I64, TypeFact::STRING)],
    )
    .expect("to_option fact");
    assert_eq!(to_option.returns, TypeFact::option(TypeFact::I64));
    let err_to_option = stdlib_function_fact(
        "result::to_option",
        &[TypeFact::result_err(TypeFact::STRING)],
    )
    .expect("err to_option fact");
    assert_eq!(err_to_option.returns, TypeFact::option_none());

    let to_error_option = stdlib_function_fact(
        "result::to_error_option",
        &[TypeFact::result(TypeFact::I64, TypeFact::STRING)],
    )
    .expect("to_error_option fact");
    assert_eq!(to_error_option.returns, TypeFact::option(TypeFact::STRING));
    let err_to_error_option = stdlib_function_fact(
        "result::to_error_option",
        &[TypeFact::result_err(TypeFact::STRING)],
    )
    .expect("err to_error_option fact");
    assert_eq!(
        err_to_error_option.returns,
        TypeFact::option_some(TypeFact::STRING)
    );

    let flattened_result = stdlib_function_fact(
        "result::flatten",
        &[TypeFact::result(
            TypeFact::result(TypeFact::I64, TypeFact::STRING),
            TypeFact::record("OuterError"),
        )],
    )
    .expect("result flatten fact");
    assert_eq!(
        flattened_result.returns,
        TypeFact::result(
            TypeFact::I64,
            TypeFact::union([TypeFact::record("OuterError"), TypeFact::STRING])
        )
    );
    assert!(
        stdlib_function_fact(
            "result::flatten",
            &[TypeFact::result(TypeFact::I64, TypeFact::STRING)]
        )
        .is_none()
    );
}
