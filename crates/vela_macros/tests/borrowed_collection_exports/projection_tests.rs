use super::*;

#[test]
fn untyped_dynamic_borrowed_collections_discover_supported_standard_methods() {
    let mut runtime = runtime(concat!(
        "fn array(values) { return values.max().unwrap_or(0) ",
        "+ values.filter(|value| value >= 5).len(); }\n",
        "fn map(scores) { if !scores.contains_key(7i32) || scores.contains_key(8i32) ",
        "{ return 0; } return scores.get_or(7i32, 0) ",
        "+ scores.filter(|key, value| key >= 7i32 && value >= 6).values().count(); }\n",
        "fn set(values) { return values.contains(7i32) && !values.contains(8i32) ",
        "&& values.has(9i32) && values.filter(|value| value >= 7i32).len() == 2; }\n",
        "fn grow(values) { values.push(13); return values.pop().unwrap_or(0); }\n",
        "fn mutate_map(values) { values.set(9i32, 13); ",
        "return values.get_or(9i32, 0); }\n",
        "fn mutate_set(values) { return values.insert(11i32); }\n",
        "fn reject_fixed(values) { values.push(13); }\n",
    ));

    let array = vec![3_i64, 5, 8];
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &array);
    let result = runtime
        .call("array", args, CallOptions::unbounded())
        .expect("dynamic borrowed arrays should discover read/callback methods");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(10)));
    drop(result);

    let scores = BTreeMap::from([(3_i32, 4_i64), (7_i32, 6_i64), (9_i32, 11_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_ref("scores", &scores);
    let result = runtime
        .call("map", args, CallOptions::unbounded())
        .expect("dynamic borrowed maps should discover lookup/callback methods");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(8)));
    drop(result);

    let values = BTreeSet::from([3_i32, 7_i32, 9_i32]);
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call("set", args, CallOptions::unbounded())
        .expect("dynamic borrowed sets should discover lookup/callback methods");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
    drop(result);

    let mut growable = vec![3_i64, 5];
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut growable);
    let result = runtime
        .call("grow", args, CallOptions::unbounded())
        .expect("dynamic exclusive growable arrays should discover mutators");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(13)));
    drop(result);
    assert_eq!(growable, vec![3, 5]);

    let mut map = BTreeMap::from([(3_i32, 5_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut map);
    let result = runtime
        .call("mutate_map", args, CallOptions::unbounded())
        .expect("dynamic exclusive growable maps should discover mutators");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(13)));
    drop(result);
    assert_eq!(map, BTreeMap::from([(3, 5), (9, 13)]));

    let mut set = BTreeSet::from([3_i32, 7]);
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut set);
    let result = runtime
        .call("mutate_set", args, CallOptions::unbounded())
        .expect("dynamic exclusive growable sets should discover mutators");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
    drop(result);
    assert_eq!(set, BTreeSet::from([3, 7, 11]));

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &array);
    let error = runtime
        .call("grow", args, CallOptions::unbounded())
        .expect_err("exclusive dynamic resolution must not be cached for a shared receiver");
    assert!(matches!(
        error.kind(),
        VmErrorKind::UnknownMethod { method } if method == "push"
    ));

    let mut fixed = [3_i64, 5];
    let mut args = CallArgs::new();
    args.push_slice_mut("values", fixed.as_mut_slice());
    let error = runtime
        .call("reject_fixed", args, CallOptions::unbounded())
        .expect_err("dynamic fixed arrays must not discover growable mutators");
    assert!(matches!(
        error.kind(),
        VmErrorKind::UnknownMethod { method } if method == "push"
    ));
    assert_eq!(fixed, [3, 5]);
}

