use super::*;

#[test]
fn scoped_iterator_transform_results_must_be_named_and_retained() {
    let engine = collection_engine();
    for (source, expected) in [
        (
            "fn bad(values: ArrayView<i64>) { let cursor = values.iter(); cursor.filter(|value| value > 0); }",
            "compiler::discarded_scoped_resource",
        ),
        (
            "fn bad(values: ArrayView<i64>) { let cursor = values.iter(); return cursor.filter(|value| value > 0).count(); }",
            "compiler::unnameable_scoped_resource",
        ),
        (
            "fn bad(values: ArrayView<i64>) { let source = values.iter(); let mapped = source.map(|value| value); mapped.take(1); }",
            "compiler::discarded_scoped_resource",
        ),
    ] {
        let error = engine
            .compile_source(source)
            .expect_err("a transferred scoped iterator must stay authored and nameable");
        let diagnostic = match error.kind {
            vela_engine::source::EngineSourceErrorKind::Backend(error) => error
                .to_diagnostic()
                .expect("scoped iterator errors should expose diagnostics"),
            other => panic!("expected backend diagnostic, found {other:?}"),
        };
        assert_eq!(diagnostic.code.as_deref(), Some(expected));
        assert!(diagnostic.message.contains("ScopedIterator"));
    }
}

#[test]
fn scoped_host_iterator_release_is_explicit_idempotent_and_transfers_through_pipelines() {
    let mut runtime = runtime(concat!(
        "fn count(values: ArrayView<i64>) { let source = values.iter(); ",
        "let selected = source.filter(|value| value >= 5); ",
        "let count = selected.count(); let first = host::try_release(selected); ",
        "let second = host::try_release(selected); return [count, first, second]; }\n",
        "fn release_then_mutate(values: ArrayMut<i64>) { let cursor = values.iter(); ",
        "let first = cursor.next().unwrap_or(0); host::release(cursor); ",
        "values[0] = 11; return first + values[0]; }\n",
        "fn wrong_order(owner: CollectionOwner) { let values = owner.values_mut(); ",
        "let cursor = values.iter(); host::release(values); host::release(cursor); }\n",
        "fn use_after_release(values: ArrayView<i64>) { let cursor = values.iter(); ",
        "host::release(cursor); return cursor.next(); }",
    ));

    let values = vec![2_i64, 5, 7];
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call("count", args, CallOptions::unbounded())
        .expect("a transformed scoped iterator should remain explicitly releasable");
    assert_eq!(
        runtime.value_to_owned(&result),
        Ok(OwnedValue::array([
            OwnedValue::i64(2),
            OwnedValue::Bool(true),
            OwnedValue::Bool(false),
        ]))
    );
    drop(result);

    let mut values = vec![3_i64, 5];
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut values);
    let result = runtime
        .call("release_then_mutate", args, CallOptions::unbounded())
        .expect("releasing the iterator should restore parent mutation access");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(14)));
    drop(result);
    assert_eq!(values, vec![11, 5]);

    let mut owner = CollectionOwner {
        values: vec![3_i64, 5],
        totals: BTreeMap::new(),
    };
    let error = runtime
        .call(
            "wrong_order",
            CallArgs::new().with_host_mut("owner", &mut owner),
            CallOptions::unbounded(),
        )
        .expect_err("a parent view must not release before its scoped iterator child");
    assert!(matches!(
        error.kind(),
        VmErrorKind::Host(vela_host::error::HostErrorKind::BorrowStillInUse { .. })
    ));
    assert_eq!(owner.values, vec![3, 5]);

    let values = vec![3_i64, 5];
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let error = runtime
        .call("use_after_release", args, CallOptions::unbounded())
        .expect_err("strict release must invalidate the iterator immediately");
    assert!(matches!(
        error.kind(),
        VmErrorKind::Host(vela_host::error::HostErrorKind::ExpiredBorrowedHostRef { .. })
    ));
}

