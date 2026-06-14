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

    let mapped = stdlib_method_fact(&receiver, "map", Some(&TypeFact::STRING)).expect("map fact");
    assert_eq!(mapped.returns, TypeFact::array(TypeFact::STRING));

    let found = stdlib_method_fact(&receiver, "find", None).expect("find fact");
    assert_eq!(found.returns, TypeFact::option(TypeFact::record("Reward")));
}

#[test]
fn map_lambda_methods_expose_key_and_value_parameter_facts() {
    let receiver = TypeFact::map(TypeFact::STRING, TypeFact::I64);

    let filter = stdlib_method_fact(&receiver, "filter", None).expect("filter fact");
    assert_eq!(
        filter.lambda.expect("filter lambda").params,
        vec![TypeFact::STRING, TypeFact::I64]
    );
    assert_eq!(filter.returns, receiver);

    let mapped =
        stdlib_method_fact(&receiver, "map_values", Some(&TypeFact::BOOL)).expect("map fact");
    assert_eq!(
        mapped.returns,
        TypeFact::map(TypeFact::STRING, TypeFact::BOOL)
    );
    assert_eq!(
        mapped.lambda.expect("map_values lambda").params,
        vec![TypeFact::STRING, TypeFact::I64]
    );

    let merged = stdlib_method_fact(&receiver, "merge", None).expect("merge fact");
    assert_eq!(
        merged.params,
        vec![TypeFact::map(TypeFact::STRING, TypeFact::I64)]
    );
    assert_eq!(merged.returns, receiver);

    let found = stdlib_method_fact(&receiver, "find", None).expect("find fact");
    assert_eq!(
        found.returns,
        TypeFact::option(TypeFact::record("MapEntry"))
    );
    assert_eq!(
        found.lambda.expect("find lambda").params,
        vec![TypeFact::STRING, TypeFact::I64]
    );

    let any = stdlib_method_fact(&receiver, "any", None).expect("any fact");
    assert_eq!(any.returns, TypeFact::BOOL);
    assert_eq!(
        any.lambda.expect("any lambda").params,
        vec![TypeFact::STRING, TypeFact::I64]
    );

    let all = stdlib_method_fact(&receiver, "all", None).expect("all fact");
    assert_eq!(all.returns, TypeFact::BOOL);
    assert_eq!(
        all.lambda.expect("all lambda").params,
        vec![TypeFact::STRING, TypeFact::I64]
    );

    let count = stdlib_method_fact(&receiver, "count", None).expect("count fact");
    assert_eq!(count.returns, TypeFact::I64);
    assert_eq!(
        count.lambda.expect("count lambda").params,
        vec![TypeFact::STRING, TypeFact::I64]
    );
}

#[test]
fn map_lambda_methods_expose_value_only_parameter_facts_by_arity() {
    let receiver = TypeFact::map(TypeFact::STRING, TypeFact::I64);

    let mapped = stdlib_method_fact_with_lambda_arity(
        &receiver,
        "map_values",
        Some(&TypeFact::BOOL),
        Some(1),
    )
    .expect("map_values fact");
    assert_eq!(
        mapped.lambda.expect("map_values lambda").params,
        vec![TypeFact::I64]
    );
    assert_eq!(
        mapped.params,
        vec![TypeFact::function(vec![TypeFact::I64], TypeFact::BOOL)]
    );
    assert_eq!(
        mapped.returns,
        TypeFact::map(TypeFact::STRING, TypeFact::BOOL)
    );

    let filter = stdlib_method_fact_with_lambda_arity(&receiver, "filter", None, Some(0))
        .expect("filter fact");
    assert_eq!(
        filter.lambda.expect("filter lambda").params,
        Vec::<TypeFact>::new()
    );
    assert_eq!(
        filter.params,
        vec![TypeFact::function(Vec::<TypeFact>::new(), TypeFact::BOOL)]
    );
    assert_eq!(filter.returns, receiver);
}

