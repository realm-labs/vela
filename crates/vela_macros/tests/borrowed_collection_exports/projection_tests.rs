use super::*;

#[test]
fn borrowed_array_ordering_uses_one_bounded_projection() {
    let mut runtime = runtime(
        "fn numeric(values: ArrayView<i64>) { let sorted = values.sort(); return sorted[0] == 3 && sorted[1] == 3 && sorted[2] == 5 && sorted[3] == 8 && values.min().unwrap_or(0) == 3 && values.max().unwrap_or(0) == 8; } fn retained(owner: CollectionOwner) { let values = owner.values(); return values.sort()[0] + values.min().unwrap_or(0) + values.max().unwrap_or(0); } fn words(values: ArrayView<String>) { return values.sort().join(\"|\") == \"north|star|west\" && values.min().unwrap_or(\"\") == \"north\" && values.max().unwrap_or(\"\") == \"west\"; } fn empty(values: ArrayView<i64>) { return values.sort().is_empty() && values.min().unwrap_or(17) == 17 && values.max().unwrap_or(19) == 19; }",
    );

    let values = vec![3_i64, 5, 3, 8];
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call("numeric", args, CallOptions::unbounded())
        .expect("borrowed array ordering should reuse Vela ordering semantics");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
    drop(result);
    assert_eq!(values, vec![3, 5, 3, 8]);

    let owner = CollectionOwner {
        values: vec![7, 5, 11],
        totals: BTreeMap::new(),
    };
    let result = runtime
        .call(
            "retained",
            CallArgs::new().with_host_ref("owner", &owner),
            CallOptions::unbounded(),
        )
        .expect("retained borrowed ordering should use the parent lease");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(21)));
    drop(result);
    assert_eq!(owner.values, vec![7, 5, 11]);

    let words = vec!["west".to_owned(), "north".to_owned(), "star".to_owned()];
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &words);
    let result = runtime
        .call("words", args, CallOptions::unbounded())
        .expect("borrowed string ordering should retain lexical semantics");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
    drop(result);

    let empty = Vec::<i64>::new();
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &empty);
    let result = runtime
        .call("empty", args, CallOptions::unbounded())
        .expect("empty borrowed ordering should preserve Array and Option semantics");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
}

#[test]
fn borrowed_array_ordering_charges_projected_length() {
    let mut runtime =
        runtime("fn minimum(values: ArrayView<i64>) { return values.min().unwrap_or(0); }");
    let base_limit = (0..64)
        .find(|limit| {
            let values = Vec::<i64>::new();
            let mut args = CallArgs::new();
            args.push_collection_ref("values", &values);
            runtime
                .call(
                    "minimum",
                    args,
                    CallOptions::new(*limit, usize::MAX, usize::MAX),
                )
                .is_ok()
        })
        .expect("empty borrowed minimum should fit a small bounded call");

    let values = vec![8_i64, 3, 5];
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    assert!(
        runtime
            .call(
                "minimum",
                args,
                CallOptions::new(base_limit + 2, usize::MAX, usize::MAX),
            )
            .is_err(),
        "three projected elements must cost three execution units"
    );

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call(
            "minimum",
            args,
            CallOptions::new(base_limit + 3, usize::MAX, usize::MAX),
        )
        .expect("the exact ordering projection budget should succeed");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(3)));
}

#[test]
fn borrowed_array_transforms_use_one_bounded_projection() {
    let mut runtime = runtime(
        "fn numeric(values: ArrayView<i64>) { let unique = values.distinct(); let reversed = values.reverse(); let middle = values.slice(1, 3); return unique.len() == 3 && unique[2] == 8 && reversed[0] == 8 && reversed[3] == 3 && middle[0] == 5 && middle[1] == 3; } fn retained(owner: CollectionOwner) { let values = owner.values(); return values.reverse()[0] + values.slice(1, 3)[0]; } fn joined(values: ArrayView<String>) { return values.join(\"|\"); } fn empty(values: ArrayView<i64>) { return values.distinct().is_empty() && values.reverse().is_empty() && values.slice(0, 0).is_empty(); } fn invalid(values: ArrayView<i64>) { return values.slice(0, 9); }",
    );

    let values = vec![3_i64, 5, 3, 8];
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call("numeric", args, CallOptions::unbounded())
        .expect("borrowed array transforms should return owned projected results");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
    drop(result);
    assert_eq!(values, vec![3, 5, 3, 8]);

    let owner = CollectionOwner {
        values: vec![5, 7, 11],
        totals: BTreeMap::new(),
    };
    let result = runtime
        .call(
            "retained",
            CallArgs::new().with_host_ref("owner", &owner),
            CallOptions::unbounded(),
        )
        .expect("retained borrowed array transforms should use the parent lease");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(18)));
    drop(result);
    assert_eq!(owner.values, vec![5, 7, 11]);

    let words = vec!["north".to_owned(), "star".to_owned()];
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &words);
    let result = runtime
        .call("joined", args, CallOptions::unbounded())
        .expect("borrowed string arrays should join projected strings");
    assert_eq!(
        runtime.value_to_owned(&result),
        Ok(OwnedValue::String("north|star".to_owned()))
    );
    drop(result);

    let empty = Vec::<i64>::new();
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &empty);
    let result = runtime
        .call("empty", args, CallOptions::unbounded())
        .expect("empty borrowed transforms should preserve owned array semantics");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
    drop(result);

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let error = runtime
        .call("invalid", args, CallOptions::unbounded())
        .expect_err("borrowed slice past the projected length must fail");
    assert!(matches!(
        error.kind(),
        VmErrorKind::IndexOutOfBounds { index: 9, len: 4 }
    ));
}

