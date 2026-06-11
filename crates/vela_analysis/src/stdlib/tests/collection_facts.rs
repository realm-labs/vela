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

    let mapped = stdlib_method_fact(&receiver, "map", Some(&TypeFact::String)).expect("map fact");
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
fn map_lambda_methods_expose_value_only_parameter_facts_by_arity() {
    let receiver = TypeFact::map(TypeFact::String, TypeFact::Int);

    let mapped = stdlib_method_fact_with_lambda_arity(
        &receiver,
        "map_values",
        Some(&TypeFact::Bool),
        Some(1),
    )
    .expect("map_values fact");
    assert_eq!(
        mapped.lambda.expect("map_values lambda").params,
        vec![TypeFact::Int]
    );
    assert_eq!(
        mapped.params,
        vec![TypeFact::function(vec![TypeFact::Int], TypeFact::Bool)]
    );
    assert_eq!(
        mapped.returns,
        TypeFact::map(TypeFact::String, TypeFact::Bool)
    );

    let filter = stdlib_method_fact_with_lambda_arity(&receiver, "filter", None, Some(0))
        .expect("filter fact");
    assert_eq!(
        filter.lambda.expect("filter lambda").params,
        Vec::<TypeFact>::new()
    );
    assert_eq!(
        filter.params,
        vec![TypeFact::function(Vec::<TypeFact>::new(), TypeFact::Bool)]
    );
    assert_eq!(filter.returns, receiver);
}