#[test]
fn scalar_collection_methods_return_non_generic_facts() {
    let map = TypeFact::map(TypeFact::STRING, TypeFact::I64);
    let array = TypeFact::array(TypeFact::F64);
    let set = TypeFact::set(TypeFact::STRING);
    let range = TypeFact::Range;

    assert_eq!(
        stdlib_method_fact(&map, "keys", None)
            .expect("keys fact")
            .returns,
        TypeFact::iterator(TypeFact::STRING)
    );
    assert_eq!(
        stdlib_method_fact(&map, "values", None)
            .expect("values fact")
            .returns,
        TypeFact::iterator(TypeFact::I64)
    );
    assert_eq!(
        stdlib_method_fact(&map, "iter", None)
            .expect("map iter fact")
            .returns,
        TypeFact::iterator(TypeFact::I64)
    );
    assert_eq!(
        stdlib_method_fact(&map, "entries", None)
            .expect("entries fact")
            .returns,
        TypeFact::iterator(TypeFact::record("MapEntry"))
    );
    assert_eq!(
        stdlib_method_fact(&map, "clear", None)
            .expect("map clear fact")
            .returns,
        TypeFact::NULL
    );
    let map_extend = stdlib_method_fact(&map, "extend", None).expect("map extend fact");
    assert_eq!(
        map_extend.params,
        vec![TypeFact::map(TypeFact::STRING, TypeFact::I64)]
    );
    assert_eq!(map_extend.returns, TypeFact::NULL);
    assert_eq!(
        stdlib_method_fact(&array, "sum", None)
            .expect("sum fact")
            .returns,
        TypeFact::F64
    );
    assert_eq!(
        stdlib_method_fact(&array, "pop", None)
            .expect("pop fact")
            .returns,
        TypeFact::option(TypeFact::F64)
    );
    assert_eq!(
        stdlib_method_fact(&array, "first", None)
            .expect("first fact")
            .returns,
        TypeFact::option(TypeFact::F64)
    );
    assert_eq!(
        stdlib_method_fact(&array, "last", None)
            .expect("last fact")
            .returns,
        TypeFact::option(TypeFact::F64)
    );
    let remove_at = stdlib_method_fact(&array, "remove_at", None).expect("remove_at fact");
    assert_eq!(remove_at.params, vec![TypeFact::I64]);
    assert_eq!(remove_at.returns, TypeFact::option(TypeFact::F64));
    let insert = stdlib_method_fact(&array, "insert", None).expect("insert fact");
    assert_eq!(insert.params, vec![TypeFact::I64, TypeFact::F64]);
    assert_eq!(insert.returns, TypeFact::NULL);
    let extend = stdlib_method_fact(&array, "extend", None).expect("extend fact");
    assert_eq!(extend.params, vec![TypeFact::array(TypeFact::F64)]);
    assert_eq!(extend.returns, TypeFact::NULL);
    assert_eq!(
        stdlib_method_fact(&array, "clear", None)
            .expect("array clear fact")
            .returns,
        TypeFact::NULL
    );
    let join = stdlib_method_fact(&array, "join", None).expect("join fact");
    assert_eq!(join.params, vec![TypeFact::STRING]);
    assert_eq!(join.returns, TypeFact::STRING);
    let contains = stdlib_method_fact(&array, "contains", None).expect("contains fact");
    assert_eq!(contains.params, vec![TypeFact::F64]);
    assert_eq!(contains.returns, TypeFact::BOOL);
    let index_of = stdlib_method_fact(&array, "index_of", None).expect("index_of fact");
    assert_eq!(index_of.params, vec![TypeFact::F64]);
    assert_eq!(index_of.returns, TypeFact::option(TypeFact::I64));
    assert_eq!(
        stdlib_method_fact(&array, "distinct", None)
            .expect("distinct fact")
            .returns,
        TypeFact::array(TypeFact::F64)
    );
    assert_eq!(
        stdlib_method_fact(&array, "reverse", None)
            .expect("reverse fact")
            .returns,
        TypeFact::array(TypeFact::F64)
    );
    assert_eq!(
        stdlib_method_fact(&array, "sort", None)
            .expect("sort fact")
            .returns,
        TypeFact::array(TypeFact::F64)
    );
    assert_eq!(
        stdlib_method_fact(&array, "min", None)
            .expect("min fact")
            .returns,
        TypeFact::option(TypeFact::F64)
    );
    assert_eq!(
        stdlib_method_fact(&array, "max", None)
            .expect("max fact")
            .returns,
        TypeFact::option(TypeFact::F64)
    );
    let slice = stdlib_method_fact(&array, "slice", None).expect("slice fact");
    assert_eq!(slice.params, vec![TypeFact::I64, TypeFact::I64]);
    assert_eq!(slice.returns, TypeFact::array(TypeFact::F64));
    assert_eq!(
        stdlib_method_fact(&array, "iter", None)
            .expect("array iter fact")
            .returns,
        TypeFact::iterator(TypeFact::F64)
    );
    assert_eq!(
        stdlib_method_fact(&array, "values", None)
            .expect("array values fact")
            .returns,
        TypeFact::iterator(TypeFact::F64)
    );
    assert_eq!(
        stdlib_method_fact(&set, "values", None)
            .expect("values fact")
            .returns,
        TypeFact::iterator(TypeFact::STRING)
    );
    assert_eq!(
        stdlib_method_fact(&set, "iter", None)
            .expect("set iter fact")
            .returns,
        TypeFact::iterator(TypeFact::STRING)
    );
    assert_eq!(
        stdlib_method_fact(&set, "clear", None)
            .expect("set clear fact")
            .returns,
        TypeFact::NULL
    );
    let set_extend = stdlib_method_fact(&set, "extend", None).expect("set extend fact");
    assert_eq!(set_extend.params, vec![TypeFact::set(TypeFact::STRING)]);
    assert_eq!(set_extend.returns, TypeFact::NULL);
    let set_map = stdlib_method_fact(&set, "map", Some(&TypeFact::I64)).expect("set map fact");
    assert_eq!(
        set_map.params,
        vec![TypeFact::function(vec![TypeFact::STRING], TypeFact::I64)]
    );
    assert_eq!(set_map.returns, TypeFact::set(TypeFact::I64));
    let set_filter = stdlib_method_fact(&set, "filter", None).expect("set filter fact");
    assert_eq!(
        set_filter.params,
        vec![TypeFact::function(vec![TypeFact::STRING], TypeFact::BOOL)]
    );
    assert_eq!(set_filter.returns, TypeFact::set(TypeFact::STRING));
    let set_find = stdlib_method_fact(&set, "find", None).expect("set find fact");
    assert_eq!(
        set_find.params,
        vec![TypeFact::function(vec![TypeFact::STRING], TypeFact::BOOL)]
    );
    assert_eq!(set_find.returns, TypeFact::option(TypeFact::STRING));
    let set_any = stdlib_method_fact(&set, "any", None).expect("set any fact");
    assert_eq!(
        set_any.params,
        vec![TypeFact::function(vec![TypeFact::STRING], TypeFact::BOOL)]
    );
    assert_eq!(set_any.returns, TypeFact::BOOL);
    let set_all = stdlib_method_fact(&set, "all", None).expect("set all fact");
    assert_eq!(
        set_all.params,
        vec![TypeFact::function(vec![TypeFact::STRING], TypeFact::BOOL)]
    );
    assert_eq!(set_all.returns, TypeFact::BOOL);
    let set_count = stdlib_method_fact(&set, "count", None).expect("set count fact");
    assert_eq!(
        set_count.params,
        vec![TypeFact::function(vec![TypeFact::STRING], TypeFact::BOOL)]
    );
    assert_eq!(set_count.returns, TypeFact::I64);
    let union = stdlib_method_fact(&set, "union", None).expect("union fact");
    assert_eq!(union.params, vec![TypeFact::set(TypeFact::STRING)]);
    assert_eq!(union.returns, TypeFact::set(TypeFact::STRING));
    let intersection = stdlib_method_fact(&set, "intersection", None).expect("intersection fact");
    assert_eq!(intersection.params, vec![TypeFact::set(TypeFact::STRING)]);
    assert_eq!(intersection.returns, TypeFact::set(TypeFact::STRING));
    let difference = stdlib_method_fact(&set, "difference", None).expect("difference fact");
    assert_eq!(difference.params, vec![TypeFact::set(TypeFact::STRING)]);
    assert_eq!(difference.returns, TypeFact::set(TypeFact::STRING));
    let symmetric_difference =
        stdlib_method_fact(&set, "symmetric_difference", None).expect("symmetric fact");
    assert_eq!(
        symmetric_difference.params,
        vec![TypeFact::set(TypeFact::STRING)]
    );
    assert_eq!(
        symmetric_difference.returns,
        TypeFact::set(TypeFact::STRING)
    );
    let subset = stdlib_method_fact(&set, "is_subset", None).expect("is_subset fact");
    assert_eq!(subset.params, vec![TypeFact::set(TypeFact::STRING)]);
    assert_eq!(subset.returns, TypeFact::BOOL);
    let superset = stdlib_method_fact(&set, "is_superset", None).expect("is_superset fact");
    assert_eq!(superset.params, vec![TypeFact::set(TypeFact::STRING)]);
    assert_eq!(superset.returns, TypeFact::BOOL);
    let disjoint = stdlib_method_fact(&set, "is_disjoint", None).expect("is_disjoi64 fact");
    assert_eq!(disjoint.params, vec![TypeFact::set(TypeFact::STRING)]);
    assert_eq!(disjoint.returns, TypeFact::BOOL);
    assert_eq!(
        stdlib_method_fact(&range, "len", None)
            .expect("range len fact")
            .returns,
        TypeFact::I64
    );
    assert_eq!(
        stdlib_method_fact(&range, "is_empty", None)
            .expect("range is_empty fact")
            .returns,
        TypeFact::BOOL
    );
    assert_eq!(
        stdlib_method_fact(&range, "iter", None)
            .expect("range iter fact")
            .returns,
        TypeFact::iterator(TypeFact::I64)
    );
}

