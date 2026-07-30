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
fn suspended_slice_adapters_complete_or_cancel_with_all_leases_released() {
    let mut runtime = runtime(
        "async fn complete(values: ArrayView<i64>) { return collections::slice_yield_once(values).await; } async fn cancel(values: ArrayMut<i64>) { return collections::slice_wait(values).await; } fn recover(values: ArrayMut<i64>) { return collections::slice_bump(values); }",
    );
    let values = [2_i64, 3, 5];
    let mut args = CallArgs::new();
    args.push_slice_ref("values", &values);
    let mut future = Box::pin(runtime.call_async("complete", args, CallOptions::unbounded()));
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(matches!(
        std::future::Future::poll(future.as_mut(), &mut context),
        Poll::Pending
    ));
    let Poll::Ready(result) = std::future::Future::poll(future.as_mut(), &mut context) else {
        panic!("the second poll should complete the yielded slice future");
    };
    let result = result.expect("the yielded slice future should succeed");
    drop(future);
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(10)));
    drop(result);

    let mut values = [2_i64, 3, 5];
    let mut args = CallArgs::new();
    args.push_slice_mut("values", &mut values);
    let mut future = Box::pin(runtime.call_async("cancel", args, CallOptions::unbounded()));
    assert!(matches!(
        std::future::Future::poll(future.as_mut(), &mut context),
        Poll::Pending
    ));
    drop(future);
    assert_eq!(values, [3, 3, 5]);

    let mut args = CallArgs::new();
    args.push_slice_mut("values", &mut values);
    let result = runtime
        .call("recover", args, CallOptions::unbounded())
        .expect("dropping the pending future must release its slice lease");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(7)));
    drop(result);
    assert_eq!(values, [3, 7, 5]);
}

#[test]
fn slice_error_and_panic_cleanup_leave_runtime_and_exclusive_borrow_reusable() {
    let mut runtime = runtime(
        "fn fail(values: ArrayMut<i64>) { return collections::slice_fail(values); } async fn panic_async(values: ArrayMut<i64>) { return collections::slice_panic_async(values).await; } fn recover(values: ArrayMut<i64>) { return collections::slice_bump(values); }",
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
    let mut future = Box::pin(runtime.call_async("panic_async", args, CallOptions::unbounded()));
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = std::future::Future::poll(future.as_mut(), &mut context);
    }));
    assert!(panic.is_err());
    drop(future);
    assert_eq!(values, [4, 3, 5]);

    let mut args = CallArgs::new();
    args.push_slice_mut("values", &mut values);
    let result = runtime
        .call("recover", args, CallOptions::unbounded())
        .expect("unwinding the native future must release its slice lease");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(7)));
    drop(result);
    assert_eq!(values, [4, 7, 5]);
}

#[test]
fn suspended_slice_future_keeps_its_code_generation_until_completion() {
    SLICE_GATE_READY.store(false, Ordering::SeqCst);
    let engine = collection_engine();
    let initial = engine
        .compile_hot_reload_initial(
            "async fn main(values: ArrayView<i64>) { return (collections::slice_gate(values).await) + 1; } fn version() { return 1; }",
        )
        .expect("initial slice generation should compile");
    let update = engine
        .compile_hot_reload_update(
            &initial,
            "async fn main(values: ArrayView<i64>) { return (collections::slice_gate(values).await) + 100; } fn version() { return 2; }",
        )
        .expect("updated slice generation should compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial)
        .expect("hot-reload slice runtime should initialize");
    let staging = runtime
        .hot_reload_staging_handle()
        .expect("hot-reload runtime should expose its staging handle");
    let values = [2_i64, 3, 5];
    let mut args = CallArgs::new();
    args.push_slice_ref("values", &values);
    let mut future = Box::pin(runtime.call_async("main", args, CallOptions::unbounded()));
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(matches!(
        std::future::Future::poll(future.as_mut(), &mut context),
        Poll::Pending
    ));
    assert_eq!(staging.stage_reload_update(update), None);

    SLICE_GATE_READY.store(true, Ordering::SeqCst);
    let Poll::Ready(result) = std::future::Future::poll(future.as_mut(), &mut context) else {
        panic!("opening the slice gate should complete the old generation");
    };
    let result = result.expect("the old slice generation should complete");
    drop(future);
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(11)));
    drop(result);
    assert_eq!(
        runtime
            .hot_reload_version()
            .expect("the old generation should remain active until the safe point")
            .id
            .0,
        0
    );

    runtime
        .activate_reload()
        .expect("slice generation reload check should succeed")
        .expect("the staged generation should activate after completion");
    let mut args = CallArgs::new();
    args.push_slice_ref("values", &values);
    let mut future = Box::pin(runtime.call_async("main", args, CallOptions::unbounded()));
    let Poll::Ready(result) = std::future::Future::poll(future.as_mut(), &mut context) else {
        panic!("the open gate should let the new generation complete immediately");
    };
    let result = result.expect("the new slice generation should complete");
    drop(future);
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(110)));
}
