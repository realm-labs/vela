use super::*;

#[test]
fn slice_alias_preflight_allows_shared_aliases_and_rejects_exclusive_conflicts() {
    let mut runtime = runtime(
        "fn shared(values: ArrayView<i64>) { return collections::slice_pair_sum(values, values); } fn conflict(values: ArrayMut<i64>) { return collections::slice_pair_mut(values, values); } fn recover(values: ArrayMut<i64>) { return collections::slice_bump(values); }",
    );
    let values = [2_i64, 3, 5];
    let mut args = CallArgs::new();
    args.push_slice_ref("values", &values);
    let result = runtime
        .call("shared", args, CallOptions::unbounded())
        .expect("two shared aliases should acquire coexisting leases");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(20)));
    drop(result);

    let mut values = [2_i64, 3, 5];
    let mut args = CallArgs::new();
    args.push_slice_mut("values", &mut values);
    let error = runtime
        .call("conflict", args, CallOptions::unbounded())
        .expect_err("two exclusive aliases must conflict before Rust reconstruction");
    assert!(matches!(
        error.kind(),
        VmErrorKind::AliasedMutableHostArguments { .. }
    ));
    assert_eq!(values, [2, 3, 5]);

    let mut args = CallArgs::new();
    args.push_slice_mut("values", &mut values);
    let result = runtime
        .call("recover", args, CallOptions::unbounded())
        .expect("failed preflight must release the exclusive slice lease");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(7)));
    drop(result);
    assert_eq!(values, [2, 7, 5]);
}

#[test]
fn slice_boundary_rejects_wrong_elements_and_accepts_empty_shared_and_mutable_slices() {
    let mut runtime = runtime(
        "fn read(values: ArrayView<i64>) { return collections::slice_sum(values); } fn write(values: ArrayMut<i64>) { return collections::slice_len_mut(values); }",
    );
    let wrong = [2_u64, 3, 5];
    let mut args = CallArgs::new();
    args.push_slice_ref("values", &wrong);
    let error = runtime
        .call("read", args, CallOptions::unbounded())
        .expect_err("a different concrete slice element type must fail closed");
    assert!(
        matches!(error.kind(), VmErrorKind::HostArgumentTypeMismatch { .. }),
        "unexpected wrong-element error: {error:?}"
    );

    let empty: [i64; 0] = [];
    let mut args = CallArgs::new();
    args.push_slice_ref("values", &empty);
    let result = runtime
        .call("read", args, CallOptions::unbounded())
        .expect("an empty shared slice must preserve its valid dangling pointer");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(0)));
    drop(result);

    let mut empty: [i64; 0] = [];
    let mut args = CallArgs::new();
    args.push_slice_mut("values", &mut empty);
    let result = runtime
        .call("write", args, CallOptions::unbounded())
        .expect("an empty exclusive slice must preserve its valid dangling pointer");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(0)));
}

#[test]
fn nested_native_reentry_reborrows_a_slice_under_the_parent_lease() {
    let mut runtime = runtime(
        "fn slice_nested(values: ArrayView<i64>) { return collections::slice_sum(values); } fn main(values: ArrayView<i64>) { return collections::slice_reenter(values); }",
    );
    let values = [2_i64, 3, 5];
    let mut args = CallArgs::new();
    args.push_slice_ref("values", &values);
    let result = runtime
        .call("main", args, CallOptions::unbounded())
        .expect("nested slice re-entry should complete");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(10)));
}

#[test]
fn slice_error_cleanup_leaves_runtime_and_exclusive_borrow_reusable() {
    let mut runtime = runtime(
        "fn fail(values: ArrayMut<i64>) { return collections::slice_fail(values); } fn recover(values: ArrayMut<i64>) { return collections::slice_bump(values); }",
    );
    let mut values = [2_i64, 3, 5];
    let mut args = CallArgs::new();
    args.push_slice_mut("values", &mut values);
    let error = runtime
        .call("fail", args, CallOptions::unbounded())
        .expect_err("the authored Rust error should propagate");
    assert!(matches!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "borrowed slice failure fixture"
        }
    ));
    assert_eq!(values, [3, 3, 5]);

    let mut args = CallArgs::new();
    args.push_slice_mut("values", &mut values);
    let result = runtime
        .call("recover", args, CallOptions::unbounded())
        .expect("returning a native error must release its slice lease");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(7)));
    drop(result);
    assert_eq!(values, [3, 7, 5]);
}