#[test]
fn borrowed_array_get_uses_one_live_index_read() {
    let mut runtime = runtime(concat!(
        "fn direct(values: ArrayView<i64>) { ",
        "return values.get(1).unwrap_or(0) + values.get(9).unwrap_or(4); }\n",
        "fn retained(owner: CollectionOwner) { ",
        "return owner.values().get(2).unwrap_or(0); }\n",
        "fn mutable(values: ArrayMut<i64>) { return values.get(1).unwrap_or(0); }\n",
        "fn dynamic(values) { return values.get(0).unwrap_or(0); }\n",
        "fn invalid(values: ArrayView<i64>) { return values.get(-1); }",
    ));

    let values = vec![11_i64, 13, 17];
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call("direct", args, CallOptions::unbounded())
        .expect("borrowed Array get should return Some or None through HostAccess");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(17)));
    drop(result);

    let owner = CollectionOwner {
        values: vec![3, 5, 8],
        totals: BTreeMap::new(),
    };
    let result = runtime
        .call(
            "retained",
            CallArgs::new().with_host_ref("owner", &owner),
            CallOptions::unbounded(),
        )
        .expect("retained borrowed Array get should use its parent lease");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(8)));
    drop(result);

    let mut fixed = [19_i64, 23];
    let mut args = CallArgs::new();
    args.push_slice_mut("values", fixed.as_mut_slice());
    let result = runtime
        .call("mutable", args, CallOptions::unbounded())
        .expect("fixed exclusive Array views should retain shared get behavior");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(23)));
    drop(result);
    assert_eq!(fixed, [19, 23]);

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call("dynamic", args, CallOptions::unbounded())
        .expect("dynamic borrowed Arrays should discover get");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(11)));
    drop(result);

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let error = runtime
        .call("invalid", args, CallOptions::unbounded())
        .expect_err("borrowed Array get should reject negative indexes");
    assert!(matches!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "host collection lookup"
        }
    ));
}

#[test]
fn borrowed_array_get_cost_is_independent_of_collection_length() {
    let mut runtime = runtime(
        "fn get(values: ArrayView<i64>, index: i64) { return values.get(index).unwrap_or(-1); }",
    );
    let base_limit = (0..64)
        .find(|limit| {
            let values = vec![13_i64];
            let mut args = CallArgs::new();
            args.push_collection_ref("values", &values);
            args.push_value("index", 0_i64);
            runtime
                .call(
                    "get",
                    args,
                    CallOptions::new(*limit, usize::MAX, usize::MAX),
                )
                .is_ok()
        })
        .expect("one borrowed Array index read should fit a small bounded call");

    let values = (0_i64..256).collect::<Vec<_>>();
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    args.push_value("index", 255_i64);
    let result = runtime
        .call(
            "get",
            args,
            CallOptions::new(base_limit, usize::MAX, usize::MAX),
        )
        .expect("Array get must not charge or snapshot the full collection");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(255)));
}

#[test]
fn borrowed_map_merge_uses_one_bounded_projection() {
    let mut runtime = runtime(concat!(
        "fn merge(values: MapView<i32, i64>) {\n",
        "  let patch = [MapEntry { key: 7i32, value: 13 }, ",
        "MapEntry { key: 9i32, value: 17 }].iter().collect_map();\n",
        "  let merged = values.merge(patch);\n",
        "  return merged.len() == 3 && merged[3i32] == 5 ",
        "&& merged[7i32] == 13 && merged[9i32] == 17 && values[7i32] == 11;\n",
        "}\n",
        "fn empty(values: MapView<i32, i64>) {\n",
        "  let patch = [MapEntry { key: 7i32, value: 13 }].iter().collect_map();\n",
        "  let merged = values.merge(patch);\n",
        "  return merged.len() == 1 && merged[7i32] == 13;\n",
        "}\n",
        "fn dynamic(values) {\n",
        "  let patch = [MapEntry { key: 7i32, value: 19 }].iter().collect_map();\n",
        "  return values.merge(patch)[7i32];\n",
        "}\n",
        "fn wrong(values) { return values.merge([13]); }",
    ));

    let values = BTreeMap::from([(3_i32, 5_i64), (7_i32, 11_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call("merge", args, CallOptions::unbounded())
        .expect("borrowed Map merge should return an owned Map");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
    drop(result);
    assert_eq!(values, BTreeMap::from([(3, 5), (7, 11)]));

    let empty = BTreeMap::<i32, i64>::new();
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &empty);
    let result = runtime
        .call("empty", args, CallOptions::unbounded())
        .expect("empty borrowed Maps should preserve owned merge semantics");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
    drop(result);

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call("dynamic", args, CallOptions::unbounded())
        .expect("dynamic borrowed Maps should discover merge");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(19)));
    drop(result);

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let error = runtime
        .call("wrong", args, CallOptions::unbounded())
        .expect_err("borrowed Map merge must reject a non-Map operand");
    assert!(matches!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "method merge"
        }
    ));
}

