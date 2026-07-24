use super::*;

#[test]
fn borrowed_map_callbacks_read_live_values_over_frozen_keys() {
    let mut runtime = runtime(concat!(
        "fn filter_live(scores: MapMut<i32, i64>) { ",
        "let selected = scores.filter(|key, value| { ",
        "if key == 3i32 { scores.set(7i32, 13); scores.set(9i32, 17); } ",
        "return value >= 10; }); ",
        "return selected.values().collect_array().sum(); }",
    ));

    let mut scores = BTreeMap::from([(3_i32, 5_i64), (7_i32, 7_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_mut("scores", &mut scores);
    let result = runtime
        .call("filter_live", args, CallOptions::unbounded())
        .expect("borrowed Map filter should read each frozen key immediately before its callback");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(13)));
    drop(result);
    assert_eq!(scores, BTreeMap::from([(3, 5), (7, 13), (9, 17)]));

    let mut scores = std::collections::HashMap::from([(3_i32, 5_i64), (7_i32, 7_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_mut("scores", &mut scores);
    let result = runtime
        .call("filter_live", args, CallOptions::unbounded())
        .expect("HashMap callbacks should share the prepared Map traversal protocol");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(13)));
    drop(result);
    assert_eq!(
        scores,
        std::collections::HashMap::from([(3, 5), (7, 13), (9, 17)])
    );
}

#[test]
fn borrowed_set_callbacks_validate_frozen_members_between_resumes() {
    let mut runtime = runtime(concat!(
        "fn remove_pending(values: SetMut<i32>) { ",
        "return values.filter(|value| { ",
        "if value == 3i32 { values.remove(7i32); } return true; }); }",
    ));

    let mut values = BTreeSet::from([3_i32, 7_i32]);
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut values);
    runtime
        .call("remove_pending", args, CallOptions::unbounded())
        .expect_err("removing an unvisited frozen Set member should fail its callback read");
    assert_eq!(values, BTreeSet::from([3]));

    let mut values = std::collections::HashSet::from([3_i32, 7_i32]);
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut values);
    runtime
        .call("remove_pending", args, CallOptions::unbounded())
        .expect_err("HashSet callbacks should share frozen membership validation");
    assert_eq!(values, std::collections::HashSet::from([3]));
}

#[test]
fn borrowed_keyed_callbacks_charge_frozen_structure_and_live_reads() {
    let mut runtime = runtime(concat!(
        "fn map_any(values: MapView<i32, i64>) { ",
        "return values.any(|key, value| key == 3i32 && value == 9); }\n",
        "fn set_any(values: SetView<i32>) { return values.any(|value| value == 3i32); }",
    ));

    let map_limit = |runtime: &mut Runtime, values: &BTreeMap<i32, i64>| {
        (0..512)
            .find(|limit| {
                let mut args = CallArgs::new();
                args.push_collection_ref("values", values);
                runtime
                    .call(
                        "map_any",
                        args,
                        CallOptions::new(*limit, usize::MAX, usize::MAX),
                    )
                    .is_ok()
            })
            .expect("borrowed Map callback should fit the bounded budget search")
    };
    let empty_map = BTreeMap::new();
    let first_map = BTreeMap::from([(3_i32, 9_i64), (7, 11), (9, 13)]);
    assert_eq!(
        map_limit(&mut runtime, &first_map) - map_limit(&mut runtime, &empty_map),
        7,
        "Map any should add three frozen keys and one callback over the common live poll",
    );

    let set_limit = |runtime: &mut Runtime, values: &BTreeSet<i32>| {
        (0..512)
            .find(|limit| {
                let mut args = CallArgs::new();
                args.push_collection_ref("values", values);
                runtime
                    .call(
                        "set_any",
                        args,
                        CallOptions::new(*limit, usize::MAX, usize::MAX),
                    )
                    .is_ok()
            })
            .expect("borrowed Set callback should fit the bounded budget search")
    };
    let empty_set = BTreeSet::new();
    let first_set = BTreeSet::from([3_i32, 7, 9]);
    assert_eq!(
        set_limit(&mut runtime, &first_set) - set_limit(&mut runtime, &empty_set),
        7,
        "Set any should add three frozen members and one callback over the common live poll",
    );
}
