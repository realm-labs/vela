use crate::stdlib::StdlibFunctionFact;
use crate::type_fact::TypeFact;
use vela_common::PrimitiveTag;

pub(super) fn completion_facts() -> Vec<StdlibFunctionFact> {
    let number = number_fact();
    let mut facts = vec![
        StdlibFunctionFact::new(
            "option::some",
            vec![TypeFact::Any],
            TypeFact::option(TypeFact::Any),
        ),
        StdlibFunctionFact::new("option::none", Vec::new(), TypeFact::option(TypeFact::Any)),
        StdlibFunctionFact::new(
            "option::is_some",
            vec![TypeFact::option(TypeFact::Any)],
            TypeFact::BOOL,
        ),
        StdlibFunctionFact::new(
            "option::is_none",
            vec![TypeFact::option(TypeFact::Any)],
            TypeFact::BOOL,
        ),
        StdlibFunctionFact::new(
            "option::unwrap_or",
            vec![TypeFact::option(TypeFact::Any), TypeFact::Any],
            TypeFact::Any,
        ),
        StdlibFunctionFact::new(
            "option::ok_or",
            vec![TypeFact::option(TypeFact::Any), TypeFact::Any],
            TypeFact::result(TypeFact::Any, TypeFact::Any),
        ),
        StdlibFunctionFact::new(
            "option::flatten",
            vec![TypeFact::option(TypeFact::option(TypeFact::Any))],
            TypeFact::option(TypeFact::Any),
        ),
        StdlibFunctionFact::new(
            "result::ok",
            vec![TypeFact::Any],
            TypeFact::result(TypeFact::Any, TypeFact::Any),
        ),
        StdlibFunctionFact::new(
            "result::err",
            vec![TypeFact::Any],
            TypeFact::result(TypeFact::Any, TypeFact::Any),
        ),
        StdlibFunctionFact::new(
            "result::is_ok",
            vec![TypeFact::result(TypeFact::Any, TypeFact::Any)],
            TypeFact::BOOL,
        ),
        StdlibFunctionFact::new(
            "result::is_err",
            vec![TypeFact::result(TypeFact::Any, TypeFact::Any)],
            TypeFact::BOOL,
        ),
        StdlibFunctionFact::new(
            "result::unwrap_or",
            vec![
                TypeFact::result(TypeFact::Any, TypeFact::Any),
                TypeFact::Any,
            ],
            TypeFact::Any,
        ),
        StdlibFunctionFact::new(
            "result::to_option",
            vec![TypeFact::result(TypeFact::Any, TypeFact::Any)],
            TypeFact::option(TypeFact::Any),
        ),
        StdlibFunctionFact::new(
            "result::to_error_option",
            vec![TypeFact::result(TypeFact::Any, TypeFact::Any)],
            TypeFact::option(TypeFact::Any),
        ),
        StdlibFunctionFact::new(
            "result::flatten",
            vec![TypeFact::result(
                TypeFact::result(TypeFact::Any, TypeFact::Any),
                TypeFact::Any,
            )],
            TypeFact::result(TypeFact::Any, TypeFact::Any),
        ),
        StdlibFunctionFact::new(
            "math::max",
            vec![number.clone(), number.clone()],
            number.clone(),
        ),
        StdlibFunctionFact::new(
            "math::min",
            vec![number.clone(), number.clone()],
            number.clone(),
        ),
        StdlibFunctionFact::new(
            "math::clamp",
            vec![number.clone(), number.clone(), number.clone()],
            number.clone(),
        ),
        StdlibFunctionFact::new(
            "math::lerp",
            vec![number.clone(), number.clone(), number.clone()],
            TypeFact::F64,
        ),
        StdlibFunctionFact::new(
            "math::move_towards",
            vec![number.clone(), number.clone(), number.clone()],
            number.clone(),
        ),
        StdlibFunctionFact::new(
            "math::distance2d",
            vec![
                number.clone(),
                number.clone(),
                number.clone(),
                number.clone(),
            ],
            TypeFact::F64,
        ),
        StdlibFunctionFact::new(
            "math::distance3d",
            vec![
                number.clone(),
                number.clone(),
                number.clone(),
                number.clone(),
                number.clone(),
                number.clone(),
            ],
            TypeFact::F64,
        ),
        StdlibFunctionFact::new(
            "math::pow",
            vec![number.clone(), number.clone()],
            number.clone(),
        ),
        StdlibFunctionFact::new("math::sqrt", vec![number.clone()], TypeFact::F64),
        StdlibFunctionFact::new("math::sign", vec![number.clone()], TypeFact::I64),
        StdlibFunctionFact::new("math::floor", vec![number.clone()], TypeFact::I64),
        StdlibFunctionFact::new("math::ceil", vec![number.clone()], TypeFact::I64),
        StdlibFunctionFact::new("math::round", vec![number.clone()], TypeFact::I64),
        StdlibFunctionFact::new("math::abs", vec![number.clone()], number),
        StdlibFunctionFact::new(
            "math::random",
            vec![TypeFact::I64, TypeFact::I64],
            TypeFact::I64,
        ),
        StdlibFunctionFact::new("time::now", Vec::new(), TypeFact::I64),
        StdlibFunctionFact::new("time::tick", Vec::new(), TypeFact::I64),
        StdlibFunctionFact::new("time::elapsed_since", vec![TypeFact::I64], TypeFact::I64),
        StdlibFunctionFact::new(
            "io::print",
            vec![TypeFact::Any],
            TypeFact::result(TypeFact::NULL, TypeFact::record("IoError")),
        ),
        StdlibFunctionFact::new(
            "io::println",
            vec![TypeFact::Any],
            TypeFact::result(TypeFact::NULL, TypeFact::record("IoError")),
        ),
        StdlibFunctionFact::new(
            "fs::read_to_string",
            vec![TypeFact::STRING],
            TypeFact::result(TypeFact::STRING, TypeFact::record("IoError")),
        ),
        StdlibFunctionFact::new(
            "fs::write_string",
            vec![TypeFact::STRING, TypeFact::STRING],
            TypeFact::result(TypeFact::NULL, TypeFact::record("IoError")),
        ),
        StdlibFunctionFact::new(
            "set::from_array",
            vec![TypeFact::array(TypeFact::Any)],
            TypeFact::set(TypeFact::Any),
        ),
        StdlibFunctionFact::new(
            "bytes::from_hex",
            vec![TypeFact::STRING],
            TypeFact::result(TypeFact::BYTES, TypeFact::STRING),
        ),
        StdlibFunctionFact::new("i64::from_i32", vec![TypeFact::I64], TypeFact::I64),
        StdlibFunctionFact::new("u64::from_u32", vec![TypeFact::I64], TypeFact::I64),
        StdlibFunctionFact::new("f64::from_f32", vec![TypeFact::F64], TypeFact::F64),
        StdlibFunctionFact::new(
            "i8::try_from_i64",
            vec![TypeFact::I64],
            TypeFact::result(TypeFact::I64, TypeFact::STRING),
        ),
        StdlibFunctionFact::new(
            "u8::try_from_u64",
            vec![TypeFact::I64],
            TypeFact::result(TypeFact::I64, TypeFact::STRING),
        ),
        StdlibFunctionFact::new(
            "f32::try_from_f64",
            vec![TypeFact::F64],
            TypeFact::result(TypeFact::F64, TypeFact::STRING),
        ),
        StdlibFunctionFact::new(
            "u8::wrapping_add",
            vec![TypeFact::I64, TypeFact::I64],
            TypeFact::I64,
        ),
        StdlibFunctionFact::new(
            "u32::wrapping_mul",
            vec![TypeFact::I64, TypeFact::I64],
            TypeFact::I64,
        ),
        StdlibFunctionFact::new(
            "i8::wrapping_add",
            vec![TypeFact::I64, TypeFact::I64],
            TypeFact::I64,
        ),
        StdlibFunctionFact::new(
            "u8::bit_and",
            vec![TypeFact::I64, TypeFact::I64],
            TypeFact::I64,
        ),
        StdlibFunctionFact::new(
            "u8::bit_or",
            vec![TypeFact::I64, TypeFact::I64],
            TypeFact::I64,
        ),
        StdlibFunctionFact::new(
            "u8::bit_xor",
            vec![TypeFact::I64, TypeFact::I64],
            TypeFact::I64,
        ),
        StdlibFunctionFact::new(
            "u8::shift_left",
            vec![TypeFact::I64, TypeFact::I64],
            TypeFact::I64,
        ),
        StdlibFunctionFact::new(
            "u8::shift_right",
            vec![TypeFact::I64, TypeFact::I64],
            TypeFact::I64,
        ),
        StdlibFunctionFact::new(
            "u8::rotate_left",
            vec![TypeFact::I64, TypeFact::I64],
            TypeFact::I64,
        ),
        StdlibFunctionFact::new(
            "u8::rotate_right",
            vec![TypeFact::I64, TypeFact::I64],
            TypeFact::I64,
        ),
    ];
    facts.extend(super::reflect::completion_facts());
    facts
}