#[test]
fn borrowed_map_get_or_insert_writes_only_missing_entries() {
    let mut runtime = runtime(concat!(
        "fn direct(scores: MapMut<i32, i64>) { ",
        "let existing = scores.get_or_insert(7i32, 99); ",
        "let inserted = scores.get_or_insert(9i32, 13); ",
        "return existing * 100 + inserted; }\n",
        "fn retained(owner: CollectionOwner) { let totals = owner.totals_mut(); ",
        "return totals.get_or_insert(\"sum\", 17); }\n",
        "fn dynamic(scores) { return scores.get_or_insert(5i32, 11); }\n",
        "fn existing_invalid(scores) { return scores.get_or_insert(7i32, \"bad\"); }\n",
        "fn invalid(scores) { return scores.get_or_insert(9i32, \"bad\"); }",
    ));

    let mut scores = BTreeMap::from([(7_i32, 5_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_mut("scores", &mut scores);
    let result = runtime
        .call("direct", args, CallOptions::unbounded())
        .expect("MapMut get_or_insert should preserve or insert through HostAccess");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(513)));
    drop(result);
    assert_eq!(scores, BTreeMap::from([(7, 5), (9, 13)]));

    let mut owner = CollectionOwner {
        values: Vec::new(),
        totals: BTreeMap::new(),
    };
    let result = runtime
        .call(
            "retained",
            CallArgs::new().with_host_mut("owner", &mut owner),
            CallOptions::unbounded(),
        )
        .expect("retained MapMut get_or_insert should use its parent lease");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(17)));
    drop(result);
    assert_eq!(owner.totals, BTreeMap::from([("sum".to_owned(), 17)]));

    let shared = BTreeMap::from([(5_i32, 7_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_ref("scores", &shared);
    let error = runtime
        .call("dynamic", args, CallOptions::unbounded())
        .expect_err("a shared dynamic Map view must not discover get_or_insert");
    assert!(matches!(
        error.kind(),
        VmErrorKind::UnknownMethod { method } if method == "get_or_insert"
    ));
    assert_eq!(shared, BTreeMap::from([(5, 7)]));

    let mut existing = BTreeMap::from([(7_i32, 5_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_mut("scores", &mut existing);
    let result = runtime
        .call("existing_invalid", args, CallOptions::unbounded())
        .expect("an existing entry must not convert the unused default");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(5)));
    drop(result);
    assert_eq!(existing, BTreeMap::from([(7, 5)]));

    let mut unchanged = BTreeMap::from([(7_i32, 5_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_mut("scores", &mut unchanged);
    runtime
        .call("invalid", args, CallOptions::unbounded())
        .expect_err("an invalid inserted value must fail before host mutation");
    assert_eq!(unchanged, BTreeMap::from([(7, 5)]));
}

#[test]
fn borrowed_map_merge_charges_projected_length() {
    let mut runtime = runtime(concat!(
        "fn merge(values: MapView<i32, i64>) { ",
        "return values.merge([].iter().collect_map()).len(); }",
    ));
    let base_limit = (0..64)
        .find(|limit| {
            let values = BTreeMap::<i32, i64>::new();
            let mut args = CallArgs::new();
            args.push_collection_ref("values", &values);
            runtime
                .call(
                    "merge",
                    args,
                    CallOptions::new(*limit, usize::MAX, usize::MAX),
                )
                .is_ok()
        })
        .expect("empty borrowed Map merge should fit a small bounded call");

    let values = BTreeMap::from([(3_i32, 5_i64), (7_i32, 11_i64), (9_i32, 17_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    assert!(
        runtime
            .call(
                "merge",
                args,
                CallOptions::new(base_limit + 2, usize::MAX, usize::MAX),
            )
            .is_err(),
        "three projected Map entries must cost three execution units"
    );

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call(
            "merge",
            args,
            CallOptions::new(base_limit + 3, usize::MAX, usize::MAX),
        )
        .expect("the exact Map projection budget should succeed");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(3)));
}

#[test]
fn borrowed_set_algebra_uses_one_bounded_projection() {
    let mut runtime = runtime(concat!(
        "fn combine(values: SetView<i32>) {\n",
        "  let other = set::from_array([2i32, 4i32, 5i32]);\n",
        "  let unioned = values.union(other);\n",
        "  let shared = values.intersection(other);\n",
        "  let left = values.difference(other);\n",
        "  let changed = values.symmetric_difference(other);\n",
        "  return unioned.len() == 5 && unioned.has(4i32)\n",
        "    && shared.len() == 2 && shared.has(2i32) && shared.has(5i32)\n",
        "    && left.len() == 2 && left.has(1i32) && left.has(3i32)\n",
        "    && changed.len() == 3 && changed.has(1i32)\n",
        "    && changed.has(3i32) && changed.has(4i32);\n",
        "}\n",
        "fn relations(values: SetView<i32>) {\n",
        "  return values.is_subset(set::from_array([1i32, 2i32, 3i32, 5i32, 8i32]))\n",
        "    && values.is_superset(set::from_array([1i32, 3i32]))\n",
        "    && values.is_disjoint(set::from_array([8i32, 13i32]));\n",
        "}\n",
        "fn empty(values: SetView<i32>) {\n",
        "  let other = set::from_array([7i32]);\n",
        "  return values.union(other).len() == 1\n",
        "    && values.intersection(other).is_empty()\n",
        "    && values.difference(other).is_empty()\n",
        "    && values.symmetric_difference(other).has(7i32);\n",
        "}\n",
        "fn dynamic(values) { return values.union(set::from_array([8i32])).len(); }\n",
        "fn wrong(values) { return values.union([8i32]); }",
    ));

    let values = BTreeSet::from([1_i32, 2, 3, 5]);
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call("combine", args, CallOptions::unbounded())
        .expect("borrowed Set combinations should return owned Set values");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
    drop(result);
    assert_eq!(values, BTreeSet::from([1, 2, 3, 5]));

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call("relations", args, CallOptions::unbounded())
        .expect("borrowed Set relations should reuse owned Set semantics");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
    drop(result);

    let empty = BTreeSet::<i32>::new();
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &empty);
    let result = runtime
        .call("empty", args, CallOptions::unbounded())
        .expect("empty borrowed Sets should preserve owned algebra semantics");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
    drop(result);

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call("dynamic", args, CallOptions::unbounded())
        .expect("dynamic borrowed Sets should discover implemented algebra methods");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(5)));
    drop(result);

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let error = runtime
        .call("wrong", args, CallOptions::unbounded())
        .expect_err("borrowed Set algebra must reject a non-Set operand");
    assert!(matches!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "method union"
        }
    ));
}

#[test]
fn borrowed_set_algebra_charges_projected_length() {
    let mut runtime = runtime(
        "fn union(values: SetView<i32>) { return values.union(set::from_array([])).len(); }",
    );
    let base_limit = (0..64)
        .find(|limit| {
            let values = BTreeSet::<i32>::new();
            let mut args = CallArgs::new();
            args.push_collection_ref("values", &values);
            runtime
                .call(
                    "union",
                    args,
                    CallOptions::new(*limit, usize::MAX, usize::MAX),
                )
                .is_ok()
        })
        .expect("empty borrowed Set algebra should fit a small bounded call");

    let values = BTreeSet::from([3_i32, 5, 8]);
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    assert!(
        runtime
            .call(
                "union",
                args,
                CallOptions::new(base_limit + 2, usize::MAX, usize::MAX),
            )
            .is_err(),
        "three projected Set elements must cost three execution units"
    );

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call(
            "union",
            args,
            CallOptions::new(base_limit + 3, usize::MAX, usize::MAX),
        )
        .expect("the exact Set projection budget should succeed");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(3)));
}

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

