use super::*;

#[test]
fn borrowed_map_iterators_read_live_values_over_frozen_keys() {
    let mut runtime = runtime(concat!(
        "fn next_values(scores: MapMut<i32, i64>) { let cursor = scores.values(); ",
        "scores.set(3i32, 10); let first = cursor.next().unwrap_or(0); ",
        "scores.set(7i32, 20); scores.set(9i32, 30); return first ",
        "+ cursor.next().unwrap_or(0) + cursor.next().unwrap_or(7); }\n",
        "fn pipeline(scores: MapMut<i32, i64>) { let cursor = scores.entries(); ",
        "scores.set(3i32, 11); scores.set(7i32, 13); return cursor.map(|entry| entry.value)",
        ".collect_array().sum(); }\n",
        "fn remove_pending(scores: MapMut<i32, i64>) { let cursor = scores.values(); ",
        "cursor.next(); scores.remove(7i32); return cursor.next(); }\n",
        "fn string_entry(scores: MapMut<String, i64>) { let cursor = scores.entries(); ",
        "scores.set(\"a\", 13); return cursor.next()",
        ".unwrap_or(MapEntry { key: \"\", value: 0 }).value; }\n",
        "fn escape(scores) { return scores.values().map(|value| value).take(1); }",
    ));

    let mut scores = BTreeMap::from([(3_i32, 2_i64), (7_i32, 3_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_mut("scores", &mut scores);
    let result = runtime
        .call("next_values", args, CallOptions::unbounded())
        .expect("borrowed Map values should read each frozen key on demand");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(37)));
    drop(result);
    assert_eq!(
        scores,
        BTreeMap::from([(3_i32, 10_i64), (7_i32, 20_i64), (9_i32, 30_i64)])
    );

    let mut scores = std::collections::HashMap::from([(3_i32, 2_i64), (7_i32, 3_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_mut("scores", &mut scores);
    let result = runtime
        .call("next_values", args, CallOptions::unbounded())
        .expect("HashMap should share the prepared Map traversal protocol");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(37)));
    drop(result);
    assert_eq!(
        scores,
        std::collections::HashMap::from([(3, 10), (7, 20), (9, 30)])
    );

    let mut scores = BTreeMap::from([(3_i32, 5_i64), (7_i32, 7_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_mut("scores", &mut scores);
    let result = runtime
        .call("pipeline", args, CallOptions::unbounded())
        .expect("live Map entries should survive resumable callback pipelines");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(24)));
    drop(result);
    assert_eq!(scores, BTreeMap::from([(3, 11), (7, 13)]));

    let mut scores = BTreeMap::from([(3_i32, 5_i64), (7_i32, 7_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_mut("scores", &mut scores);
    runtime
        .call("remove_pending", args, CallOptions::unbounded())
        .expect_err("removing an unvisited frozen key should fail its live value read");
    assert_eq!(scores, BTreeMap::from([(3, 5)]));

    let mut scores = BTreeMap::from([("a".to_owned(), 2_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_mut("scores", &mut scores);
    let result = runtime
        .call("string_entry", args, CallOptions::unbounded())
        .expect("heap-backed String keys should remain rooted across live entry reads");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(13)));
    drop(result);
    assert_eq!(scores, BTreeMap::from([("a".to_owned(), 13)]));

    let scores = BTreeMap::from([(3_i32, 5_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_ref("scores", &scores);
    let result = runtime
        .call("escape", args, CallOptions::unbounded())
        .expect("creating a borrowed Map iterator should succeed within the host call");
    let error = runtime
        .value_to_owned(&result)
        .expect_err("a live Map iterator must not escape as an owned snapshot");
    assert!(matches!(
        error.kind(),
        VmErrorKind::TypeMismatch { operation }
            if operation == "host-backed iterator escape"
    ));
}

#[test]
fn borrowed_set_iterators_validate_frozen_values_on_each_step() {
    let mut runtime = runtime(concat!(
        "fn frozen(values: SetMut<i32>) { let cursor = values.iter(); ",
        "values.insert(9i32); return cursor.count(); }\n",
        "fn pipeline(values: SetMut<i32>) { return values.values()",
        ".filter(|value| value >= 7i32).count(); }\n",
        "fn remove_pending(values: SetMut<i32>) { let cursor = values.iter(); ",
        "cursor.next(); values.remove(7i32); return cursor.next(); }\n",
        "fn string_count(values: SetMut<String>) { return values.iter().count(); }\n",
        "fn escape(values) { return values.iter().map(|value| value).take(1); }",
    ));

    let mut values = BTreeSet::from([3_i32, 7_i32]);
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut values);
    let result = runtime
        .call("frozen", args, CallOptions::unbounded())
        .expect("new Set values should not extend an active frozen traversal");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(2)));
    drop(result);
    assert_eq!(values, BTreeSet::from([3, 7, 9]));

    let mut values = std::collections::HashSet::from([3_i32, 7_i32]);
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut values);
    let result = runtime
        .call("frozen", args, CallOptions::unbounded())
        .expect("HashSet should share the prepared Set traversal protocol");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(2)));
    drop(result);
    assert_eq!(values, std::collections::HashSet::from([3, 7, 9]));

    let mut values = BTreeSet::from([3_i32, 7_i32]);
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut values);
    let result = runtime
        .call("pipeline", args, CallOptions::unbounded())
        .expect("prepared Set traversal should survive resumable callback pipelines");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(1)));
    drop(result);

    let mut values = BTreeSet::from([3_i32, 7_i32]);
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut values);
    runtime
        .call("remove_pending", args, CallOptions::unbounded())
        .expect_err("removing an unvisited frozen Set value should invalidate its read");
    assert_eq!(values, BTreeSet::from([3]));

    let mut values = BTreeSet::from(["gold".to_owned(), "xp".to_owned()]);
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut values);
    let result = runtime
        .call("string_count", args, CallOptions::unbounded())
        .expect("heap-backed String values should remain rooted across Set membership reads");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(2)));
    drop(result);

    let values = BTreeSet::from([3_i32, 7_i32]);
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call("escape", args, CallOptions::unbounded())
        .expect("creating a borrowed Set iterator should succeed within the host call");
    let error = runtime
        .value_to_owned(&result)
        .expect_err("a live Set iterator must not escape as an owned snapshot");
    assert!(matches!(
        error.kind(),
        VmErrorKind::TypeMismatch { operation }
            if operation == "host-backed iterator escape"
    ));
}

