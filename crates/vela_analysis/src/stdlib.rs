use crate::TypeFact;

const ARRAY_METHOD_NAMES: &[&str] = &[
    "len", "is_empty", "push", "pop", "first", "last", "join", "contains", "map", "filter", "find",
    "any", "all", "count", "sum", "group_by", "sort_by",
];
const MAP_METHOD_NAMES: &[&str] = &[
    "len",
    "is_empty",
    "has",
    "get",
    "get_or",
    "set",
    "remove",
    "keys",
    "values",
    "entries",
    "map_values",
    "filter",
    "any",
    "all",
    "count",
];
const SET_METHOD_NAMES: &[&str] = &[
    "len",
    "is_empty",
    "has",
    "add",
    "remove",
    "values",
    "union",
    "intersection",
    "difference",
    "is_subset",
    "is_superset",
    "is_disjoint",
];
const STRING_METHOD_NAMES: &[&str] = &[
    "len",
    "is_empty",
    "contains",
    "find",
    "starts_with",
    "ends_with",
    "strip_prefix",
    "strip_suffix",
    "to_upper",
    "to_lower",
    "trim",
    "trim_start",
    "trim_end",
    "replace",
    "slice",
    "split",
];

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
    pub params: Vec<TypeFact>,
    pub lambda: Option<LambdaFact>,
    pub returns: TypeFact,
}

impl StdlibMethodFact {
    fn new(receiver: TypeFact, method: &'static str, returns: TypeFact) -> Self {
        Self {
            receiver,
            method,
            params: Vec::new(),
            lambda: None,
            returns,
        }
    }

    fn with_params(mut self, params: Vec<TypeFact>) -> Self {
        self.params = params;
        self
    }

