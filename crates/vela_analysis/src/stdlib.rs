use crate::TypeFact;

mod functions;
mod methods;

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
    methods::method_fact(receiver, method, lambda_return)
}

pub fn stdlib_method_facts(
    receiver: &TypeFact,
    lambda_return: Option<&TypeFact>,
) -> Vec<StdlibMethodFact> {
    methods::method_facts(receiver, lambda_return)
}

pub fn stdlib_function_completion_facts() -> Vec<StdlibFunctionFact> {
    functions::completion_facts()
}

pub fn stdlib_function_fact(name: &str, args: &[TypeFact]) -> Option<StdlibFunctionFact> {
    functions::function_fact(name, args)
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
            vec![TypeFact::String, TypeFact::Int]
        );

        let merged = stdlib_method_fact(&receiver, "merge", None).expect("merge fact");
        assert_eq!(
            merged.params,
            vec![TypeFact::map(TypeFact::String, TypeFact::Int)]
        );
        assert_eq!(merged.returns, receiver);

        let found = stdlib_method_fact(&receiver, "find", None).expect("find fact");
        assert_eq!(
            found.returns,
            TypeFact::option(TypeFact::record("MapEntry"))
        );
        assert_eq!(
            found.lambda.expect("find lambda").params,
            vec![TypeFact::String, TypeFact::Int]
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
            stdlib_method_fact(&array, "distinct", None)
                .expect("distinct fact")
                .returns,
            TypeFact::array(TypeFact::Float)
        );
        assert_eq!(
            stdlib_method_fact(&array, "reverse", None)
                .expect("reverse fact")
                .returns,
            TypeFact::array(TypeFact::Float)
        );
        let slice = stdlib_method_fact(&array, "slice", None).expect("slice fact");
        assert_eq!(slice.params, vec![TypeFact::Int, TypeFact::Int]);
        assert_eq!(slice.returns, TypeFact::array(TypeFact::Float));
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
        let symmetric_difference =
            stdlib_method_fact(&set, "symmetric_difference", None).expect("symmetric fact");
        assert_eq!(
            symmetric_difference.params,
            vec![TypeFact::set(TypeFact::String)]
        );
        assert_eq!(
            symmetric_difference.returns,
            TypeFact::set(TypeFact::String)
        );
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

        let repeat = stdlib_method_fact(&TypeFact::String, "repeat", None).expect("repeat fact");
        assert_eq!(repeat.params, vec![TypeFact::Int]);
        assert_eq!(repeat.returns, TypeFact::String);

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

        let parse_int =
            stdlib_method_fact(&TypeFact::String, "parse_int", None).expect("parse_int fact");
        assert_eq!(parse_int.params, Vec::<TypeFact>::new());
        assert_eq!(parse_int.returns, TypeFact::option(TypeFact::Int));

        let parse_float =
            stdlib_method_fact(&TypeFact::String, "parse_float", None).expect("parse_float fact");
        assert_eq!(parse_float.params, Vec::<TypeFact>::new());
        assert_eq!(parse_float.returns, TypeFact::option(TypeFact::Float));

        let parse_bool =
            stdlib_method_fact(&TypeFact::String, "parse_bool", None).expect("parse_bool fact");
        assert_eq!(parse_bool.params, Vec::<TypeFact>::new());
        assert_eq!(parse_bool.returns, TypeFact::option(TypeFact::Bool));
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
        assert!(
            stdlib_function_fact("option.flatten", &[TypeFact::option(TypeFact::Int)]).is_none()
        );

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

    #[test]
    fn option_and_result_map_methods_expose_dynamic_enum_facts() {
        let unwrapped_option =
            stdlib_method_fact(&TypeFact::option(TypeFact::Int), "unwrap_or", None)
                .expect("option unwrap_or fact");
        assert_eq!(
            unwrapped_option.returns,
            TypeFact::union([TypeFact::Int, TypeFact::Any])
        );
        assert_eq!(unwrapped_option.params, vec![TypeFact::Any]);

        let unwrapped_some =
            stdlib_method_fact(&TypeFact::option_some(TypeFact::Int), "unwrap_or", None)
                .expect("some unwrap_or fact");
        assert_eq!(unwrapped_some.returns, TypeFact::Int);

        let ok_or = stdlib_method_fact(&TypeFact::option(TypeFact::Int), "ok_or", None)
            .expect("option ok_or fact");
        assert_eq!(
            ok_or.returns,
            TypeFact::result(TypeFact::Int, TypeFact::Any)
        );
        assert_eq!(ok_or.params, vec![TypeFact::Any]);

        let none_ok_or =
            stdlib_method_fact(&TypeFact::option_none(), "ok_or", None).expect("none ok_or fact");
        assert_eq!(none_ok_or.returns, TypeFact::result_err(TypeFact::Any));

        let maybe = stdlib_method_fact(
            &TypeFact::option(TypeFact::Int),
            "map",
            Some(&TypeFact::String),
        )
        .expect("option map fact");
        assert_eq!(maybe.returns, TypeFact::option(TypeFact::String));
        assert_eq!(
            maybe.lambda.expect("option map lambda").params,
            vec![TypeFact::Int]
        );

        let some = stdlib_method_fact(
            &TypeFact::option_some(TypeFact::Int),
            "map",
            Some(&TypeFact::String),
        )
        .expect("some map fact");
        assert_eq!(some.returns, TypeFact::option_some(TypeFact::String));

        let none = stdlib_method_fact(&TypeFact::option_none(), "map", Some(&TypeFact::String))
            .expect("none map fact");
        assert_eq!(none.returns, TypeFact::option_none());

        let chained = stdlib_method_fact(
            &TypeFact::option(TypeFact::Int),
            "and_then",
            Some(&TypeFact::option(TypeFact::String)),
        )
        .expect("option and_then fact");
        assert_eq!(chained.returns, TypeFact::option(TypeFact::String));
        assert_eq!(
            chained.lambda.expect("option and_then lambda").params,
            vec![TypeFact::Int]
        );

        let chained_some = stdlib_method_fact(
            &TypeFact::option_some(TypeFact::Int),
            "and_then",
            Some(&TypeFact::option_none()),
        )
        .expect("some and_then fact");
        assert_eq!(chained_some.returns, TypeFact::option_none());

        let recovered = stdlib_method_fact(
            &TypeFact::option(TypeFact::Int),
            "or_else",
            Some(&TypeFact::option(TypeFact::String)),
        )
        .expect("option or_else fact");
        assert_eq!(
            recovered.returns,
            TypeFact::option(TypeFact::union([TypeFact::Int, TypeFact::String]))
        );
        assert_eq!(
            recovered.lambda.expect("option or_else lambda").params,
            Vec::<TypeFact>::new()
        );

        let recovered_some = stdlib_method_fact(
            &TypeFact::option_some(TypeFact::Int),
            "or_else",
            Some(&TypeFact::option(TypeFact::String)),
        )
        .expect("some or_else fact");
        assert_eq!(recovered_some.returns, TypeFact::option_some(TypeFact::Int));

        let recovered_none = stdlib_method_fact(
            &TypeFact::option_none(),
            "or_else",
            Some(&TypeFact::option_some(TypeFact::String)),
        )
        .expect("none or_else fact");
        assert_eq!(
            recovered_none.returns,
            TypeFact::option_some(TypeFact::String)
        );

        let filtered = stdlib_method_fact(&TypeFact::option(TypeFact::Int), "filter", None)
            .expect("option filter fact");
        assert_eq!(filtered.returns, TypeFact::option(TypeFact::Int));
        assert_eq!(
            filtered.lambda.expect("option filter lambda").params,
            vec![TypeFact::Int]
        );

        let filtered_some =
            stdlib_method_fact(&TypeFact::option_some(TypeFact::String), "filter", None)
                .expect("some filter fact");
        assert_eq!(filtered_some.returns, TypeFact::option(TypeFact::String));

        let filtered_none =
            stdlib_method_fact(&TypeFact::option_none(), "filter", None).expect("none filter fact");
        assert_eq!(filtered_none.returns, TypeFact::option_none());

        let flattened_option = stdlib_method_fact(
            &TypeFact::option(TypeFact::option(TypeFact::String)),
            "flatten",
            None,
        )
        .expect("option flatten fact");
        assert_eq!(flattened_option.returns, TypeFact::option(TypeFact::String));
        assert!(stdlib_method_fact(&TypeFact::option(TypeFact::String), "flatten", None).is_none());

        let result = stdlib_method_fact(
            &TypeFact::result(TypeFact::Int, TypeFact::record("Error")),
            "map",
            Some(&TypeFact::String),
        )
        .expect("result map fact");
        assert_eq!(
            result.returns,
            TypeFact::result(TypeFact::String, TypeFact::record("Error"))
        );
        assert_eq!(
            result.lambda.expect("result map lambda").params,
            vec![TypeFact::Int]
        );

        let ok = stdlib_method_fact(
            &TypeFact::result_ok(TypeFact::Int),
            "map",
            Some(&TypeFact::String),
        )
        .expect("ok map fact");
        assert_eq!(ok.returns, TypeFact::result_ok(TypeFact::String));

        let err = stdlib_method_fact(
            &TypeFact::result_err(TypeFact::record("Error")),
            "map",
            Some(&TypeFact::String),
        )
        .expect("err map fact");
        assert_eq!(err.returns, TypeFact::result_err(TypeFact::record("Error")));

        let mapped_error = stdlib_method_fact(
            &TypeFact::result(TypeFact::Int, TypeFact::record("Error")),
            "map_err",
            Some(&TypeFact::String),
        )
        .expect("result map_err fact");
        assert_eq!(
            mapped_error.returns,
            TypeFact::result(TypeFact::Int, TypeFact::String)
        );
        assert_eq!(
            mapped_error.lambda.expect("map_err lambda").params,
            vec![TypeFact::record("Error")]
        );

        let ok_error = stdlib_method_fact(
            &TypeFact::result_ok(TypeFact::Int),
            "map_err",
            Some(&TypeFact::String),
        )
        .expect("ok map_err fact");
        assert_eq!(ok_error.returns, TypeFact::result_ok(TypeFact::Int));

        let err_error = stdlib_method_fact(
            &TypeFact::result_err(TypeFact::record("Error")),
            "map_err",
            Some(&TypeFact::String),
        )
        .expect("err map_err fact");
        assert_eq!(err_error.returns, TypeFact::result_err(TypeFact::String));

        let chained_result = stdlib_method_fact(
            &TypeFact::result(TypeFact::Int, TypeFact::record("Error")),
            "and_then",
            Some(&TypeFact::result(TypeFact::String, TypeFact::String)),
        )
        .expect("result and_then fact");
        assert_eq!(
            chained_result.returns,
            TypeFact::result(
                TypeFact::String,
                TypeFact::union([TypeFact::record("Error"), TypeFact::String])
            )
        );
        assert_eq!(
            chained_result
                .lambda
                .expect("result and_then lambda")
                .params,
            vec![TypeFact::Int]
        );

        let chained_ok = stdlib_method_fact(
            &TypeFact::result_ok(TypeFact::Int),
            "and_then",
            Some(&TypeFact::result_err(TypeFact::String)),
        )
        .expect("ok and_then fact");
        assert_eq!(chained_ok.returns, TypeFact::result_err(TypeFact::String));

        let chained_err = stdlib_method_fact(
            &TypeFact::result_err(TypeFact::record("Error")),
            "and_then",
            Some(&TypeFact::result(TypeFact::String, TypeFact::String)),
        )
        .expect("err and_then fact");
        assert_eq!(
            chained_err.returns,
            TypeFact::result_err(TypeFact::record("Error"))
        );

        let recovered_result = stdlib_method_fact(
            &TypeFact::result(TypeFact::Int, TypeFact::record("Error")),
            "or_else",
            Some(&TypeFact::result(TypeFact::String, TypeFact::String)),
        )
        .expect("result or_else fact");
        assert_eq!(
            recovered_result.returns,
            TypeFact::result(
                TypeFact::union([TypeFact::Int, TypeFact::String]),
                TypeFact::String
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
            &TypeFact::result_ok(TypeFact::Int),
            "or_else",
            Some(&TypeFact::result(TypeFact::String, TypeFact::String)),
        )
        .expect("ok or_else fact");
        assert_eq!(recovered_ok.returns, TypeFact::result_ok(TypeFact::Int));

        let recovered_err = stdlib_method_fact(
            &TypeFact::result_err(TypeFact::record("Error")),
            "or_else",
            Some(&TypeFact::result_ok(TypeFact::String)),
        )
        .expect("err or_else fact");
        assert_eq!(recovered_err.returns, TypeFact::result_ok(TypeFact::String));

        let result_is_ok = stdlib_method_fact(
            &TypeFact::result(TypeFact::Int, TypeFact::record("Error")),
            "is_ok",
            None,
        )
        .expect("result is_ok fact");
        assert_eq!(result_is_ok.returns, TypeFact::Bool);

        let unwrapped_result = stdlib_method_fact(
            &TypeFact::result(TypeFact::Int, TypeFact::record("Error")),
            "unwrap_or",
            None,
        )
        .expect("result unwrap_or fact");
        assert_eq!(
            unwrapped_result.returns,
            TypeFact::union([TypeFact::Int, TypeFact::Any])
        );
        assert_eq!(unwrapped_result.params, vec![TypeFact::Any]);

        let ok_to_option =
            stdlib_method_fact(&TypeFact::result_ok(TypeFact::Int), "to_option", None)
                .expect("ok to_option fact");
        assert_eq!(ok_to_option.returns, TypeFact::option_some(TypeFact::Int));

        let err_to_option = stdlib_method_fact(
            &TypeFact::result_err(TypeFact::record("Error")),
            "to_option",
            None,
        )
        .expect("err to_option fact");
        assert_eq!(err_to_option.returns, TypeFact::option_none());

        let maybe_to_error_option = stdlib_method_fact(
            &TypeFact::result(TypeFact::Int, TypeFact::record("Error")),
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
                TypeFact::result(TypeFact::String, TypeFact::record("InnerError")),
                TypeFact::record("OuterError"),
            ),
            "flatten",
            None,
        )
        .expect("result flatten fact");
        assert_eq!(
            flattened_result.returns,
            TypeFact::result(
                TypeFact::String,
                TypeFact::union([
                    TypeFact::record("OuterError"),
                    TypeFact::record("InnerError")
                ])
            )
        );
        assert!(
            stdlib_method_fact(
                &TypeFact::result(TypeFact::String, TypeFact::record("Error")),
                "flatten",
                None
            )
            .is_none()
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
            stdlib_function_fact(
                "math.distance2d",
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
                "math.distance3d",
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
            stdlib_function_fact("math.pow", &[TypeFact::Int, TypeFact::Float])
                .expect("pow fact")
                .returns,
            TypeFact::Union(vec![TypeFact::Int, TypeFact::Float])
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
        let option_facts = stdlib_method_facts(&TypeFact::option(TypeFact::Int), None);
        assert!(
            option_facts
                .iter()
                .any(|fact| fact.method == "unwrap_or" && fact.params == vec![TypeFact::Any])
        );
        assert!(option_facts.iter().any(|fact| fact.method == "ok_or"
            && fact.returns == TypeFact::result(TypeFact::Int, TypeFact::Any)));
        assert!(option_facts.iter().any(|fact| {
            fact.method == "map"
                && fact
                    .lambda
                    .as_ref()
                    .is_some_and(|lambda| lambda.params == vec![TypeFact::Int])
        }));
        let nested_option_facts =
            stdlib_method_facts(&TypeFact::option(TypeFact::option(TypeFact::Int)), None);
        assert!(nested_option_facts.iter().any(|fact| {
            fact.method == "flatten" && fact.returns == TypeFact::option(TypeFact::Int)
        }));
        let result_facts =
            stdlib_method_facts(&TypeFact::result(TypeFact::Int, TypeFact::String), None);
        assert!(
            result_facts
                .iter()
                .any(|fact| fact.method == "unwrap_or" && fact.params == vec![TypeFact::Any])
        );
        assert!(
            result_facts.iter().any(|fact| fact.method == "to_option"
                && fact.returns == TypeFact::option(TypeFact::Int))
        );
        assert!(
            result_facts
                .iter()
                .any(|fact| fact.method == "to_error_option"
                    && fact.returns == TypeFact::option(TypeFact::String))
        );
        assert!(result_facts.iter().any(|fact| {
            fact.method == "map_err"
                && fact
                    .lambda
                    .as_ref()
                    .is_some_and(|lambda| lambda.params == vec![TypeFact::String])
        }));
        let nested_result_facts = stdlib_method_facts(
            &TypeFact::result(
                TypeFact::result(TypeFact::Int, TypeFact::String),
                TypeFact::record("OuterError"),
            ),
            None,
        );
        assert!(nested_result_facts.iter().any(|fact| {
            fact.method == "flatten"
                && fact.returns
                    == TypeFact::result(
                        TypeFact::Int,
                        TypeFact::union([TypeFact::record("OuterError"), TypeFact::String]),
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

    #[test]
    fn stdlib_function_completion_facts_enumerate_global_api_surface() {
        let number = TypeFact::Union(vec![TypeFact::Int, TypeFact::Float]);
        let facts = stdlib_function_completion_facts();

        assert!(facts.iter().any(|fact| {
            fact.name == "option.unwrap_or"
                && fact.params == vec![TypeFact::option(TypeFact::Any), TypeFact::Any]
                && fact.returns == TypeFact::Any
        }));
        assert!(facts.iter().any(|fact| {
            fact.name == "option.ok_or"
                && fact.params == vec![TypeFact::option(TypeFact::Any), TypeFact::Any]
                && fact.returns == TypeFact::result(TypeFact::Any, TypeFact::Any)
        }));
        assert!(facts.iter().any(|fact| {
            fact.name == "option.flatten"
                && fact.params == vec![TypeFact::option(TypeFact::option(TypeFact::Any))]
                && fact.returns == TypeFact::option(TypeFact::Any)
        }));
        assert!(facts.iter().any(|fact| {
            fact.name == "result.to_option"
                && fact.params == vec![TypeFact::result(TypeFact::Any, TypeFact::Any)]
                && fact.returns == TypeFact::option(TypeFact::Any)
        }));
        assert!(facts.iter().any(|fact| {
            fact.name == "result.to_error_option"
                && fact.params == vec![TypeFact::result(TypeFact::Any, TypeFact::Any)]
                && fact.returns == TypeFact::option(TypeFact::Any)
        }));
        assert!(facts.iter().any(|fact| {
            fact.name == "result.flatten"
                && fact.params
                    == vec![TypeFact::result(
                        TypeFact::result(TypeFact::Any, TypeFact::Any),
                        TypeFact::Any,
                    )]
                && fact.returns == TypeFact::result(TypeFact::Any, TypeFact::Any)
        }));
        assert!(facts.iter().any(|fact| {
            fact.name == "math.clamp" && fact.params.len() == 3 && fact.returns == number
        }));
        assert!(facts.iter().any(|fact| {
            fact.name == "math.lerp" && fact.params.len() == 3 && fact.returns == TypeFact::Float
        }));
        assert!(facts.iter().any(|fact| {
            fact.name == "math.distance2d"
                && fact.params.len() == 4
                && fact.returns == TypeFact::Float
        }));
        assert!(facts.iter().any(|fact| {
            fact.name == "math.distance3d"
                && fact.params.len() == 6
                && fact.returns == TypeFact::Float
        }));
        assert!(facts.iter().any(|fact| {
            fact.name == "math.pow" && fact.params.len() == 2 && fact.returns == number
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
