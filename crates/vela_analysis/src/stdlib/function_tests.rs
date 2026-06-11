use super::*;

#[test]
fn math_set_and_time_functions_expose_return_facts() {
    assert_eq!(
        stdlib_function_fact("math::max", &[TypeFact::I64, TypeFact::I64])
            .expect("max fact")
            .returns,
        TypeFact::I64
    );
    assert_eq!(
        stdlib_function_fact(
            "math::clamp",
            &[TypeFact::F64, TypeFact::I64, TypeFact::F64],
        )
        .expect("clamp fact")
        .returns,
        TypeFact::F64
    );
    assert_eq!(
        stdlib_function_fact("math::lerp", &[TypeFact::I64, TypeFact::I64, TypeFact::F64])
            .expect("lerp fact")
            .returns,
        TypeFact::F64
    );
    assert_eq!(
        stdlib_function_fact(
            "math::move_towards",
            &[TypeFact::I64, TypeFact::I64, TypeFact::I64]
        )
        .expect("move_towards i64 fact")
        .returns,
        TypeFact::I64
    );
    assert_eq!(
        stdlib_function_fact(
            "math::move_towards",
            &[TypeFact::I64, TypeFact::F64, TypeFact::I64],
        )
        .expect("move_towards f64 fact")
        .returns,
        TypeFact::F64
    );
    assert_eq!(
        stdlib_function_fact(
            "math::distance2d",
            &[TypeFact::I64, TypeFact::I64, TypeFact::F64, TypeFact::F64,],
        )
        .expect("distance2d fact")
        .returns,
        TypeFact::F64
    );
    assert_eq!(
        stdlib_function_fact(
            "math::distance3d",
            &[
                TypeFact::I64,
                TypeFact::I64,
                TypeFact::I64,
                TypeFact::F64,
                TypeFact::F64,
                TypeFact::F64,
            ],
        )
        .expect("distance3d fact")
        .returns,
        TypeFact::F64
    );
    assert_eq!(
        stdlib_function_fact("math::pow", &[TypeFact::I64, TypeFact::F64])
            .expect("pow fact")
            .returns,
        TypeFact::Union(vec![TypeFact::I64, TypeFact::F64])
    );
    assert_eq!(
        stdlib_function_fact("math::sqrt", &[TypeFact::I64])
            .expect("sqrt fact")
            .returns,
        TypeFact::F64
    );
    assert_eq!(
        stdlib_function_fact("math::sign", &[TypeFact::F64])
            .expect("sign fact")
            .returns,
        TypeFact::I64
    );
    assert_eq!(
        stdlib_function_fact("math::floor", &[TypeFact::F64])
            .expect("floor fact")
            .returns,
        TypeFact::I64
    );
    assert_eq!(
        stdlib_function_fact("math::ceil", &[TypeFact::F64])
            .expect("ceil fact")
            .returns,
        TypeFact::I64
    );
    assert_eq!(
        stdlib_function_fact("math::round", &[TypeFact::F64])
            .expect("round fact")
            .returns,
        TypeFact::I64
    );
    assert_eq!(
        stdlib_function_fact("math::abs", &[TypeFact::F64])
            .expect("abs fact")
            .returns,
        TypeFact::F64
    );
    assert_eq!(
        stdlib_function_fact("set::from_array", &[TypeFact::array(TypeFact::STRING)])
            .expect("set::from_array fact")
            .returns,
        TypeFact::set(TypeFact::STRING)
    );
    assert_eq!(
        stdlib_function_fact("bytes::from_hex", &[TypeFact::STRING])
            .expect("bytes::from_hex fact")
            .returns,
        TypeFact::result(TypeFact::BYTES, TypeFact::STRING)
    );
    assert_eq!(
        stdlib_function_fact("i64::from_i32", &[TypeFact::I64])
            .expect("i64::from_i32 fact")
            .returns,
        TypeFact::I64
    );
    assert_eq!(
        stdlib_function_fact("i8::try_from_i64", &[TypeFact::I64])
            .expect("i8::try_from_i64 fact")
            .returns,
        TypeFact::result(TypeFact::I64, TypeFact::STRING)
    );
    assert_eq!(
        stdlib_function_fact("f32::try_from_f64", &[TypeFact::F64])
            .expect("f32::try_from_f64 fact")
            .returns,
        TypeFact::result(TypeFact::F64, TypeFact::STRING)
    );
    assert_eq!(
        stdlib_function_fact("u8::wrapping_add", &[TypeFact::I64, TypeFact::I64])
            .expect("u8::wrapping_add fact")
            .returns,
        TypeFact::I64
    );
    assert_eq!(
        stdlib_function_fact("u8::rotate_right", &[TypeFact::I64, TypeFact::I64])
            .expect("u8::rotate_right fact")
            .returns,
        TypeFact::I64
    );
    assert_eq!(
        stdlib_function_fact("time::now", &[])
            .expect("time::now fact")
            .returns,
        TypeFact::I64
    );
    assert_eq!(
        stdlib_function_fact("time::tick", &[])
            .expect("time::tick fact")
            .returns,
        TypeFact::I64
    );
    assert_eq!(
        stdlib_function_fact("time::elapsed_since", &[TypeFact::I64])
            .expect("time::elapsed_since fact")
            .returns,
        TypeFact::I64
    );
}