pub(super) fn function_fact(name: &str, args: &[TypeFact]) -> Option<StdlibFunctionFact> {
    if let Some(fact) = super::reflect::function_fact(name, args) {
        return Some(fact);
    }

    match name {
        "option::some" => {
            expect_len(args, 1)?;
            Some(StdlibFunctionFact::new(
                "option::some",
                args.to_vec(),
                TypeFact::option(args[0].clone()),
            ))
        }
        "option::none" => {
            expect_len(args, 0)?;
            Some(StdlibFunctionFact::new(
                "option::none",
                Vec::new(),
                TypeFact::option(TypeFact::Any),
            ))
        }
        "option::is_some" | "option::is_none" => {
            expect_len(args, 1)?;
            Some(StdlibFunctionFact::new(
                canonical_function_name(name)?,
                args.to_vec(),
                TypeFact::BOOL,
            ))
        }
        "option::unwrap_or" => {
            expect_len(args, 2)?;
            Some(StdlibFunctionFact::new(
                "option::unwrap_or",
                args.to_vec(),
                option_unwrap_or_return(&args[0], args[1].clone()),
            ))
        }
        "option::ok_or" => {
            expect_len(args, 2)?;
            Some(StdlibFunctionFact::new(
                "option::ok_or",
                args.to_vec(),
                option_ok_or_return(&args[0], args[1].clone()),
            ))
        }
        "option::flatten" => {
            expect_len(args, 1)?;
            option_flatten_return(&args[0])
                .map(|returns| StdlibFunctionFact::new("option::flatten", args.to_vec(), returns))
        }
        "result::ok" => {
            expect_len(args, 1)?;
            Some(StdlibFunctionFact::new(
                "result::ok",
                args.to_vec(),
                TypeFact::result(args[0].clone(), TypeFact::Any),
            ))
        }
        "result::err" => {
            expect_len(args, 1)?;
            Some(StdlibFunctionFact::new(
                "result::err",
                args.to_vec(),
                TypeFact::result(TypeFact::Any, args[0].clone()),
            ))
        }
        "result::is_ok" | "result::is_err" => {
            expect_len(args, 1)?;
            Some(StdlibFunctionFact::new(
                canonical_function_name(name)?,
                args.to_vec(),
                TypeFact::BOOL,
            ))
        }
        "result::unwrap_or" => {
            expect_len(args, 2)?;
            Some(StdlibFunctionFact::new(
                "result::unwrap_or",
                args.to_vec(),
                result_unwrap_or_return(&args[0], args[1].clone()),
            ))
        }
        "result::to_option" => {
            expect_len(args, 1)?;
            Some(StdlibFunctionFact::new(
                "result::to_option",
                args.to_vec(),
                result_to_option_return(&args[0]),
            ))
        }
        "result::to_error_option" => {
            expect_len(args, 1)?;
            Some(StdlibFunctionFact::new(
                "result::to_error_option",
                args.to_vec(),
                result_to_error_option_return(&args[0]),
            ))
        }
        "result::flatten" => {
            expect_len(args, 1)?;
            result_flatten_return(&args[0])
                .map(|returns| StdlibFunctionFact::new("result::flatten", args.to_vec(), returns))
        }
        "math::max" | "math::min" => {
            expect_len(args, 2)?;
            Some(StdlibFunctionFact::new(
                canonical_function_name(name)?,
                args.to_vec(),
                numeric_result(args),
            ))
        }
        "math::clamp" => {
            expect_len(args, 3)?;
            Some(StdlibFunctionFact::new(
                "math::clamp",
                args.to_vec(),
                numeric_result(args),
            ))
        }
        "math::lerp" => {
            expect_len(args, 3)?;
            Some(StdlibFunctionFact::new(
                "math::lerp",
                args.to_vec(),
                TypeFact::F64,
            ))
        }
        "math::move_towards" => {
            expect_len(args, 3)?;
            Some(StdlibFunctionFact::new(
                "math::move_towards",
                args.to_vec(),
                numeric_result(args),
            ))
        }
        "math::distance2d" => {
            expect_len(args, 4)?;
            Some(StdlibFunctionFact::new(
                "math::distance2d",
                args.to_vec(),
                TypeFact::F64,
            ))
        }
        "math::distance3d" => {
            expect_len(args, 6)?;
            Some(StdlibFunctionFact::new(
                "math::distance3d",
                args.to_vec(),
                TypeFact::F64,
            ))
        }
        "math::pow" => {
            expect_len(args, 2)?;
            Some(StdlibFunctionFact::new(
                "math::pow",
                args.to_vec(),
                number_fact(),
            ))
        }
        "math::sqrt" => {
            expect_len(args, 1)?;
            Some(StdlibFunctionFact::new(
                "math::sqrt",
                args.to_vec(),
                TypeFact::F64,
            ))
        }
        "math::sign" | "math::floor" | "math::ceil" | "math::round" => {
            expect_len(args, 1)?;
            Some(StdlibFunctionFact::new(
                canonical_function_name(name)?,
                args.to_vec(),
                TypeFact::I64,
            ))
        }
        "math::abs" => {
            expect_len(args, 1)?;
            Some(StdlibFunctionFact::new(
                "math::abs",
                args.to_vec(),
                numeric_return(&args[0]),
            ))
        }
        "math::random" => {
            expect_len(args, 2)?;
            Some(StdlibFunctionFact::new(
                "math::random",
                args.to_vec(),
                TypeFact::I64,
            ))
        }
        "time::now" | "time::tick" => {
            expect_len(args, 0)?;
            Some(StdlibFunctionFact::new(
                canonical_function_name(name)?,
                Vec::new(),
                TypeFact::I64,
            ))
        }
        "time::elapsed_since" => {
            expect_len(args, 1)?;
            Some(StdlibFunctionFact::new(
                "time::elapsed_since",
                args.to_vec(),
                TypeFact::I64,
            ))
        }
        "io::print" | "io::println" => {
            expect_len(args, 1)?;
            Some(StdlibFunctionFact::new(
                canonical_function_name(name)?,
                args.to_vec(),
                TypeFact::result(TypeFact::NULL, TypeFact::record("IoError")),
            ))
        }
        "fs::read_to_string" => {
            expect_len(args, 1)?;
            Some(StdlibFunctionFact::new(
                "fs::read_to_string",
                args.to_vec(),
                TypeFact::result(TypeFact::STRING, TypeFact::record("IoError")),
            ))
        }
        "fs::write_string" => {
            expect_len(args, 2)?;
            Some(StdlibFunctionFact::new(
                "fs::write_string",
                args.to_vec(),
                TypeFact::result(TypeFact::NULL, TypeFact::record("IoError")),
            ))
        }
        "set::from_array" => {
            expect_len(args, 1)?;
            let TypeFact::Array { element } = &args[0] else {
                return Some(StdlibFunctionFact::new(
                    "set::from_array",
                    args.to_vec(),
                    TypeFact::set(TypeFact::Any),
                ));
            };
            Some(StdlibFunctionFact::new(
                "set::from_array",
                args.to_vec(),
                TypeFact::set((**element).clone()),
            ))
        }
        "bytes::from_hex" => {
            expect_len(args, 1)?;
            Some(StdlibFunctionFact::new(
                "bytes::from_hex",
                args.to_vec(),
                TypeFact::result(TypeFact::BYTES, TypeFact::STRING),
            ))
        }
        "i64::from_i32" | "u64::from_u32" => {
            expect_len(args, 1)?;
            Some(StdlibFunctionFact::new(
                canonical_function_name(name)?,
                args.to_vec(),
                TypeFact::I64,
            ))
        }
        "f64::from_f32" => {
            expect_len(args, 1)?;
            Some(StdlibFunctionFact::new(
                "f64::from_f32",
                args.to_vec(),
                TypeFact::F64,
            ))
        }
        "i8::try_from_i64" | "u8::try_from_u64" => {
            expect_len(args, 1)?;
            Some(StdlibFunctionFact::new(
                canonical_function_name(name)?,
                args.to_vec(),
                TypeFact::result(TypeFact::I64, TypeFact::STRING),
            ))
        }
        "f32::try_from_f64" => {
            expect_len(args, 1)?;
            Some(StdlibFunctionFact::new(
                "f32::try_from_f64",
                args.to_vec(),
                TypeFact::result(TypeFact::F64, TypeFact::STRING),
            ))
        }
        "u8::wrapping_add" | "u32::wrapping_mul" | "i8::wrapping_add" | "u8::bit_and"
        | "u8::bit_or" | "u8::bit_xor" | "u8::shift_left" | "u8::shift_right"
        | "u8::rotate_left" | "u8::rotate_right" => {
            expect_len(args, 2)?;
            Some(StdlibFunctionFact::new(
                canonical_function_name(name)?,
                args.to_vec(),
                TypeFact::I64,
            ))
        }
        _ => None,
    }
}