#[test]
fn borrowed_collection_retain_callbacks_write_through_transactionally() {
    let mut runtime = runtime(
        "fn array_retain(values) { values.retain(|value| value % 2 == 0); return values.len(); } fn returned_array_retain(owner: CollectionOwner) { let values = owner.values_mut(); values.retain(|value| value >= 7); return values.len(); } fn map_retain(scores: MapMut<i32, i64>) { scores.retain(|key, value| key >= 7i32 && value >= 6); return scores.len(); } fn set_retain(values: SetMut<i32>) { values.retain(|value| value >= 7i32); return values.len(); } fn array_retain_error(values) { values.retain(|value| collections::retain_non_two(value)); }",
    );

    let mut array = vec![2_i64, 3, 4, 5];
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut array);
    let result = runtime
        .call("array_retain", args, CallOptions::unbounded())
        .expect("borrowed Array retain should write through one completed callback mask");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(2)));
    drop(result);
    assert_eq!(array, vec![2, 4]);

    let shared = vec![2_i64, 3, 4];
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &shared);
    let error = runtime
        .call("array_retain", args, CallOptions::unbounded())
        .expect_err("a shared dynamic Array view must not discover retain");
    assert!(matches!(
        error.kind(),
        VmErrorKind::UnknownMethod { method } if method == "retain"
    ));
    assert_eq!(shared, vec![2, 3, 4]);

    let mut fixed = [2_i64, 3, 4];
    let mut args = CallArgs::new();
    args.push_slice_mut("values", fixed.as_mut_slice());
    let error = runtime
        .call("array_retain", args, CallOptions::unbounded())
        .expect_err("a fixed-length dynamic Array view must not discover retain");
    assert!(matches!(
        error.kind(),
        VmErrorKind::UnknownMethod { method } if method == "retain"
    ));
    assert_eq!(fixed, [2, 3, 4]);

    let mut owner = CollectionOwner {
        values: vec![5, 7, 11],
        totals: BTreeMap::new(),
    };
    let result = runtime
        .call(
            "returned_array_retain",
            CallArgs::new().with_host_mut("owner", &mut owner),
            CallOptions::unbounded(),
        )
        .expect("a retained mutable collection view should keep its parent lease for write-back");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(2)));
    drop(result);
    assert_eq!(owner.values, vec![7, 11]);

    let mut scores = BTreeMap::from([(3_i32, 4_i64), (7_i32, 6_i64), (9_i32, 11_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_mut("scores", &mut scores);
    let result = runtime
        .call("map_retain", args, CallOptions::unbounded())
        .expect("borrowed Map retain should write through selected keys");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(2)));
    drop(result);
    assert_eq!(scores, BTreeMap::from([(7_i32, 6_i64), (9_i32, 11_i64)]));

    let mut values = BTreeSet::from([3_i32, 7_i32, 9_i32]);
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut values);
    let result = runtime
        .call("set_retain", args, CallOptions::unbounded())
        .expect("borrowed Set retain should write through selected keys");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(2)));
    drop(result);
    assert_eq!(values, BTreeSet::from([7_i32, 9_i32]));

    let mut unchanged = vec![1_i64, 2, 3];
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut unchanged);
    runtime
        .call("array_retain_error", args, CallOptions::unbounded())
        .expect_err("a callback error must occur before retain mutates Rust state");
    assert_eq!(unchanged, vec![1, 2, 3]);
}

