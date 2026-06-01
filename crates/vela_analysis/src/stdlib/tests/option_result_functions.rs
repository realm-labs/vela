use super::*;

#[test]
fn unknown_or_unsupported_receiver_methods_have_no_stdlib_fact() {
    assert!(stdlib_method_fact(&TypeFact::Int, "len", None).is_none());
    assert!(stdlib_method_fact(&TypeFact::String, "map", None).is_none());
}

#[test]
fn option_and_result_functions_expose_dynamic_enum_facts() {
    let some = stdlib_function_fact("option.some", &[TypeFact::String]).expect("some fact");
    assert_eq!(some.returns, TypeFact::option(TypeFact::String));

    let unwrapped = stdlib_function_fact(
        "option.unwrap_or",
        &[TypeFact::option(TypeFact::String), TypeFact::String],
    )
    .expect("unwrap_or fact");
    assert_eq!(unwrapped.returns, TypeFact::String);
    let none_unwrapped = stdlib_function_fact(
        "option.unwrap_or",
        &[TypeFact::option_none(), TypeFact::String],
    )
    .expect("none unwrap_or fact");
    assert_eq!(none_unwrapped.returns, TypeFact::String);
    let ok_or = stdlib_function_fact(
        "option.ok_or",
        &[
            TypeFact::option(TypeFact::String),
            TypeFact::record("ParseError"),
        ],
    )
    .expect("ok_or fact");
    assert_eq!(
        ok_or.returns,
        TypeFact::result(TypeFact::String, TypeFact::record("ParseError"))
    );
    let none_ok_or =
        stdlib_function_fact("option.ok_or", &[TypeFact::option_none(), TypeFact::String])
            .expect("none ok_or fact");
    assert_eq!(none_ok_or.returns, TypeFact::result_err(TypeFact::String));
    let flattened_option = stdlib_function_fact(
        "option.flatten",
        &[TypeFact::option(TypeFact::option(TypeFact::Int))],
    )
    .expect("option flatten fact");
    assert_eq!(flattened_option.returns, TypeFact::option(TypeFact::Int));
    assert!(stdlib_function_fact("option.flatten", &[TypeFact::option(TypeFact::Int)]).is_none());

    let ok = stdlib_function_fact("result.ok", &[TypeFact::Int]).expect("ok fact");
    assert_eq!(ok.returns, TypeFact::result(TypeFact::Int, TypeFact::Any));

    let narrowed_ok_unwrapped = stdlib_function_fact(
        "result.unwrap_or",
        &[TypeFact::result_ok(TypeFact::Int), TypeFact::Float],
    )
    .expect("narrowed result unwrap_or fact");
    assert_eq!(narrowed_ok_unwrapped.returns, TypeFact::Int);

    let result_unwrapped = stdlib_function_fact(
        "result.unwrap_or",
        &[
            TypeFact::result(TypeFact::Int, TypeFact::String),
            TypeFact::Float,
        ],
    )
    .expect("result unwrap_or fact");
    assert_eq!(
        result_unwrapped.returns,
        TypeFact::Union(vec![TypeFact::Int, TypeFact::Float])
    );
    let to_option = stdlib_function_fact(
        "result.to_option",
        &[TypeFact::result(TypeFact::Int, TypeFact::String)],
    )
    .expect("to_option fact");
    assert_eq!(to_option.returns, TypeFact::option(TypeFact::Int));
    let err_to_option = stdlib_function_fact(
        "result.to_option",
        &[TypeFact::result_err(TypeFact::String)],
    )
    .expect("err to_option fact");
    assert_eq!(err_to_option.returns, TypeFact::option_none());

    let to_error_option = stdlib_function_fact(
        "result.to_error_option",
        &[TypeFact::result(TypeFact::Int, TypeFact::String)],
    )
    .expect("to_error_option fact");
    assert_eq!(to_error_option.returns, TypeFact::option(TypeFact::String));
    let err_to_error_option = stdlib_function_fact(
        "result.to_error_option",
        &[TypeFact::result_err(TypeFact::String)],
    )
    .expect("err to_error_option fact");
    assert_eq!(
        err_to_error_option.returns,
        TypeFact::option_some(TypeFact::String)
    );

    let flattened_result = stdlib_function_fact(
        "result.flatten",
        &[TypeFact::result(
            TypeFact::result(TypeFact::Int, TypeFact::String),
            TypeFact::record("OuterError"),
        )],
    )
    .expect("result flatten fact");
    assert_eq!(
        flattened_result.returns,
        TypeFact::result(
            TypeFact::Int,
            TypeFact::union([TypeFact::record("OuterError"), TypeFact::String])
        )
    );
    assert!(
        stdlib_function_fact(
            "result.flatten",
            &[TypeFact::result(TypeFact::Int, TypeFact::String)]
        )
        .is_none()
    );
}