fn expect_len(args: &[TypeFact], expected: usize) -> Option<()> {
    (args.len() == expected).then_some(())
}

fn option_payload(value: &TypeFact) -> Option<TypeFact> {
    match value {
        TypeFact::Option { some } | TypeFact::OptionSome { some } => Some((**some).clone()),
        TypeFact::OptionNone => Some(TypeFact::Never),
        _ => None,
    }
}

fn option_unwrap_or_return(value: &TypeFact, fallback: TypeFact) -> TypeFact {
    match value {
        TypeFact::OptionSome { some } => (**some).clone(),
        TypeFact::OptionNone => fallback,
        _ => value_or_fallback(option_payload(value).unwrap_or(TypeFact::Any), fallback),
    }
}

fn option_ok_or_return(value: &TypeFact, err: TypeFact) -> TypeFact {
    match value {
        TypeFact::OptionSome { some } => TypeFact::result_ok((**some).clone()),
        TypeFact::OptionNone => TypeFact::result_err(err),
        _ => TypeFact::result(option_payload(value).unwrap_or(TypeFact::Any), err),
    }
}

fn option_flatten_return(value: &TypeFact) -> Option<TypeFact> {
    match value {
        TypeFact::OptionSome { some } => option_like_return(some),
        TypeFact::Option { some } => option_maybe_flatten_return(some),
        TypeFact::OptionNone => Some(TypeFact::option_none()),
        TypeFact::Any | TypeFact::Unknown => Some(TypeFact::option(TypeFact::Any)),
        _ => None,
    }
}