    fn with_lambda(mut self, params: Vec<TypeFact>, returns: TypeFact) -> Self {
        self.params = vec![TypeFact::function(params.clone(), returns.clone())];
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

pub fn stdlib_method_facts(
    receiver: &TypeFact,
    lambda_return: Option<&TypeFact>,
) -> Vec<StdlibMethodFact> {
    stdlib_method_names(receiver)
        .iter()
        .filter_map(|method| stdlib_method_fact(receiver, method, lambda_return))
        .collect()
}

pub fn stdlib_function_completion_facts() -> Vec<StdlibFunctionFact> {
    let number = number_fact();
    vec![
        StdlibFunctionFact::new(
            "option.some",
            vec![TypeFact::Any],
            TypeFact::option(TypeFact::Any),
        ),
        StdlibFunctionFact::new("option.none", Vec::new(), TypeFact::option(TypeFact::Any)),
        StdlibFunctionFact::new(
            "option.is_some",
            vec![TypeFact::option(TypeFact::Any)],
            TypeFact::Bool,
        ),
        StdlibFunctionFact::new(
            "option.is_none",
            vec![TypeFact::option(TypeFact::Any)],
            TypeFact::Bool,
        ),
        StdlibFunctionFact::new(
            "option.unwrap_or",
            vec![TypeFact::option(TypeFact::Any), TypeFact::Any],
            TypeFact::Any,
        ),
        StdlibFunctionFact::new(
            "result.ok",
            vec![TypeFact::Any],
            TypeFact::result(TypeFact::Any, TypeFact::Any),
        ),
        StdlibFunctionFact::new(
            "result.err",
            vec![TypeFact::Any],
            TypeFact::result(TypeFact::Any, TypeFact::Any),
        ),
        StdlibFunctionFact::new(
            "result.is_ok",
            vec![TypeFact::result(TypeFact::Any, TypeFact::Any)],
            TypeFact::Bool,
        ),
        StdlibFunctionFact::new(
            "result.is_err",
            vec![TypeFact::result(TypeFact::Any, TypeFact::Any)],
            TypeFact::Bool,
        ),
        StdlibFunctionFact::new(
            "result.unwrap_or",
            vec![
                TypeFact::result(TypeFact::Any, TypeFact::Any),
                TypeFact::Any,
            ],
            TypeFact::Any,
        ),
        StdlibFunctionFact::new(
            "math.max",
            vec![number.clone(), number.clone()],
            number.clone(),
        ),
        StdlibFunctionFact::new(
            "math.min",
            vec![number.clone(), number.clone()],
            number.clone(),
        ),
        StdlibFunctionFact::new(
            "math.clamp",
            vec![number.clone(), number.clone(), number.clone()],
            number.clone(),
        ),
        StdlibFunctionFact::new(
            "math.lerp",
            vec![number.clone(), number.clone(), number.clone()],
            TypeFact::Float,
        ),
        StdlibFunctionFact::new("math.floor", vec![number.clone()], TypeFact::Int),
        StdlibFunctionFact::new("math.ceil", vec![number.clone()], TypeFact::Int),
        StdlibFunctionFact::new("math.round", vec![number.clone()], TypeFact::Int),
        StdlibFunctionFact::new("math.abs", vec![number.clone()], number),
        StdlibFunctionFact::new(
            "math.random",
            vec![TypeFact::Int, TypeFact::Int],
            TypeFact::Int,
        ),
        StdlibFunctionFact::new("ctx.now", Vec::new(), TypeFact::Int),
        StdlibFunctionFact::new("ctx.tick", Vec::new(), TypeFact::Int),
        StdlibFunctionFact::new(
            "set.from_array",
            vec![TypeFact::array(TypeFact::Any)],
            TypeFact::set(TypeFact::Any),
        ),
    ]
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
            Some(StdlibFunctionFact::new(
                "option.unwrap_or",
                args.to_vec(),
                option_unwrap_or_return(&args[0], args[1].clone()),
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
            Some(StdlibFunctionFact::new(
                "result.unwrap_or",
                args.to_vec(),
                result_unwrap_or_return(&args[0], args[1].clone()),
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
        "math.lerp" => {
            expect_len(args, 3)?;
            Some(StdlibFunctionFact::new(
                "math.lerp",
                args.to_vec(),
                TypeFact::Float,
            ))
        }
        "math.floor" | "math.ceil" | "math.round" => {
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
        "ctx.now" | "ctx.tick" => {
            expect_len(args, 0)?;
            Some(StdlibFunctionFact::new(
                canonical_function_name(name)?,
                Vec::new(),
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

fn stdlib_method_names(receiver: &TypeFact) -> &'static [&'static str] {
    match receiver {
        TypeFact::Array { .. } => ARRAY_METHOD_NAMES,
        TypeFact::Map { .. } => MAP_METHOD_NAMES,
        TypeFact::Set { .. } => SET_METHOD_NAMES,
        TypeFact::String => STRING_METHOD_NAMES,
        _ => &[],
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
        "push" => Some(
            StdlibMethodFact::new(receiver, "push", TypeFact::Null)
                .with_params(vec![element.clone()]),
        ),
        "pop" => Some(StdlibMethodFact::new(
            receiver,
            "pop",
            TypeFact::option(element.clone()),
        )),
        "first" => Some(StdlibMethodFact::new(
            receiver,
            "first",
            TypeFact::option(element.clone()),
        )),
        "last" => Some(StdlibMethodFact::new(
            receiver,
            "last",
            TypeFact::option(element),
        )),
        "join" => Some(
            StdlibMethodFact::new(receiver, "join", TypeFact::String)
                .with_params(vec![TypeFact::String]),
        ),
        "contains" => Some(
            StdlibMethodFact::new(receiver, "contains", TypeFact::Bool)
                .with_params(vec![element.clone()]),
        ),
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
        "has" => Some(
            StdlibMethodFact::new(receiver, "has", TypeFact::Bool).with_params(vec![key.clone()]),
        ),
        "get" => Some(
            StdlibMethodFact::new(receiver, "get", TypeFact::option(value.clone()))
                .with_params(vec![key.clone()]),
        ),
        "get_or" => Some(
            StdlibMethodFact::new(receiver, "get_or", value.clone())
                .with_params(vec![key.clone(), value.clone()]),
        ),
        "set" => Some(
            StdlibMethodFact::new(receiver, "set", value.clone())
                .with_params(vec![key.clone(), value.clone()]),
        ),
        "remove" => Some(
            StdlibMethodFact::new(receiver, "remove", TypeFact::option(value.clone()))
                .with_params(vec![key.clone()]),
        ),
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
                .with_lambda(vec![key.clone(), value.clone()], TypeFact::Bool),
        ),
        "all" => Some(
            StdlibMethodFact::new(receiver, "all", TypeFact::Bool)
                .with_lambda(vec![key.clone(), value.clone()], TypeFact::Bool),
        ),
        "count" => Some(
            StdlibMethodFact::new(receiver, "count", TypeFact::Int)
                .with_lambda(vec![key, value], TypeFact::Bool),
        ),
        _ => None,
    }
}

fn set_method_fact(element: TypeFact, method: &str) -> Option<StdlibMethodFact> {
    let receiver = TypeFact::set(element.clone());
    match method {
        "len" => Some(StdlibMethodFact::new(receiver, "len", TypeFact::Int)),
        "is_empty" => Some(StdlibMethodFact::new(receiver, "is_empty", TypeFact::Bool)),
        "has" => Some(
            StdlibMethodFact::new(receiver, "has", TypeFact::Bool)
                .with_params(vec![element.clone()]),
        ),
        "add" => Some(
            StdlibMethodFact::new(receiver, "add", TypeFact::Bool)
                .with_params(vec![element.clone()]),
        ),
        "remove" => Some(
            StdlibMethodFact::new(receiver, "remove", TypeFact::Bool)
                .with_params(vec![element.clone()]),
        ),
        "values" => Some(StdlibMethodFact::new(
            receiver,
            "values",
            TypeFact::array(element.clone()),
        )),
        "union" | "intersection" | "difference" => Some(
            StdlibMethodFact::new(
                receiver,
                match method {
                    "union" => "union",
                    "intersection" => "intersection",
                    _ => "difference",
                },
                TypeFact::set(element.clone()),
            )
            .with_params(vec![TypeFact::set(element)]),
        ),
        "is_subset" | "is_superset" | "is_disjoint" => Some(
            StdlibMethodFact::new(
                receiver,
                match method {
                    "is_subset" => "is_subset",
                    "is_superset" => "is_superset",
                    _ => "is_disjoint",
                },
                TypeFact::Bool,
            )
            .with_params(vec![TypeFact::set(element)]),
        ),
        _ => None,
    }
}

fn string_method_fact(method: &str) -> Option<StdlibMethodFact> {
    let receiver = TypeFact::String;
    match method {
        "len" => Some(StdlibMethodFact::new(receiver, "len", TypeFact::Int)),
        "is_empty" => Some(StdlibMethodFact::new(receiver, "is_empty", TypeFact::Bool)),
        "contains" => Some(
            StdlibMethodFact::new(receiver, "contains", TypeFact::Bool)
                .with_params(vec![TypeFact::String]),
        ),
        "find" => Some(
            StdlibMethodFact::new(receiver, "find", TypeFact::option(TypeFact::Int))
                .with_params(vec![TypeFact::String]),
        ),
        "starts_with" => Some(
            StdlibMethodFact::new(receiver, "starts_with", TypeFact::Bool)
                .with_params(vec![TypeFact::String]),
        ),
        "ends_with" => Some(
            StdlibMethodFact::new(receiver, "ends_with", TypeFact::Bool)
                .with_params(vec![TypeFact::String]),
        ),
        "strip_prefix" => Some(
            StdlibMethodFact::new(receiver, "strip_prefix", TypeFact::option(TypeFact::String))
                .with_params(vec![TypeFact::String]),
        ),
        "strip_suffix" => Some(
            StdlibMethodFact::new(receiver, "strip_suffix", TypeFact::option(TypeFact::String))
                .with_params(vec![TypeFact::String]),
        ),
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
        "trim" | "trim_start" | "trim_end" => Some(StdlibMethodFact::new(
            receiver,
            match method {
                "trim_start" => "trim_start",
                "trim_end" => "trim_end",
                _ => "trim",
            },
            TypeFact::String,
        )),
        "replace" => Some(
            StdlibMethodFact::new(receiver, "replace", TypeFact::String)
                .with_params(vec![TypeFact::String, TypeFact::String]),
        ),
        "slice" => Some(
            StdlibMethodFact::new(receiver, "slice", TypeFact::String)
                .with_params(vec![TypeFact::Int, TypeFact::Int]),
        ),
        "split" => Some(
            StdlibMethodFact::new(receiver, "split", TypeFact::array(TypeFact::String))
                .with_params(vec![TypeFact::String]),
        ),
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
        "math.round" => Some("math.round"),
        "ctx.now" => Some("ctx.now"),
        "ctx.tick" => Some("ctx.tick"),
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

fn result_ok_payload(value: &TypeFact) -> Option<TypeFact> {
    match value {
        TypeFact::Result { ok, .. } | TypeFact::ResultOk { ok } => Some((**ok).clone()),
        TypeFact::ResultErr { .. } => Some(TypeFact::Never),
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

fn value_or_fallback(value: TypeFact, fallback: TypeFact) -> TypeFact {
    if value == fallback {
        value
    } else {
        TypeFact::union([value, fallback])
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

fn number_fact() -> TypeFact {
    TypeFact::Union(vec![TypeFact::Int, TypeFact::Float])
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
        assert_eq!(
            any.lambda.expect("any lambda").params,
            vec![TypeFact::String, TypeFact::Int]
        );

        let all = stdlib_method_fact(&receiver, "all", None).expect("all fact");
        assert_eq!(all.returns, TypeFact::Bool);
        assert_eq!(
            all.lambda.expect("all lambda").params,
            vec![TypeFact::String, TypeFact::Int]
        );

        let count = stdlib_method_fact(&receiver, "count", None).expect("count fact");
        assert_eq!(count.returns, TypeFact::Int);
        assert_eq!(
            count.lambda.expect("count lambda").params,
            vec![TypeFact::String, TypeFact::Int]
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
            stdlib_method_fact(&array, "pop", None)
                .expect("pop fact")
                .returns,
            TypeFact::option(TypeFact::Float)
        );
        assert_eq!(
            stdlib_method_fact(&array, "first", None)
                .expect("first fact")
                .returns,
            TypeFact::option(TypeFact::Float)
        );
        assert_eq!(
            stdlib_method_fact(&array, "last", None)
                .expect("last fact")
                .returns,
            TypeFact::option(TypeFact::Float)
        );
        let join = stdlib_method_fact(&array, "join", None).expect("join fact");
        assert_eq!(join.params, vec![TypeFact::String]);
        assert_eq!(join.returns, TypeFact::String);
        let contains = stdlib_method_fact(&array, "contains", None).expect("contains fact");
        assert_eq!(contains.params, vec![TypeFact::Float]);
        assert_eq!(contains.returns, TypeFact::Bool);
        assert_eq!(
            stdlib_method_fact(&set, "values", None)
                .expect("values fact")
                .returns,
            TypeFact::array(TypeFact::String)
        );
        let union = stdlib_method_fact(&set, "union", None).expect("union fact");
        assert_eq!(union.params, vec![TypeFact::set(TypeFact::String)]);
        assert_eq!(union.returns, TypeFact::set(TypeFact::String));
        let intersection =
            stdlib_method_fact(&set, "intersection", None).expect("intersection fact");
        assert_eq!(intersection.params, vec![TypeFact::set(TypeFact::String)]);
        assert_eq!(intersection.returns, TypeFact::set(TypeFact::String));
        let difference = stdlib_method_fact(&set, "difference", None).expect("difference fact");
        assert_eq!(difference.params, vec![TypeFact::set(TypeFact::String)]);
        assert_eq!(difference.returns, TypeFact::set(TypeFact::String));
        let subset = stdlib_method_fact(&set, "is_subset", None).expect("is_subset fact");
        assert_eq!(subset.params, vec![TypeFact::set(TypeFact::String)]);
        assert_eq!(subset.returns, TypeFact::Bool);
        let superset = stdlib_method_fact(&set, "is_superset", None).expect("is_superset fact");
        assert_eq!(superset.params, vec![TypeFact::set(TypeFact::String)]);
        assert_eq!(superset.returns, TypeFact::Bool);
        let disjoint = stdlib_method_fact(&set, "is_disjoint", None).expect("is_disjoint fact");
        assert_eq!(disjoint.params, vec![TypeFact::set(TypeFact::String)]);
        assert_eq!(disjoint.returns, TypeFact::Bool);
    }

    #[test]
    fn string_methods_expose_replacement_and_split_facts() {
        let find = stdlib_method_fact(&TypeFact::String, "find", None).expect("find fact");
        assert_eq!(find.params, vec![TypeFact::String]);
        assert_eq!(find.returns, TypeFact::option(TypeFact::Int));

        let strip_prefix =
            stdlib_method_fact(&TypeFact::String, "strip_prefix", None).expect("prefix fact");
        assert_eq!(strip_prefix.params, vec![TypeFact::String]);
        assert_eq!(strip_prefix.returns, TypeFact::option(TypeFact::String));

        let strip_suffix =
            stdlib_method_fact(&TypeFact::String, "strip_suffix", None).expect("suffix fact");
        assert_eq!(strip_suffix.params, vec![TypeFact::String]);
        assert_eq!(strip_suffix.returns, TypeFact::option(TypeFact::String));

        let replace = stdlib_method_fact(&TypeFact::String, "replace", None).expect("replace fact");
        assert_eq!(replace.params, vec![TypeFact::String, TypeFact::String]);
        assert_eq!(replace.returns, TypeFact::String);

        let trim_start =
            stdlib_method_fact(&TypeFact::String, "trim_start", None).expect("trim_start fact");
        assert_eq!(trim_start.params, Vec::<TypeFact>::new());
        assert_eq!(trim_start.returns, TypeFact::String);

        let trim_end =
            stdlib_method_fact(&TypeFact::String, "trim_end", None).expect("trim_end fact");
        assert_eq!(trim_end.params, Vec::<TypeFact>::new());
        assert_eq!(trim_end.returns, TypeFact::String);

        let slice = stdlib_method_fact(&TypeFact::String, "slice", None).expect("slice fact");
        assert_eq!(slice.params, vec![TypeFact::Int, TypeFact::Int]);
        assert_eq!(slice.returns, TypeFact::String);

        let split = stdlib_method_fact(&TypeFact::String, "split", None).expect("split fact");
        assert_eq!(split.params, vec![TypeFact::String]);
        assert_eq!(split.returns, TypeFact::array(TypeFact::String));
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
        let none_unwrapped = stdlib_function_fact(
            "option.unwrap_or",
            &[TypeFact::option_none(), TypeFact::String],
        )
        .expect("none unwrap_or fact");
        assert_eq!(none_unwrapped.returns, TypeFact::String);

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
            stdlib_function_fact(
                "math.lerp",
                &[TypeFact::Int, TypeFact::Int, TypeFact::Float]
            )
            .expect("lerp fact")
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
            stdlib_function_fact("math.round", &[TypeFact::Float])
                .expect("round fact")
                .returns,
            TypeFact::Int
        );
        assert_eq!(
            stdlib_function_fact("set.from_array", &[TypeFact::array(TypeFact::String)])
                .expect("set.from_array fact")
                .returns,
            TypeFact::set(TypeFact::String)
        );
        assert_eq!(
            stdlib_function_fact("ctx.now", &[])
                .expect("ctx.now fact")
                .returns,
            TypeFact::Int
        );
        assert_eq!(
            stdlib_function_fact("ctx.tick", &[])
                .expect("ctx.tick fact")
                .returns,
            TypeFact::Int
        );
    }

    #[test]
    fn stdlib_function_facts_reject_unknown_names_and_wrong_arity() {
        assert!(stdlib_function_fact("option.some", &[]).is_none());
        assert!(stdlib_function_fact("game.spawn", &[TypeFact::String]).is_none());
    }

    #[test]
    fn stdlib_method_facts_enumerate_receiver_api_surface() {
        let map = TypeFact::map(TypeFact::String, TypeFact::Int);
        let facts = stdlib_method_facts(&map, Some(&TypeFact::Bool));

        assert!(facts.iter().any(|fact| {
            fact.method == "map_values"
                && fact.returns == TypeFact::map(TypeFact::String, TypeFact::Bool)
        }));
        assert!(facts.iter().any(|fact| {
            fact.method == "filter"
                && fact
                    .lambda
                    .as_ref()
                    .is_some_and(|lambda| lambda.params == vec![TypeFact::String, TypeFact::Int])
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

    #[test]
    fn stdlib_function_completion_facts_enumerate_global_api_surface() {
        let facts = stdlib_function_completion_facts();

        assert!(facts.iter().any(|fact| {
            fact.name == "option.unwrap_or"
                && fact.params == vec![TypeFact::option(TypeFact::Any), TypeFact::Any]
                && fact.returns == TypeFact::Any
        }));
        assert!(facts.iter().any(|fact| {
            fact.name == "math.clamp" && fact.params.len() == 3 && fact.returns == number_fact()
        }));
        assert!(facts.iter().any(|fact| {
            fact.name == "math.lerp" && fact.params.len() == 3 && fact.returns == TypeFact::Float
        }));
        assert!(facts.iter().any(|fact| {
            fact.name == "math.round" && fact.params.len() == 1 && fact.returns == TypeFact::Int
        }));
        assert!(facts.iter().any(|fact| {
            fact.name == "set.from_array" && fact.returns == TypeFact::set(TypeFact::Any)
        }));
        assert!(
            facts
                .iter()
                .any(|fact| fact.name == "ctx.now" && fact.returns == TypeFact::Int)
        );
        assert!(
            facts
                .iter()
                .any(|fact| fact.name == "ctx.tick" && fact.returns == TypeFact::Int)
        );
    }
}