#[test]
fn growable_borrowed_collections_extend_from_borrowed_sources() {
    let mut runtime = runtime(
        "fn array_extend(target, source: ArrayView<i64>) { target.extend(source); return target.len(); } fn map_extend(target: MapMut<i32, i64>, source: MapView<i32, i64>) { target.extend(source); return target.len(); } fn set_extend(target: SetMut<i32>, source: SetView<i32>) { target.extend(source); return target.len(); } fn self_extend(owner: CollectionOwner) { let values = owner.values_mut(); values.extend(values); return values.len(); } fn wrong_extend(target, source) { target.extend(source); }",
    );

    let mut array_target = vec![2_i64];
    let array_source = vec![3_i64, 5];
    let mut args = CallArgs::new();
    args.push_collection_mut("target", &mut array_target);
    args.push_collection_ref("source", &array_source);
    let result = runtime
        .call("array_extend", args, CallOptions::unbounded())
        .expect("borrowed Array source should extend one growable host target");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(3)));
    drop(result);
    assert_eq!(array_target, vec![2, 3, 5]);
    assert_eq!(array_source, vec![3, 5]);

    let mut map_target = BTreeMap::from([(2_i32, 3_i64)]);
    let map_source = BTreeMap::from([(2_i32, 5_i64), (7_i32, 11_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_mut("target", &mut map_target);
    args.push_collection_ref("source", &map_source);
    let result = runtime
        .call("map_extend", args, CallOptions::unbounded())
        .expect("borrowed Map source should extend one growable host target");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(2)));
    drop(result);
    assert_eq!(map_target, BTreeMap::from([(2, 5), (7, 11)]));
    assert_eq!(map_source, BTreeMap::from([(2, 5), (7, 11)]));

    let mut set_target = BTreeSet::from([2_i32]);
    let set_source = BTreeSet::from([3_i32, 5]);
    let mut args = CallArgs::new();
    args.push_collection_mut("target", &mut set_target);
    args.push_collection_ref("source", &set_source);
    let result = runtime
        .call("set_extend", args, CallOptions::unbounded())
        .expect("borrowed Set source should extend one growable host target");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(3)));
    drop(result);
    assert_eq!(set_target, BTreeSet::from([2, 3, 5]));
    assert_eq!(set_source, BTreeSet::from([3, 5]));

    let mut owner = CollectionOwner {
        values: vec![2, 3],
        totals: BTreeMap::new(),
    };
    let result = runtime
        .call(
            "self_extend",
            CallArgs::new().with_host_mut("owner", &mut owner),
            CallOptions::unbounded(),
        )
        .expect("one exclusive HostRef alias should snapshot before extending itself");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(4)));
    drop(result);
    assert_eq!(owner.values, vec![2, 3, 2, 3]);

    let mut wrong_target = vec![2_i32];
    let wrong_source = BTreeSet::from([3_i32, 5]);
    let mut args = CallArgs::new();
    args.push_collection_mut("target", &mut wrong_target);
    args.push_collection_ref("source", &wrong_source);
    runtime
        .call("wrong_extend", args, CallOptions::unbounded())
        .expect_err("a borrowed source from another collection protocol must be rejected");
    assert_eq!(wrong_target, vec![2]);
}