fn option_like_return(value: &TypeFact) -> Option<TypeFact> {
    match value {
        TypeFact::Option { .. } | TypeFact::OptionSome { .. } | TypeFact::OptionNone => {
            Some(value.clone())
        }
        TypeFact::Any | TypeFact::Unknown => Some(TypeFact::option(TypeFact::Any)),
        _ => None,
    }
}

fn option_maybe_flatten_return(value: &TypeFact) -> Option<TypeFact> {
    match value {
        TypeFact::Option { some } | TypeFact::OptionSome { some } => {
            Some(TypeFact::option((**some).clone()))
        }
        TypeFact::OptionNone => Some(TypeFact::option_none()),
        TypeFact::Any | TypeFact::Unknown => Some(TypeFact::option(TypeFact::Any)),
        _ => None,
    }
}

fn result_ok_payload(value: &TypeFact) -> Option<TypeFact> {
    match value {
        TypeFact::Result { ok, .. } | TypeFact::ResultOk { ok } => Some((**ok).clone()),
        TypeFact::ResultErr { .. } => Some(TypeFact::Never),
        _ => None,
    }
}

fn result_err_payload(value: &TypeFact) -> Option<TypeFact> {
    match value {
        TypeFact::Result { err, .. } | TypeFact::ResultErr { err } => Some((**err).clone()),
        TypeFact::ResultOk { .. } => Some(TypeFact::Never),
        _ => None,
    }
}