#[test]
fn iterator_methods_expose_item_and_callback_facts_without_generics() {
    let iterator = TypeFact::iterator(TypeFact::record("Reward"));

    let next = stdlib_method_fact(&iterator, "next", None).expect("next fact");
    assert_eq!(next.returns, TypeFact::option(TypeFact::record("Reward")));

    let mapped = stdlib_method_fact(&iterator, "map", Some(&TypeFact::STRING)).expect("map fact");
    assert_eq!(mapped.returns, TypeFact::iterator(TypeFact::STRING));
    assert_eq!(
        mapped.lambda.expect("map lambda").params,
        vec![TypeFact::record("Reward")]
    );

    let filter = stdlib_method_fact(&iterator, "filter", None).expect("filter fact");
    assert_eq!(
        filter.params,
        vec![TypeFact::function(
            vec![TypeFact::record("Reward")],
            TypeFact::BOOL
        )]
    );
    assert_eq!(filter.returns, iterator.clone());

    let take = stdlib_method_fact(&iterator, "take", None).expect("take fact");
    assert_eq!(take.params, vec![TypeFact::I64]);
    assert_eq!(take.returns, TypeFact::iterator(TypeFact::record("Reward")));

    let collect = stdlib_method_fact(&iterator, "collect_array", None).expect("collect_array fact");
    assert_eq!(collect.returns, TypeFact::array(TypeFact::record("Reward")));
    let collect_set = stdlib_method_fact(&iterator, "collect_set", None).expect("collect_set fact");
    assert_eq!(
        collect_set.returns,
        TypeFact::set(TypeFact::record("Reward"))
    );
    let collect_map = stdlib_method_fact(&iterator, "collect_map", None).expect("collect_map fact");
    assert_eq!(
        collect_map.returns,
        TypeFact::map(TypeFact::STRING, TypeFact::Any)
    );
    assert_eq!(iterator.display_name(), "Iterator");
}