#[test]
fn function_completion_facts_enumerate_global_api_surface() {
    let number = TypeFact::Union(vec![TypeFact::I64, TypeFact::F64]);
    let facts = stdlib_function_completion_facts();

    assert!(facts.iter().any(|fact| {
        fact.name == "option::unwrap_or"
            && fact.params == vec![TypeFact::option(TypeFact::Any), TypeFact::Any]
            && fact.returns == TypeFact::Any
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "option::ok_or"
            && fact.params == vec![TypeFact::option(TypeFact::Any), TypeFact::Any]
            && fact.returns == TypeFact::result(TypeFact::Any, TypeFact::Any)
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "option::flatten"
            && fact.params == vec![TypeFact::option(TypeFact::option(TypeFact::Any))]
            && fact.returns == TypeFact::option(TypeFact::Any)
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "result::to_option"
            && fact.params == vec![TypeFact::result(TypeFact::Any, TypeFact::Any)]
            && fact.returns == TypeFact::option(TypeFact::Any)
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "bytes::from_hex"
            && fact.params == vec![TypeFact::STRING]
            && fact.returns == TypeFact::result(TypeFact::BYTES, TypeFact::STRING)
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "i64::from_i32"
            && fact.params == vec![TypeFact::I64]
            && fact.returns == TypeFact::I64
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "f32::try_from_f64"
            && fact.params == vec![TypeFact::F64]
            && fact.returns == TypeFact::result(TypeFact::F64, TypeFact::STRING)
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "u8::bit_and"
            && fact.params == vec![TypeFact::I64, TypeFact::I64]
            && fact.returns == TypeFact::I64
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "u8::shift_left"
            && fact.params == vec![TypeFact::I64, TypeFact::I64]
            && fact.returns == TypeFact::I64
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "result::to_error_option"
            && fact.params == vec![TypeFact::result(TypeFact::Any, TypeFact::Any)]
            && fact.returns == TypeFact::option(TypeFact::Any)
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "result::flatten"
            && fact.params
                == vec![TypeFact::result(
                    TypeFact::result(TypeFact::Any, TypeFact::Any),
                    TypeFact::Any,
                )]
            && fact.returns == TypeFact::result(TypeFact::Any, TypeFact::Any)
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "math::clamp" && fact.params.len() == 3 && fact.returns == number
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "math::lerp" && fact.params.len() == 3 && fact.returns == TypeFact::F64
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "math::move_towards" && fact.params.len() == 3 && fact.returns == number
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "math::distance2d" && fact.params.len() == 4 && fact.returns == TypeFact::F64
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "math::distance3d" && fact.params.len() == 6 && fact.returns == TypeFact::F64
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "math::pow" && fact.params.len() == 2 && fact.returns == number
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "math::sqrt" && fact.params.len() == 1 && fact.returns == TypeFact::F64
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "math::sign" && fact.params.len() == 1 && fact.returns == TypeFact::I64
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "math::floor" && fact.params.len() == 1 && fact.returns == TypeFact::I64
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "math::ceil" && fact.params.len() == 1 && fact.returns == TypeFact::I64
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "math::round" && fact.params.len() == 1 && fact.returns == TypeFact::I64
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "math::abs" && fact.params.len() == 1 && fact.returns == number
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "set::from_array" && fact.returns == TypeFact::set(TypeFact::Any)
    }));
    assert!(
        facts
            .iter()
            .any(|fact| fact.name == "time::now" && fact.returns == TypeFact::I64)
    );
    assert!(
        facts
            .iter()
            .any(|fact| fact.name == "time::tick" && fact.returns == TypeFact::I64)
    );
    assert!(facts.iter().any(|fact| {
        fact.name == "time::elapsed_since"
            && fact.params == [TypeFact::I64]
            && fact.returns == TypeFact::I64
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "reflect::types"
            && fact.returns == TypeFact::array(TypeFact::record("ReflectType"))
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "reflect::fields"
            && fact.returns == TypeFact::array(TypeFact::record("ReflectField"))
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "reflect::functions"
            && fact.returns == TypeFact::array(TypeFact::record("ReflectFunction"))
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "reflect::exports"
            && fact.params
                == vec![TypeFact::union([
                    TypeFact::STRING,
                    TypeFact::record("ReflectModule"),
                ])]
            && fact.returns == TypeFact::array(TypeFact::STRING)
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "reflect::call"
            && fact.params == vec![TypeFact::Any, TypeFact::STRING]
            && fact.returns == TypeFact::Any
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "reflect::call"
            && fact.params == vec![TypeFact::record("ReflectFunction")]
            && fact.returns == TypeFact::Any
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "reflect::implements"
            && fact.params
                == vec![
                    TypeFact::Any,
                    TypeFact::union([TypeFact::STRING, TypeFact::record("ReflectTrait")]),
                ]
            && fact.returns == TypeFact::BOOL
    }));
}