fn result_unwrap_or_return(value: &TypeFact, fallback: TypeFact) -> TypeFact {
    match value {
        TypeFact::ResultOk { ok } => (**ok).clone(),
        TypeFact::ResultErr { .. } => fallback,
        _ => value_or_fallback(result_ok_payload(value).unwrap_or(TypeFact::Any), fallback),
    }
}

fn result_to_option_return(value: &TypeFact) -> TypeFact {
    match value {
        TypeFact::ResultOk { ok } => TypeFact::option_some((**ok).clone()),
        TypeFact::ResultErr { .. } => TypeFact::option_none(),
        _ => TypeFact::option(result_ok_payload(value).unwrap_or(TypeFact::Any)),
    }
}

fn result_to_error_option_return(value: &TypeFact) -> TypeFact {
    match value {
        TypeFact::ResultOk { .. } => TypeFact::option_none(),
        TypeFact::ResultErr { err } => TypeFact::option_some((**err).clone()),
        _ => TypeFact::option(result_err_payload(value).unwrap_or(TypeFact::Any)),
    }
}

fn result_flatten_return(value: &TypeFact) -> Option<TypeFact> {
    match value {
        TypeFact::ResultOk { ok } => result_like_return(ok),
        TypeFact::Result { ok, err } => result_maybe_flatten_return(ok, err),
        TypeFact::ResultErr { err } => Some(TypeFact::result_err((**err).clone())),
        TypeFact::Any | TypeFact::Unknown => Some(TypeFact::result(TypeFact::Any, TypeFact::Any)),
        _ => None,
    }
}

