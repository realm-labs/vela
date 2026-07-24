use super::*;

#[test]
fn borrowed_array_searches_use_prepared_index_reads() {
    let mut runtime = runtime(
        "fn direct(values: ArrayView<i64>) { return values.contains(12) && !values.contains(14) && values.index_of(13).unwrap_or(9) == 2 && values.index_of(14).unwrap_or(4) == 4; } fn retained(owner: CollectionOwner) { let values = owner.values(); return values.contains(7) && values.index_of(11).unwrap_or(0) == 2; } fn empty(values: ArrayView<i64>) { return !values.contains(1) && values.index_of(1).unwrap_or(6) == 6; }",
    );

    let values = vec![11_i64, 12, 13];
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call("direct", args, CallOptions::unbounded())
        .expect("borrowed array searches should use prepared indexed reads");
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
fn borrowed_array_search_charges_only_scanned_elements() {
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

    let values = vec![9_i64, 5, 3];
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    assert!(
        runtime
            .call(
                "contains",
                args,
                CallOptions::new(base_limit, usize::MAX, usize::MAX),
            )
            .is_err(),
        "reading the first element must cost one execution unit"
    );

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call(
            "contains",
            args,
            CallOptions::new(base_limit + 1, usize::MAX, usize::MAX),
        )
        .expect("a first-element match should not charge the unscanned suffix");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));

    let values = vec![3_i64, 5, 7];
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
        "a missing value must charge every scanned element"
    );

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call(
            "contains",
            args,
            CallOptions::new(base_limit + 3, usize::MAX, usize::MAX),
        )
        .expect("the exact scan budget should allow a complete miss");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(false)));
}