#[test]
fn char_methods_expose_rust_like_character_facts() {
    let to_string =
        stdlib_method_fact(&TypeFact::CHAR, "to_string", None).expect("char to_string fact");
    assert_eq!(to_string.params, Vec::<TypeFact>::new());
    assert_eq!(to_string.returns, TypeFact::STRING);

    let is_ascii_digit = stdlib_method_fact(&TypeFact::CHAR, "is_ascii_digit", None)
        .expect("char is_ascii_digit fact");
    assert_eq!(is_ascii_digit.params, Vec::<TypeFact>::new());
    assert_eq!(is_ascii_digit.returns, TypeFact::BOOL);
}

#[test]
fn string_methods_expose_replacement_and_split_facts() {
    let find = stdlib_method_fact(&TypeFact::STRING, "find", None).expect("find fact");
    assert_eq!(find.params, vec![TypeFact::STRING]);
    assert_eq!(find.returns, TypeFact::option(TypeFact::I64));
    assert_eq!(
        stdlib_method_fact(&TypeFact::STRING, "chars", None)
            .expect("chars fact")
            .returns,
        TypeFact::iterator(TypeFact::CHAR)
    );
    assert_eq!(
        stdlib_method_fact(&TypeFact::STRING, "bytes", None)
            .expect("bytes fact")
            .returns,
        TypeFact::iterator(TypeFact::U8)
    );

    let strip_prefix =
        stdlib_method_fact(&TypeFact::STRING, "strip_prefix", None).expect("prefix fact");
    assert_eq!(strip_prefix.params, vec![TypeFact::STRING]);
    assert_eq!(strip_prefix.returns, TypeFact::option(TypeFact::STRING));

    let strip_suffix =
        stdlib_method_fact(&TypeFact::STRING, "strip_suffix", None).expect("suffix fact");
    assert_eq!(strip_suffix.params, vec![TypeFact::STRING]);
    assert_eq!(strip_suffix.returns, TypeFact::option(TypeFact::STRING));

    let replace = stdlib_method_fact(&TypeFact::STRING, "replace", None).expect("replace fact");
    assert_eq!(replace.params, vec![TypeFact::STRING, TypeFact::STRING]);
    assert_eq!(replace.returns, TypeFact::STRING);

    let repeat = stdlib_method_fact(&TypeFact::STRING, "repeat", None).expect("repeat fact");
    assert_eq!(repeat.params, vec![TypeFact::I64]);
    assert_eq!(repeat.returns, TypeFact::STRING);

    let trim_start =
        stdlib_method_fact(&TypeFact::STRING, "trim_start", None).expect("trim_start fact");
    assert_eq!(trim_start.params, Vec::<TypeFact>::new());
    assert_eq!(trim_start.returns, TypeFact::STRING);

    let trim_end = stdlib_method_fact(&TypeFact::STRING, "trim_end", None).expect("trim_end fact");
    assert_eq!(trim_end.params, Vec::<TypeFact>::new());
    assert_eq!(trim_end.returns, TypeFact::STRING);

    let slice = stdlib_method_fact(&TypeFact::STRING, "slice", None).expect("slice fact");
    assert_eq!(slice.params, vec![TypeFact::I64, TypeFact::I64]);
    assert_eq!(slice.returns, TypeFact::STRING);

    let split = stdlib_method_fact(&TypeFact::STRING, "split", None).expect("split fact");
    assert_eq!(split.params, vec![TypeFact::STRING]);
    assert_eq!(split.returns, TypeFact::array(TypeFact::STRING));

    let split_once =
        stdlib_method_fact(&TypeFact::STRING, "split_once", None).expect("split_once fact");
    assert_eq!(split_once.params, vec![TypeFact::STRING]);
    assert_eq!(
        split_once.returns,
        TypeFact::option(TypeFact::array(TypeFact::STRING))
    );

    let split_lines =
        stdlib_method_fact(&TypeFact::STRING, "split_lines", None).expect("split_lines fact");
    assert_eq!(split_lines.params, Vec::<TypeFact>::new());
    assert_eq!(split_lines.returns, TypeFact::array(TypeFact::STRING));

    for (method, returns) in [
        ("parse_i8", TypeFact::I8),
        ("parse_i16", TypeFact::I16),
        ("parse_i32", TypeFact::I32),
        ("parse_i64", TypeFact::I64),
        ("parse_u8", TypeFact::U8),
        ("parse_u16", TypeFact::U16),
        ("parse_u32", TypeFact::U32),
        ("parse_u64", TypeFact::U64),
        ("parse_f32", TypeFact::F32),
        ("parse_f64", TypeFact::F64),
        ("parse_bool", TypeFact::BOOL),
        ("parse_char", TypeFact::CHAR),
    ] {
        let fact = stdlib_method_fact(&TypeFact::STRING, method, None).expect("parse fact");
        assert_eq!(fact.params, Vec::<TypeFact>::new());
        assert_eq!(fact.returns, TypeFact::option(returns));
    }
}

