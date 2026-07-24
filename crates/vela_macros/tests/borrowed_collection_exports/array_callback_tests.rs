use super::*;

#[test]
fn borrowed_array_callbacks_read_values_live_between_callback_resumes() {
    let mut runtime = runtime(concat!(
        "fn filter_live(values: ArrayMut<i64>) { ",
        "let selected = values.filter(|value| { ",
        "if value == 5 { values[1] = 13; } ",
        "return value >= 10; }); ",
        "return selected.sum() * 10 + selected.len(); }\n",
        "fn group_live(values: ArrayMut<i64>) { ",
        "let grouped = values.group_by(|value| { ",
        "if value == 5 { values[1] = 12; } ",
        "if value % 2 == 0 { return \"even\"; } return \"odd\"; }); ",
        "return grouped[\"even\"].sum() * 10 + grouped[\"odd\"].sum(); }",
    ));

    let mut values = vec![5_i64, 7_i64];
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut values);
    let result = runtime
        .call("filter_live", args, CallOptions::unbounded())
        .expect("borrowed Array filter should read each value immediately before its callback");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(131)));
    drop(result);
    assert_eq!(values, vec![5, 13]);

    let mut values = vec![5_i64, 7_i64];
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut values);
    let result = runtime
        .call("group_live", args, CallOptions::unbounded())
        .expect("borrowed Array group_by should consume the same live prepared traversal");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(125)));
    drop(result);
    assert_eq!(values, vec![5, 12]);
}

#[test]
fn borrowed_array_callbacks_charge_only_the_live_prefix_they_consume() {
    let mut runtime =
        runtime("fn any(values: ArrayView<i64>) { return values.any(|value| value == 9); }");
    let minimum_limit = |runtime: &mut Runtime, values: &Vec<i64>| {
        (0..512)
            .find(|limit| {
                let mut args = CallArgs::new();
                args.push_collection_ref("values", values);
                runtime
                    .call(
                        "any",
                        args,
                        CallOptions::new(*limit, usize::MAX, usize::MAX),
                    )
                    .is_ok()
            })
            .expect("borrowed Array callback should fit the bounded budget search")
    };

    let first_match = vec![9_i64];
    let large_first_match = std::iter::once(9_i64)
        .chain(std::iter::repeat_n(1_i64, 127))
        .collect();
    assert_eq!(
        minimum_limit(&mut runtime, &first_match),
        minimum_limit(&mut runtime, &large_first_match),
        "a short-circuiting callback must not materialize or charge the unread suffix",
    );

    let empty = Vec::<i64>::new();
    let three_misses = vec![1_i64, 3, 5];
    assert_eq!(
        minimum_limit(&mut runtime, &three_misses) - minimum_limit(&mut runtime, &empty),
        15,
        "each consumed host value keeps one read unit and four callback units",
    );
}