#[test]
fn borrowed_array_searches_use_one_bounded_projection() {
    let mut runtime = runtime(
        "fn direct(values: ArrayView<i64>) { return values.contains(12) && !values.contains(14) && values.index_of(13).unwrap_or(9) == 2 && values.index_of(14).unwrap_or(4) == 4; } fn retained(owner: CollectionOwner) { let values = owner.values(); return values.contains(7) && values.index_of(11).unwrap_or(0) == 2; } fn empty(values: ArrayView<i64>) { return !values.contains(1) && values.index_of(1).unwrap_or(6) == 6; }",
    );

    let values = vec![11_i64, 12, 13];
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call("direct", args, CallOptions::unbounded())
        .expect("borrowed array searches should use exact projected values");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
    drop(result);

    let owner = CollectionOwner {
        values: vec![5, 7, 11],
        totals: BTreeMap::new(),
    };
    let result = runtime
        .call(
            "retained",
            CallArgs::new().with_host_ref("owner", &owner),
            CallOptions::unbounded(),
        )
        .expect("retained borrowed array searches should use the parent lease");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
    drop(result);

    let values = Vec::<i64>::new();
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call("empty", args, CallOptions::unbounded())
        .expect("empty borrowed array searches should preserve false/None semantics");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
}

#[test]
fn borrowed_array_search_charges_projected_length() {
    let mut runtime = runtime("fn contains(values: ArrayView<i64>) { return values.contains(9); }");
    let base_limit = (0..64)
        .find(|limit| {
            let values = Vec::<i64>::new();
            let mut args = CallArgs::new();
            args.push_collection_ref("values", &values);
            runtime
                .call(
                    "contains",
                    args,
                    CallOptions::new(*limit, usize::MAX, usize::MAX),
                )
                .is_ok()
        })
        .expect("empty borrowed array search should fit a small bounded call");

    let values = vec![3_i64, 5, 9];
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    assert!(
        runtime
            .call(
                "contains",
                args,
                CallOptions::new(base_limit + 2, usize::MAX, usize::MAX),
            )
            .is_err(),
        "three projected elements must cost three execution units"
    );

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call(
            "contains",
            args,
            CallOptions::new(base_limit + 3, usize::MAX, usize::MAX),
        )
        .expect("the exact projection budget should allow the search");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
}

#[test]
fn borrowed_collection_projections_feed_iterator_pipelines() {
    let mut runtime = runtime(
        "fn array_iter(values: ArrayView<i64>) { return values.iter().filter(|value| value >= 6).count() + values.values().count(); } fn map_iter(scores: MapView<i32, i64>) { return scores.keys().count() + scores.entries().count() + scores.values().filter(|value| value >= 6).count(); } fn set_iter(values: SetView<i32>) { return values.iter().filter(|value| value >= 7i32).count() + values.values().count(); }",
    );

    let array = vec![4_i64, 6_i64, 11_i64];
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &array);
    let result = runtime
        .call("array_iter", args, CallOptions::unbounded())
        .expect("borrowed array projections should feed iterator methods");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(5)));
    drop(result);

    let scores = BTreeMap::from([(3_i32, 4_i64), (7_i32, 6_i64), (9_i32, 11_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_ref("scores", &scores);
    let result = runtime
        .call("map_iter", args, CallOptions::unbounded())
        .expect("borrowed map projections should feed iterator methods");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(8)));
    drop(result);

    let values = BTreeSet::from([3_i32, 7_i32, 9_i32]);
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call("set_iter", args, CallOptions::unbounded())
        .expect("borrowed set projections should feed iterator methods");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(5)));
}

#[test]
fn borrowed_collection_callbacks_reuse_owned_collection_semantics() {
    let mut runtime = runtime(
        "fn array_callbacks(values: ArrayView<i64>) { let filtered = values.filter(|value| value >= 6); let groups = values.group_by(|value| if value % 2 == 0 { \"even\" } else { \"odd\" }); return filtered.sum() + groups[\"even\"].sum() + groups[\"odd\"].len(); } fn map_callbacks(scores: MapView<i32, i64>) { return scores.filter(|key, value| key >= 7i32 && value >= 6).values().collect_array().sum(); } fn set_callbacks(values: SetView<i32>) { return values.filter(|value| value >= 7i32).map(|value| value + 1i32).len(); }",
    );

    let array = vec![4_i64, 6_i64, 11_i64];
    for _ in 0..2 {
        let mut args = CallArgs::new();
        args.push_collection_ref("values", &array);
        let result = runtime
            .call("array_callbacks", args, CallOptions::unbounded())
            .expect("borrowed array filter and group_by should reuse callback execution");
        assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(28)));
        drop(result);
    }

    let scores = BTreeMap::from([(3_i32, 4_i64), (7_i32, 6_i64), (9_i32, 11_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_ref("scores", &scores);
    let result = runtime
        .call("map_callbacks", args, CallOptions::unbounded())
        .expect("borrowed map filter should preserve key/value callback semantics");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(17)));
    drop(result);

    let values = BTreeSet::from([3_i32, 7_i32, 9_i32]);
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call("set_callbacks", args, CallOptions::unbounded())
        .expect("borrowed set callbacks should return ordinary owned sets");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(2)));
}
