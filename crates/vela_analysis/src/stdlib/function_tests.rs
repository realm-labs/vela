use super::*;

#[test]
fn math_set_and_time_functions_expose_return_facts() {
    assert_eq!(
        stdlib_function_fact("math::max", &[TypeFact::Int, TypeFact::Int])
            .expect("max fact")
            .returns,
        TypeFact::Int
    );
    assert_eq!(
        stdlib_function_fact(
            "math::clamp",
            &[TypeFact::Float, TypeFact::Int, TypeFact::Float],
        )
        .expect("clamp fact")
        .returns,
        TypeFact::Float
    );
    assert_eq!(
        stdlib_function_fact(
            "math::lerp",
            &[TypeFact::Int, TypeFact::Int, TypeFact::Float]
        )
        .expect("lerp fact")
        .returns,
        TypeFact::Float
    );
    assert_eq!(
        stdlib_function_fact(
            "math::move_towards",
            &[TypeFact::Int, TypeFact::Int, TypeFact::Int]
        )
        .expect("move_towards int fact")
        .returns,
        TypeFact::Int
    );
    assert_eq!(
        stdlib_function_fact(
            "math::move_towards",
            &[TypeFact::Int, TypeFact::Float, TypeFact::Int],
        )
        .expect("move_towards float fact")
        .returns,
        TypeFact::Float
    );
    assert_eq!(
        stdlib_function_fact(
            "math::distance2d",
            &[
                TypeFact::Int,
                TypeFact::Int,
                TypeFact::Float,
                TypeFact::Float,
            ],
        )
        .expect("distance2d fact")
        .returns,
        TypeFact::Float
    );
    assert_eq!(
        stdlib_function_fact(
            "math::distance3d",
            &[
                TypeFact::Int,
                TypeFact::Int,
                TypeFact::Int,
                TypeFact::Float,
                TypeFact::Float,
                TypeFact::Float,
            ],
        )
        .expect("distance3d fact")
        .returns,
        TypeFact::Float
    );
    assert_eq!(
        stdlib_function_fact("math::pow", &[TypeFact::Int, TypeFact::Float])
            .expect("pow fact")
            .returns,
        TypeFact::Union(vec![TypeFact::Int, TypeFact::Float])
    );
    assert_eq!(
        stdlib_function_fact("math::sqrt", &[TypeFact::Int])
            .expect("sqrt fact")
            .returns,
        TypeFact::Float
    );
    assert_eq!(
        stdlib_function_fact("math::sign", &[TypeFact::Float])
            .expect("sign fact")
            .returns,
        TypeFact::Int
    );
    assert_eq!(
        stdlib_function_fact("math::floor", &[TypeFact::Float])
            .expect("floor fact")
            .returns,
        TypeFact::Int
    );
    assert_eq!(
        stdlib_function_fact("math::ceil", &[TypeFact::Float])
            .expect("ceil fact")
            .returns,
        TypeFact::Int
    );
    assert_eq!(
        stdlib_function_fact("math::round", &[TypeFact::Float])
            .expect("round fact")
            .returns,
        TypeFact::Int
    );
    assert_eq!(
        stdlib_function_fact("math::abs", &[TypeFact::Float])
            .expect("abs fact")
            .returns,
        TypeFact::Float
    );
    assert_eq!(
        stdlib_function_fact("set::from_array", &[TypeFact::array(TypeFact::String)])
            .expect("set::from_array fact")
            .returns,
        TypeFact::set(TypeFact::String)
    );
    assert_eq!(
        stdlib_function_fact("bytes::from_hex", &[TypeFact::String])
            .expect("bytes::from_hex fact")
            .returns,
        TypeFact::result(TypeFact::Bytes, TypeFact::String)
    );
    assert_eq!(
        stdlib_function_fact("i64::from_i32", &[TypeFact::Int])
            .expect("i64::from_i32 fact")
            .returns,
        TypeFact::Int
    );
    assert_eq!(
        stdlib_function_fact("i8::try_from_i64", &[TypeFact::Int])
            .expect("i8::try_from_i64 fact")
            .returns,
        TypeFact::result(TypeFact::Int, TypeFact::String)
    );
    assert_eq!(
        stdlib_function_fact("f32::try_from_f64", &[TypeFact::Float])
            .expect("f32::try_from_f64 fact")
            .returns,
        TypeFact::result(TypeFact::Float, TypeFact::String)
    );
    assert_eq!(
        stdlib_function_fact("time::now", &[])
            .expect("time::now fact")
            .returns,
        TypeFact::Int
    );
    assert_eq!(
        stdlib_function_fact("time::tick", &[])
            .expect("time::tick fact")
            .returns,
        TypeFact::Int
    );
    assert_eq!(
        stdlib_function_fact("time::elapsed_since", &[TypeFact::Int])
            .expect("time::elapsed_since fact")
            .returns,
        TypeFact::Int
    );
}

#[test]
fn function_completion_facts_enumerate_global_api_surface() {
    let number = TypeFact::Union(vec![TypeFact::Int, TypeFact::Float]);
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
            && fact.params == vec![TypeFact::String]
            && fact.returns == TypeFact::result(TypeFact::Bytes, TypeFact::String)
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "i64::from_i32"
            && fact.params == vec![TypeFact::Int]
            && fact.returns == TypeFact::Int
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "f32::try_from_f64"
            && fact.params == vec![TypeFact::Float]
            && fact.returns == TypeFact::result(TypeFact::Float, TypeFact::String)
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
        fact.name == "math::lerp" && fact.params.len() == 3 && fact.returns == TypeFact::Float
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "math::move_towards" && fact.params.len() == 3 && fact.returns == number
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "math::distance2d" && fact.params.len() == 4 && fact.returns == TypeFact::Float
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "math::distance3d" && fact.params.len() == 6 && fact.returns == TypeFact::Float
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "math::pow" && fact.params.len() == 2 && fact.returns == number
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "math::sqrt" && fact.params.len() == 1 && fact.returns == TypeFact::Float
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "math::sign" && fact.params.len() == 1 && fact.returns == TypeFact::Int
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "math::floor" && fact.params.len() == 1 && fact.returns == TypeFact::Int
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "math::ceil" && fact.params.len() == 1 && fact.returns == TypeFact::Int
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "math::round" && fact.params.len() == 1 && fact.returns == TypeFact::Int
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
            .any(|fact| fact.name == "time::now" && fact.returns == TypeFact::Int)
    );
    assert!(
        facts
            .iter()
            .any(|fact| fact.name == "time::tick" && fact.returns == TypeFact::Int)
    );
    assert!(facts.iter().any(|fact| {
        fact.name == "time::elapsed_since"
            && fact.params == [TypeFact::Int]
            && fact.returns == TypeFact::Int
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
                    TypeFact::String,
                    TypeFact::record("ReflectModule"),
                ])]
            && fact.returns == TypeFact::array(TypeFact::String)
    }));
    assert!(facts.iter().any(|fact| {
        fact.name == "reflect::call"
            && fact.params == vec![TypeFact::Any, TypeFact::String]
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
                    TypeFact::union([TypeFact::String, TypeFact::record("ReflectTrait")]),
                ]
            && fact.returns == TypeFact::Bool
    }));
}