#[test]
fn bytes_methods_expose_binary_api_facts() {
    let len = stdlib_method_fact(&TypeFact::BYTES, "len", None).expect("bytes len fact");
    assert_eq!(len.params, Vec::<TypeFact>::new());
    assert_eq!(len.returns, TypeFact::I64);

    let slice = stdlib_method_fact(&TypeFact::BYTES, "slice", None).expect("bytes slice fact");
    assert_eq!(slice.params, vec![TypeFact::I64, TypeFact::I64]);
    assert_eq!(slice.returns, TypeFact::BYTES);

    let get = stdlib_method_fact(&TypeFact::BYTES, "get", None).expect("bytes get fact");
    assert_eq!(get.params, vec![TypeFact::I64]);
    assert_eq!(get.returns, TypeFact::U8);

    let read_le =
        stdlib_method_fact(&TypeFact::BYTES, "read_u32_le", None).expect("bytes read fact");
    assert_eq!(read_le.params, vec![TypeFact::I64]);
    assert_eq!(read_le.returns, TypeFact::U32);

    let hex = stdlib_method_fact(&TypeFact::BYTES, "to_hex", None).expect("bytes hex fact");
    assert_eq!(hex.returns, TypeFact::STRING);

    let values = stdlib_method_fact(&TypeFact::BYTES, "values", None).expect("bytes values fact");
    assert_eq!(values.params, Vec::<TypeFact>::new());
    assert_eq!(values.returns, TypeFact::iterator(TypeFact::U8));
}