fn result_like_return(value: &TypeFact) -> Option<TypeFact> {
    match value {
        TypeFact::Result { .. } | TypeFact::ResultOk { .. } | TypeFact::ResultErr { .. } => {
            Some(value.clone())
        }
        TypeFact::Any | TypeFact::Unknown => Some(TypeFact::result(TypeFact::Any, TypeFact::Any)),
        _ => None,
    }
}

fn result_maybe_flatten_return(ok: &TypeFact, err: &TypeFact) -> Option<TypeFact> {
    match ok {
        TypeFact::Result {
            ok: inner_ok,
            err: inner_err,
        } => Some(TypeFact::result(
            (**inner_ok).clone(),
            TypeFact::union([err.clone(), (**inner_err).clone()]),
        )),
        TypeFact::ResultOk { ok: inner_ok } => {
            Some(TypeFact::result((**inner_ok).clone(), err.clone()))
        }
        TypeFact::ResultErr { err: inner_err } => Some(TypeFact::result_err(TypeFact::union([
            err.clone(),
            (**inner_err).clone(),
        ]))),
        TypeFact::Any | TypeFact::Unknown => Some(TypeFact::result(TypeFact::Any, TypeFact::Any)),
        _ => None,
    }
}

fn value_or_fallback(value: TypeFact, fallback: TypeFact) -> TypeFact {
    if value == fallback {
        value
    } else {
        TypeFact::union([value, fallback])
    }
}