#[test]
fn keyed_host_iterators_charge_projection_and_each_live_step() {
    let mut runtime = runtime(concat!(
        "fn map_count(values) { return values.values().count(); }\n",
        "fn set_count(values) { return values.iter().count(); }",
    ));

    let map_limit = |runtime: &mut Runtime, values: &BTreeMap<i32, i64>| {
        (0..256)
            .find(|limit| {
                let mut args = CallArgs::new();
                args.push_collection_ref("values", values);
                runtime
                    .call(
                        "map_count",
                        args,
                        CallOptions::new(*limit, usize::MAX, usize::MAX),
                    )
                    .is_ok()
            })
            .expect("borrowed Map iterator should fit the bounded budget search")
    };
    let empty_map = BTreeMap::new();
    let three_map = BTreeMap::from([(2_i32, 3_i64), (5, 7), (11, 13)]);
    assert_eq!(
        map_limit(&mut runtime, &three_map) - map_limit(&mut runtime, &empty_map),
        6,
        "Map traversal should charge three frozen keys and three live value reads",
    );

    let set_limit = |runtime: &mut Runtime, values: &BTreeSet<i32>| {
        (0..256)
            .find(|limit| {
                let mut args = CallArgs::new();
                args.push_collection_ref("values", values);
                runtime
                    .call(
                        "set_count",
                        args,
                        CallOptions::new(*limit, usize::MAX, usize::MAX),
                    )
                    .is_ok()
            })
            .expect("borrowed Set iterator should fit the bounded budget search")
    };
    let empty_set = BTreeSet::new();
    let three_set = BTreeSet::from([2_i32, 5, 11]);
    assert_eq!(
        set_limit(&mut runtime, &three_set) - set_limit(&mut runtime, &empty_set),
        6,
        "Set traversal should charge three frozen values and three membership reads",
    );
}