#[test]
fn scoped_map_iterators_freeze_parent_access_until_release() {
    let mut runtime = runtime(concat!(
        "fn read_then_mutate(scores: MapMut<i32, i64>) { let cursor = scores.values(); ",
        "let first = cursor.next().unwrap_or(0); let second = cursor.next().unwrap_or(0); ",
        "host::release(cursor); scores.set(3i32, 10); scores.set(7i32, 20); ",
        "scores.set(9i32, 30); return first + second; }\n",
        "fn pipeline(scores: MapMut<i32, i64>) { let source = scores.entries(); ",
        "let mapped = source.map(|entry| entry.value); let result = mapped.collect_array().sum(); ",
        "host::release(mapped); scores.set(3i32, 11); scores.set(7i32, 13); return result; }\n",
        "fn mutate_while_live(scores: MapMut<i32, i64>) { let cursor = scores.values(); ",
        "cursor.next(); scores.remove(7i32); host::release(cursor); }\n",
        "fn string_entry(scores: MapMut<String, i64>) { let cursor = scores.entries(); ",
        "let result = cursor.next().unwrap_or(MapEntry { key: \"\", value: 0 }).value; ",
        "host::release(cursor); scores.set(\"a\", 13); return result; }\n",
        "fn escape(scores: MapView<i32, i64>) { let source = scores.values(); ",
        "let mapped = source.map(|value| value); let taken = mapped.take(1); return taken; }",
    ));

    let mut scores = BTreeMap::from([(3_i32, 2_i64), (7_i32, 3_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_mut("scores", &mut scores);
    let result = runtime
        .call("read_then_mutate", args, CallOptions::unbounded())
        .expect("releasing a Map iterator should restore mutation access");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(5)));
    drop(result);
    assert_eq!(
        scores,
        BTreeMap::from([(3_i32, 10_i64), (7_i32, 20_i64), (9_i32, 30_i64)])
    );

    let mut scores = std::collections::HashMap::from([(3_i32, 2_i64), (7_i32, 3_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_mut("scores", &mut scores);
    let result = runtime
        .call("read_then_mutate", args, CallOptions::unbounded())
        .expect("HashMap should share explicit scoped iterator release");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(5)));
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
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(12)));
    drop(result);
    assert_eq!(scores, BTreeMap::from([(3, 11), (7, 13)]));

    let mut scores = BTreeMap::from([(3_i32, 5_i64), (7_i32, 7_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_mut("scores", &mut scores);
    runtime
        .call("mutate_while_live", args, CallOptions::unbounded())
        .expect_err("a live Map iterator must freeze parent mutation");
    assert_eq!(scores, BTreeMap::from([(3, 5), (7, 7)]));

    let mut scores = BTreeMap::from([("a".to_owned(), 2_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_mut("scores", &mut scores);
    let result = runtime
        .call("string_entry", args, CallOptions::unbounded())
        .expect("heap-backed String keys should survive explicit iterator release");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(2)));
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
fn scoped_set_iterators_freeze_parent_access_until_release() {
    let mut runtime = runtime(concat!(
        "fn frozen(values: SetMut<i32>) { let cursor = values.iter(); ",
        "let count = cursor.count(); host::release(cursor); values.insert(9i32); return count; }\n",
        "fn pipeline(values: SetMut<i32>) { let source = values.values(); ",
        "let selected = source.filter(|value| value >= 7i32); let count = selected.count(); ",
        "host::release(selected); return count; }\n",
        "fn mutate_while_live(values: SetMut<i32>) { let cursor = values.iter(); ",
        "cursor.next(); values.remove(7i32); host::release(cursor); }\n",
        "fn string_count(values: SetMut<String>) { let cursor = values.iter(); ",
        "let count = cursor.count(); host::release(cursor); return count; }\n",
        "fn escape(values: SetView<i32>) { let source = values.iter(); ",
        "let mapped = source.map(|value| value); let taken = mapped.take(1); return taken; }",
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
        .call("mutate_while_live", args, CallOptions::unbounded())
        .expect_err("a live Set iterator must freeze parent mutation");
    assert_eq!(values, BTreeSet::from([3, 7]));

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
        "fn map_count(values: MapView<i32, i64>) { let cursor = values.values(); let count = cursor.count(); host::release(cursor); return count; }\n",
        "fn set_count(values: SetView<i32>) { let cursor = values.iter(); let count = cursor.count(); host::release(cursor); return count; }",
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