#[test]
fn borrowed_source_extend_precharges_projection_and_mutation() {
    let mut runtime =
        runtime("fn extend(target, source: ArrayView<i64>) { target.extend(source); }");
    let base_limit = (0..96)
        .find(|limit| {
            let mut target = vec![1_i64];
            let source = Vec::<i64>::new();
            let mut args = CallArgs::new();
            args.push_collection_mut("target", &mut target);
            args.push_collection_ref("source", &source);
            runtime
                .call(
                    "extend",
                    args,
                    CallOptions::new(*limit, usize::MAX, usize::MAX),
                )
                .is_ok()
        })
        .expect("empty borrowed-source extension should fit a bounded call");

    let source = vec![2_i64, 3, 5];
    let mut target = vec![1_i64];
    let mut args = CallArgs::new();
    args.push_collection_mut("target", &mut target);
    args.push_collection_ref("source", &source);
    runtime
        .call(
            "extend",
            args,
            CallOptions::new(base_limit + 5, usize::MAX, usize::MAX),
        )
        .expect_err("source projection plus target mutation must each charge three units");
    assert_eq!(target, vec![1], "budget failure must precede host mutation");

    let mut args = CallArgs::new();
    args.push_collection_mut("target", &mut target);
    args.push_collection_ref("source", &source);
    runtime
        .call(
            "extend",
            args,
            CallOptions::new(base_limit + 6, usize::MAX, usize::MAX),
        )
        .expect("the exact two-traversal budget should extend the host target");
    assert_eq!(target, vec![1, 2, 3, 5]);
    assert_eq!(source, vec![2, 3, 5]);
}