fn numeric_return(value: &TypeFact) -> TypeFact {
    match value {
        TypeFact::Primitive(PrimitiveTag::F64) => TypeFact::F64,
        TypeFact::Primitive(PrimitiveTag::I64) => TypeFact::I64,
        _ => TypeFact::Union(vec![TypeFact::I64, TypeFact::F64]),
    }
}

fn numeric_result(values: &[TypeFact]) -> TypeFact {
    if values
        .iter()
        .all(|value| matches!(value, TypeFact::Primitive(PrimitiveTag::I64)))
    {
        TypeFact::I64
    } else if values.iter().all(|value| {
        matches!(
            value,
            TypeFact::Primitive(PrimitiveTag::I64 | PrimitiveTag::F64)
        )
    }) {
        TypeFact::F64
    } else {
        TypeFact::Union(vec![TypeFact::I64, TypeFact::F64])
    }
}

fn number_fact() -> TypeFact {
    TypeFact::Union(vec![TypeFact::I64, TypeFact::F64])
}

fn canonical_function_name(name: &str) -> Option<&'static str> {
    match name {
        "option::is_some" => Some("option::is_some"),
        "option::is_none" => Some("option::is_none"),
        "result::is_ok" => Some("result::is_ok"),
        "result::is_err" => Some("result::is_err"),
        "math::max" => Some("math::max"),
        "math::min" => Some("math::min"),
        "math::floor" => Some("math::floor"),
        "math::ceil" => Some("math::ceil"),
        "math::round" => Some("math::round"),
        "math::move_towards" => Some("math::move_towards"),
        "math::distance2d" => Some("math::distance2d"),
        "math::distance3d" => Some("math::distance3d"),
        "math::pow" => Some("math::pow"),
        "math::sqrt" => Some("math::sqrt"),
        "math::sign" => Some("math::sign"),
        "time::now" => Some("time::now"),
        "time::tick" => Some("time::tick"),
        "time::elapsed_since" => Some("time::elapsed_since"),
        "io::print" => Some("io::print"),
        "io::println" => Some("io::println"),
        "i64::from_i32" => Some("i64::from_i32"),
        "u64::from_u32" => Some("u64::from_u32"),
        "i8::try_from_i64" => Some("i8::try_from_i64"),
        "u8::try_from_u64" => Some("u8::try_from_u64"),
        "u8::wrapping_add" => Some("u8::wrapping_add"),
        "u32::wrapping_mul" => Some("u32::wrapping_mul"),
        "i8::wrapping_add" => Some("i8::wrapping_add"),
        "u8::bit_and" => Some("u8::bit_and"),
        "u8::bit_or" => Some("u8::bit_or"),
        "u8::bit_xor" => Some("u8::bit_xor"),
        "u8::shift_left" => Some("u8::shift_left"),
        "u8::shift_right" => Some("u8::shift_right"),
        "u8::rotate_left" => Some("u8::rotate_left"),
        "u8::rotate_right" => Some("u8::rotate_right"),
        _ => None,
    }
}
