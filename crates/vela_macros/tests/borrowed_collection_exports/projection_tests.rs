use super::*;

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