#[test]
fn scalar_collection_methods_return_non_generic_facts() {
    let map = TypeFact::map(TypeFact::String, TypeFact::Int);
    let array = TypeFact::array(TypeFact::Float);
    let set = TypeFact::set(TypeFact::String);
    let range = TypeFact::Range;

    assert_eq!(
        stdlib_method_fact(&map, "keys", None)
            .expect("keys fact")
            .returns,
        TypeFact::array(TypeFact::String)
    );
    assert_eq!(
        stdlib_method_fact(&map, "values", None)
            .expect("values fact")
            .returns,
        TypeFact::array(TypeFact::Int)
    );
    assert_eq!(
        stdlib_method_fact(&map, "entries", None)
            .expect("entries fact")
            .returns,
        TypeFact::array(TypeFact::record("MapEntry"))
    );
    assert_eq!(
        stdlib_method_fact(&map, "clear", None)
            .expect("map clear fact")
            .returns,
        TypeFact::Null
    );
    let map_extend = stdlib_method_fact(&map, "extend", None).expect("map extend fact");
    assert_eq!(
        map_extend.params,
        vec![TypeFact::map(TypeFact::String, TypeFact::Int)]
    );
    assert_eq!(map_extend.returns, TypeFact::Null);
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
    let remove_at = stdlib_method_fact(&array, "remove_at", None).expect("remove_at fact");
    assert_eq!(remove_at.params, vec![TypeFact::Int]);
    assert_eq!(remove_at.returns, TypeFact::option(TypeFact::Float));
    let insert = stdlib_method_fact(&array, "insert", None).expect("insert fact");
    assert_eq!(insert.params, vec![TypeFact::Int, TypeFact::Float]);
    assert_eq!(insert.returns, TypeFact::Null);
    let extend = stdlib_method_fact(&array, "extend", None).expect("extend fact");
    assert_eq!(extend.params, vec![TypeFact::array(TypeFact::Float)]);
    assert_eq!(extend.returns, TypeFact::Null);
    assert_eq!(
        stdlib_method_fact(&array, "clear", None)
            .expect("array clear fact")
            .returns,
        TypeFact::Null
    );
    let join = stdlib_method_fact(&array, "join", None).expect("join fact");
    assert_eq!(join.params, vec![TypeFact::String]);
    assert_eq!(join.returns, TypeFact::String);
    let contains = stdlib_method_fact(&array, "contains", None).expect("contains fact");
    assert_eq!(contains.params, vec![TypeFact::Float]);
    assert_eq!(contains.returns, TypeFact::Bool);
    let index_of = stdlib_method_fact(&array, "index_of", None).expect("index_of fact");
    assert_eq!(index_of.params, vec![TypeFact::Float]);
    assert_eq!(index_of.returns, TypeFact::option(TypeFact::Int));
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
    assert_eq!(
        stdlib_method_fact(&array, "sort", None)
            .expect("sort fact")
            .returns,
        TypeFact::array(TypeFact::Float)
    );
    assert_eq!(
        stdlib_method_fact(&array, "min", None)
            .expect("min fact")
            .returns,
        TypeFact::option(TypeFact::Float)
    );
    assert_eq!(
        stdlib_method_fact(&array, "max", None)
            .expect("max fact")
            .returns,
        TypeFact::option(TypeFact::Float)
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
    assert_eq!(
        stdlib_method_fact(&set, "clear", None)
            .expect("set clear fact")
            .returns,
        TypeFact::Null
    );
    let set_extend = stdlib_method_fact(&set, "extend", None).expect("set extend fact");
    assert_eq!(set_extend.params, vec![TypeFact::set(TypeFact::String)]);
    assert_eq!(set_extend.returns, TypeFact::Null);
    let set_map = stdlib_method_fact(&set, "map", Some(&TypeFact::Int)).expect("set map fact");
    assert_eq!(
        set_map.params,
        vec![TypeFact::function(vec![TypeFact::String], TypeFact::Int)]
    );
    assert_eq!(set_map.returns, TypeFact::set(TypeFact::Int));
    let set_filter = stdlib_method_fact(&set, "filter", None).expect("set filter fact");
    assert_eq!(
        set_filter.params,
        vec![TypeFact::function(vec![TypeFact::String], TypeFact::Bool)]
    );
    assert_eq!(set_filter.returns, TypeFact::set(TypeFact::String));
    let set_find = stdlib_method_fact(&set, "find", None).expect("set find fact");
    assert_eq!(
        set_find.params,
        vec![TypeFact::function(vec![TypeFact::String], TypeFact::Bool)]
    );
    assert_eq!(set_find.returns, TypeFact::option(TypeFact::String));
    let set_any = stdlib_method_fact(&set, "any", None).expect("set any fact");
    assert_eq!(
        set_any.params,
        vec![TypeFact::function(vec![TypeFact::String], TypeFact::Bool)]
    );
    assert_eq!(set_any.returns, TypeFact::Bool);
    let set_all = stdlib_method_fact(&set, "all", None).expect("set all fact");
    assert_eq!(
        set_all.params,
        vec![TypeFact::function(vec![TypeFact::String], TypeFact::Bool)]
    );
    assert_eq!(set_all.returns, TypeFact::Bool);
    let set_count = stdlib_method_fact(&set, "count", None).expect("set count fact");
    assert_eq!(
        set_count.params,
        vec![TypeFact::function(vec![TypeFact::String], TypeFact::Bool)]
    );
    assert_eq!(set_count.returns, TypeFact::Int);
    let union = stdlib_method_fact(&set, "union", None).expect("union fact");
    assert_eq!(union.params, vec![TypeFact::set(TypeFact::String)]);
    assert_eq!(union.returns, TypeFact::set(TypeFact::String));
    let intersection = stdlib_method_fact(&set, "intersection", None).expect("intersection fact");
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
    assert_eq!(
        stdlib_method_fact(&range, "len", None)
            .expect("range len fact")
            .returns,
        TypeFact::Int
    );
    assert_eq!(
        stdlib_method_fact(&range, "is_empty", None)
            .expect("range is_empty fact")
            .returns,
        TypeFact::Bool
    );
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

    let trim_end = stdlib_method_fact(&TypeFact::String, "trim_end", None).expect("trim_end fact");
    assert_eq!(trim_end.params, Vec::<TypeFact>::new());
    assert_eq!(trim_end.returns, TypeFact::String);

    let slice = stdlib_method_fact(&TypeFact::String, "slice", None).expect("slice fact");
    assert_eq!(slice.params, vec![TypeFact::Int, TypeFact::Int]);
    assert_eq!(slice.returns, TypeFact::String);

    let split = stdlib_method_fact(&TypeFact::String, "split", None).expect("split fact");
    assert_eq!(split.params, vec![TypeFact::String]);
    assert_eq!(split.returns, TypeFact::array(TypeFact::String));

    let split_once =
        stdlib_method_fact(&TypeFact::String, "split_once", None).expect("split_once fact");
    assert_eq!(split_once.params, vec![TypeFact::String]);
    assert_eq!(
        split_once.returns,
        TypeFact::option(TypeFact::array(TypeFact::String))
    );

    let split_lines =
        stdlib_method_fact(&TypeFact::String, "split_lines", None).expect("split_lines fact");
    assert_eq!(split_lines.params, Vec::<TypeFact>::new());
    assert_eq!(split_lines.returns, TypeFact::array(TypeFact::String));

    let char_at = stdlib_method_fact(&TypeFact::String, "char_at", None).expect("char_at fact");
    assert_eq!(char_at.params, vec![TypeFact::Int]);
    assert_eq!(char_at.returns, TypeFact::option(TypeFact::String));

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
fn bytes_methods_expose_binary_api_facts() {
    let len = stdlib_method_fact(&TypeFact::Bytes, "len", None).expect("bytes len fact");
    assert_eq!(len.params, Vec::<TypeFact>::new());
    assert_eq!(len.returns, TypeFact::Int);

    let slice = stdlib_method_fact(&TypeFact::Bytes, "slice", None).expect("bytes slice fact");
    assert_eq!(slice.params, vec![TypeFact::Int, TypeFact::Int]);
    assert_eq!(slice.returns, TypeFact::Bytes);

    let get = stdlib_method_fact(&TypeFact::Bytes, "get", None).expect("bytes get fact");
    assert_eq!(get.params, vec![TypeFact::Int]);
    assert_eq!(get.returns, TypeFact::Int);

    let read_le =
        stdlib_method_fact(&TypeFact::Bytes, "read_u32_le", None).expect("bytes read fact");
    assert_eq!(read_le.params, vec![TypeFact::Int]);
    assert_eq!(read_le.returns, TypeFact::Int);

    let hex = stdlib_method_fact(&TypeFact::Bytes, "to_hex", None).expect("bytes hex fact");
    assert_eq!(hex.returns, TypeFact::String);
}
