use crate::TypeFact;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LambdaFact {
    pub params: Vec<TypeFact>,
    pub returns: TypeFact,
}

impl LambdaFact {
    fn new(params: Vec<TypeFact>, returns: TypeFact) -> Self {
        Self { params, returns }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StdlibMethodFact {
    pub receiver: TypeFact,
    pub method: &'static str,
    pub lambda: Option<LambdaFact>,
    pub returns: TypeFact,
}

impl StdlibMethodFact {
    fn new(receiver: TypeFact, method: &'static str, returns: TypeFact) -> Self {
        Self {
            receiver,
            method,
            lambda: None,
            returns,
        }
    }

    fn with_lambda(mut self, params: Vec<TypeFact>, returns: TypeFact) -> Self {
        self.lambda = Some(LambdaFact::new(params, returns));
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StdlibFunctionFact {
    pub name: &'static str,
    pub params: Vec<TypeFact>,
    pub returns: TypeFact,
}

impl StdlibFunctionFact {
    fn new(name: &'static str, params: Vec<TypeFact>, returns: TypeFact) -> Self {
        Self {
            name,
            params,
            returns,
        }
    }
}

pub fn stdlib_method_fact(
    receiver: &TypeFact,
    method: &str,
    lambda_return: Option<&TypeFact>,
) -> Option<StdlibMethodFact> {
    match receiver {
        TypeFact::Array { element } => {
            array_method_fact((**element).clone(), method, lambda_return)
        }
        TypeFact::Map { key, value } => {
            map_method_fact((**key).clone(), (**value).clone(), method, lambda_return)
        }
        TypeFact::Set { element } => set_method_fact((**element).clone(), method),
        TypeFact::String => string_method_fact(method),
        _ => None,
    }
}

pub fn stdlib_function_fact(name: &str, args: &[TypeFact]) -> Option<StdlibFunctionFact> {
    match name {
        "option.some" => {
            expect_len(args, 1)?;
            Some(StdlibFunctionFact::new(
                "option.some",
                args.to_vec(),
                TypeFact::option(args[0].clone()),
            ))
        }
        "option.none" => {
            expect_len(args, 0)?;
            Some(StdlibFunctionFact::new(
                "option.none",
                Vec::new(),
                TypeFact::option(TypeFact::Any),
            ))
        }
        "option.is_some" | "option.is_none" => {
            expect_len(args, 1)?;
            Some(StdlibFunctionFact::new(
                canonical_function_name(name)?,
                args.to_vec(),
                TypeFact::Bool,
            ))
        }
        "option.unwrap_or" => {
            expect_len(args, 2)?;
            let unwrapped = option_payload(&args[0]).unwrap_or(TypeFact::Any);
            Some(StdlibFunctionFact::new(
                "option.unwrap_or",
                args.to_vec(),
                value_or_fallback(unwrapped, args[1].clone()),
            ))
        }
        "result.ok" => {
            expect_len(args, 1)?;
            Some(StdlibFunctionFact::new(
                "result.ok",
                args.to_vec(),
                TypeFact::result(args[0].clone(), TypeFact::Any),
            ))
        }
        "result.err" => {
            expect_len(args, 1)?;
            Some(StdlibFunctionFact::new(
                "result.err",
                args.to_vec(),
                TypeFact::result(TypeFact::Any, args[0].clone()),
            ))
        }
        "result.is_ok" | "result.is_err" => {
            expect_len(args, 1)?;
            Some(StdlibFunctionFact::new(
                canonical_function_name(name)?,
                args.to_vec(),
                TypeFact::Bool,
            ))
        }
        "result.unwrap_or" => {
            expect_len(args, 2)?;
            let unwrapped = result_ok_payload(&args[0]).unwrap_or(TypeFact::Any);
            Some(StdlibFunctionFact::new(
                "result.unwrap_or",
                args.to_vec(),
                value_or_fallback(unwrapped, args[1].clone()),
            ))
        }
        "math.max" | "math.min" => {
            expect_len(args, 2)?;
            Some(StdlibFunctionFact::new(
                canonical_function_name(name)?,
                args.to_vec(),
                numeric_result(args),
            ))
        }
        "math.clamp" => {
            expect_len(args, 3)?;
            Some(StdlibFunctionFact::new(
                "math.clamp",
                args.to_vec(),
                numeric_result(args),
            ))
        }
        "math.floor" | "math.ceil" => {
            expect_len(args, 1)?;
            Some(StdlibFunctionFact::new(
                canonical_function_name(name)?,
                args.to_vec(),
                TypeFact::Int,
            ))
        }
        "math.abs" => {
            expect_len(args, 1)?;
            Some(StdlibFunctionFact::new(
                "math.abs",
                args.to_vec(),
                numeric_return(&args[0]),
            ))
        }
        "math.random" => {
            expect_len(args, 2)?;
            Some(StdlibFunctionFact::new(
                "math.random",
                args.to_vec(),
                TypeFact::Int,
            ))
        }
        "set.from_array" => {
            expect_len(args, 1)?;
            let TypeFact::Array { element } = &args[0] else {
                return Some(StdlibFunctionFact::new(
                    "set.from_array",
                    args.to_vec(),
                    TypeFact::set(TypeFact::Any),
                ));
            };
            Some(StdlibFunctionFact::new(
                "set.from_array",
                args.to_vec(),
                TypeFact::set((**element).clone()),
            ))
        }
        _ => None,
    }
}

fn array_method_fact(
    element: TypeFact,
    method: &str,
    lambda_return: Option<&TypeFact>,
) -> Option<StdlibMethodFact> {
    let receiver = TypeFact::array(element.clone());
    match method {
        "len" => Some(StdlibMethodFact::new(receiver, "len", TypeFact::Int)),
        "is_empty" => Some(StdlibMethodFact::new(receiver, "is_empty", TypeFact::Bool)),
        "push" => Some(StdlibMethodFact::new(receiver, "push", TypeFact::Null)),
        "pop" => Some(StdlibMethodFact::new(receiver, "pop", element)),
        "map" => {
            let mapped = lambda_return.cloned().unwrap_or(TypeFact::Any);
            Some(
                StdlibMethodFact::new(receiver, "map", TypeFact::array(mapped.clone()))
                    .with_lambda(vec![element], mapped),
            )
        }
        "filter" => Some(
            StdlibMethodFact::new(receiver, "filter", TypeFact::array(element.clone()))
                .with_lambda(vec![element], TypeFact::Bool),
        ),
        "find" => Some(
            StdlibMethodFact::new(receiver, "find", TypeFact::option(element.clone()))
                .with_lambda(vec![element], TypeFact::Bool),
        ),
        "any" => Some(
            StdlibMethodFact::new(receiver, "any", TypeFact::Bool)
                .with_lambda(vec![element], TypeFact::Bool),
        ),
        "all" => Some(
            StdlibMethodFact::new(receiver, "all", TypeFact::Bool)
                .with_lambda(vec![element], TypeFact::Bool),
        ),
        "count" => Some(
            StdlibMethodFact::new(receiver, "count", TypeFact::Int)
                .with_lambda(vec![element], TypeFact::Bool),
        ),
        "sum" => {
            let returns = lambda_return.cloned().unwrap_or(element.clone());
            Some(
                StdlibMethodFact::new(receiver, "sum", numeric_return(&returns))
                    .with_lambda(vec![element], returns),
            )
        }
        "group_by" => Some(
            StdlibMethodFact::new(
                receiver,
                "group_by",
                TypeFact::map(TypeFact::String, TypeFact::array(element.clone())),
            )
            .with_lambda(vec![element], TypeFact::String),
        ),
        "sort_by" => Some(
            StdlibMethodFact::new(receiver, "sort_by", TypeFact::array(element.clone()))
                .with_lambda(vec![element], TypeFact::Any),
        ),
        _ => None,
    }
}

fn map_method_fact(
    key: TypeFact,
    value: TypeFact,
    method: &str,
    lambda_return: Option<&TypeFact>,
) -> Option<StdlibMethodFact> {
    let receiver = TypeFact::map(key.clone(), value.clone());
    match method {
        "len" => Some(StdlibMethodFact::new(receiver, "len", TypeFact::Int)),
        "is_empty" => Some(StdlibMethodFact::new(receiver, "is_empty", TypeFact::Bool)),
        "has" => Some(StdlibMethodFact::new(receiver, "has", TypeFact::Bool)),
        "get" => Some(StdlibMethodFact::new(
            receiver,
            "get",
            TypeFact::option(value.clone()),
        )),
        "get_or" => Some(StdlibMethodFact::new(receiver, "get_or", value.clone())),
        "set" => Some(StdlibMethodFact::new(receiver, "set", value.clone())),
        "remove" => Some(StdlibMethodFact::new(
            receiver,
            "remove",
            TypeFact::option(value.clone()),
        )),
        "keys" => Some(StdlibMethodFact::new(
            receiver,
            "keys",
            TypeFact::array(key.clone()),
        )),
        "values" => Some(StdlibMethodFact::new(
            receiver,
            "values",
            TypeFact::array(value.clone()),
        )),
        "entries" => Some(StdlibMethodFact::new(
            receiver,
            "entries",
            TypeFact::array(TypeFact::record("MapEntry")),
        )),
        "map_values" => {
            let mapped = lambda_return.cloned().unwrap_or(TypeFact::Any);
            Some(
                StdlibMethodFact::new(receiver, "map_values", TypeFact::map(key, mapped.clone()))
                    .with_lambda(vec![value], mapped),
            )
        }
        "filter" => Some(
            StdlibMethodFact::new(
                receiver,
                "filter",
                TypeFact::map(key.clone(), value.clone()),
            )
            .with_lambda(vec![key, value], TypeFact::Bool),
        ),
        "any" => Some(
            StdlibMethodFact::new(receiver, "any", TypeFact::Bool)
                .with_lambda(vec![value], TypeFact::Bool),
        ),
        "all" => Some(
            StdlibMethodFact::new(receiver, "all", TypeFact::Bool)
                .with_lambda(vec![value], TypeFact::Bool),
        ),
        "count" => Some(
            StdlibMethodFact::new(receiver, "count", TypeFact::Int)
                .with_lambda(vec![value], TypeFact::Bool),
        ),
        _ => None,
    }
}

fn set_method_fact(element: TypeFact, method: &str) -> Option<StdlibMethodFact> {
    let receiver = TypeFact::set(element.clone());
    match method {
        "len" => Some(StdlibMethodFact::new(receiver, "len", TypeFact::Int)),
        "is_empty" => Some(StdlibMethodFact::new(receiver, "is_empty", TypeFact::Bool)),
        "has" => Some(StdlibMethodFact::new(receiver, "has", TypeFact::Bool)),
        "add" => Some(StdlibMethodFact::new(receiver, "add", TypeFact::Bool)),
        "remove" => Some(StdlibMethodFact::new(receiver, "remove", TypeFact::Bool)),
        "values" => Some(StdlibMethodFact::new(
            receiver,
            "values",
            TypeFact::array(element),
        )),
        _ => None,
    }
}

fn string_method_fact(method: &str) -> Option<StdlibMethodFact> {
    let receiver = TypeFact::String;
    match method {
        "len" => Some(StdlibMethodFact::new(receiver, "len", TypeFact::Int)),
        "is_empty" => Some(StdlibMethodFact::new(receiver, "is_empty", TypeFact::Bool)),
        "contains" => Some(StdlibMethodFact::new(receiver, "contains", TypeFact::Bool)),
        "starts_with" => Some(StdlibMethodFact::new(
            receiver,
            "starts_with",
            TypeFact::Bool,
        )),
        "ends_with" => Some(StdlibMethodFact::new(receiver, "ends_with", TypeFact::Bool)),
        "to_upper" => Some(StdlibMethodFact::new(
            receiver,
            "to_upper",
            TypeFact::String,
        )),
        "to_lower" => Some(StdlibMethodFact::new(
            receiver,
            "to_lower",
            TypeFact::String,
        )),
        "trim" => Some(StdlibMethodFact::new(receiver, "trim", TypeFact::String)),
        "split" => Some(StdlibMethodFact::new(
            receiver,
            "split",
            TypeFact::array(TypeFact::String),
        )),
        _ => None,
    }
}

fn numeric_return(value: &TypeFact) -> TypeFact {
    match value {
        TypeFact::Float => TypeFact::Float,
        TypeFact::Int => TypeFact::Int,
        _ => TypeFact::Union(vec![TypeFact::Int, TypeFact::Float]),
    }
}

fn canonical_function_name(name: &str) -> Option<&'static str> {
    match name {
        "option.is_some" => Some("option.is_some"),
        "option.is_none" => Some("option.is_none"),
        "result.is_ok" => Some("result.is_ok"),
        "result.is_err" => Some("result.is_err"),
        "math.max" => Some("math.max"),
        "math.min" => Some("math.min"),
        "math.floor" => Some("math.floor"),
        "math.ceil" => Some("math.ceil"),
        _ => None,
    }
}

fn expect_len(args: &[TypeFact], expected: usize) -> Option<()> {
    (args.len() == expected).then_some(())
}

fn option_payload(value: &TypeFact) -> Option<TypeFact> {
    let TypeFact::Option { some } = value else {
        return None;
    };
    Some((**some).clone())
}

fn result_ok_payload(value: &TypeFact) -> Option<TypeFact> {
    let TypeFact::Result { ok, .. } = value else {
        return None;
    };
    Some((**ok).clone())
}

fn value_or_fallback(value: TypeFact, fallback: TypeFact) -> TypeFact {
    if value == fallback {
        value
    } else {
        TypeFact::Union(vec![value, fallback])
    }
}

fn numeric_result(values: &[TypeFact]) -> TypeFact {
    if values.iter().all(|value| matches!(value, TypeFact::Int)) {
        TypeFact::Int
    } else if values
        .iter()
        .all(|value| matches!(value, TypeFact::Int | TypeFact::Float))
    {
        TypeFact::Float
    } else {
        TypeFact::Union(vec![TypeFact::Int, TypeFact::Float])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn array_lambda_methods_expose_element_parameter_facts() {
        let receiver = TypeFact::array(TypeFact::record("Reward"));

        let filter = stdlib_method_fact(&receiver, "filter", None).expect("filter fact");
        assert_eq!(
            filter.lambda.expect("filter lambda").params,
            vec![TypeFact::record("Reward")]
        );
        assert_eq!(filter.returns, receiver);

        let mapped =
            stdlib_method_fact(&receiver, "map", Some(&TypeFact::String)).expect("map fact");
        assert_eq!(mapped.returns, TypeFact::array(TypeFact::String));

        let found = stdlib_method_fact(&receiver, "find", None).expect("find fact");
        assert_eq!(found.returns, TypeFact::option(TypeFact::record("Reward")));
    }

    #[test]
    fn map_lambda_methods_expose_key_and_value_parameter_facts() {
        let receiver = TypeFact::map(TypeFact::String, TypeFact::Int);

        let filter = stdlib_method_fact(&receiver, "filter", None).expect("filter fact");
        assert_eq!(
            filter.lambda.expect("filter lambda").params,
            vec![TypeFact::String, TypeFact::Int]
        );
        assert_eq!(filter.returns, receiver);

        let mapped =
            stdlib_method_fact(&receiver, "map_values", Some(&TypeFact::Bool)).expect("map fact");
        assert_eq!(
            mapped.returns,
            TypeFact::map(TypeFact::String, TypeFact::Bool)
        );
        assert_eq!(
            mapped.lambda.expect("map_values lambda").params,
            vec![TypeFact::Int]
        );

        let any = stdlib_method_fact(&receiver, "any", None).expect("any fact");
        assert_eq!(any.returns, TypeFact::Bool);
        assert_eq!(any.lambda.expect("any lambda").params, vec![TypeFact::Int]);

        let all = stdlib_method_fact(&receiver, "all", None).expect("all fact");
        assert_eq!(all.returns, TypeFact::Bool);
        assert_eq!(all.lambda.expect("all lambda").params, vec![TypeFact::Int]);

        let count = stdlib_method_fact(&receiver, "count", None).expect("count fact");
        assert_eq!(count.returns, TypeFact::Int);
        assert_eq!(
            count.lambda.expect("count lambda").params,
            vec![TypeFact::Int]
        );
    }

    #[test]
    fn scalar_collection_methods_return_non_generic_facts() {
        let map = TypeFact::map(TypeFact::String, TypeFact::Int);
        let array = TypeFact::array(TypeFact::Float);
        let set = TypeFact::set(TypeFact::String);

        assert_eq!(
            stdlib_method_fact(&map, "keys", None)
                .expect("keys fact")
                .returns,
            TypeFact::array(TypeFact::String)
        );
        assert_eq!(
            stdlib_method_fact(&array, "sum", None)
                .expect("sum fact")
                .returns,
            TypeFact::Float
        );
        assert_eq!(
            stdlib_method_fact(&set, "values", None)
                .expect("values fact")
                .returns,
            TypeFact::array(TypeFact::String)
        );
    }

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

        let ok = stdlib_function_fact("result.ok", &[TypeFact::Int]).expect("ok fact");
        assert_eq!(ok.returns, TypeFact::result(TypeFact::Int, TypeFact::Any));

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
    }

    #[test]
    fn math_and_set_functions_expose_return_facts() {
        assert_eq!(
            stdlib_function_fact("math.max", &[TypeFact::Int, TypeFact::Int])
                .expect("max fact")
                .returns,
            TypeFact::Int
        );
        assert_eq!(
            stdlib_function_fact(
                "math.clamp",
                &[TypeFact::Float, TypeFact::Int, TypeFact::Float],
            )
            .expect("clamp fact")
            .returns,
            TypeFact::Float
        );
        assert_eq!(
            stdlib_function_fact("math.floor", &[TypeFact::Float])
                .expect("floor fact")
                .returns,
            TypeFact::Int
        );
        assert_eq!(
            stdlib_function_fact("set.from_array", &[TypeFact::array(TypeFact::String)])
                .expect("set.from_array fact")
                .returns,
            TypeFact::set(TypeFact::String)
        );
    }

    #[test]
    fn stdlib_function_facts_reject_unknown_names_and_wrong_arity() {
        assert!(stdlib_function_fact("option.some", &[]).is_none());
        assert!(stdlib_function_fact("game.spawn", &[TypeFact::String]).is_none());
    }
}
