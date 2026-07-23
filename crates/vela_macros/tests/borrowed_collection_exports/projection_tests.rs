use super::*;

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
