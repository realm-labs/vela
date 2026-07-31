use super::*;

#[test]
fn dead_scoped_collection_iterators_block_await_until_explicit_release() {
    let mut runtime = runtime(concat!(
        "async fn blocked(values: ArrayView<i64>, totals: MapMut<String, i64>) { ",
        "{ let cursor = values.iter(); cursor.next(); } ",
        "return collections::merge_async(values, totals).await; }\n",
        "async fn allowed(values: ArrayView<i64>, totals: MapMut<String, i64>) { ",
        "let cursor = values.iter(); cursor.next(); host::release(cursor); ",
        "return collections::merge_async(values, totals).await; }",
    ));
    let values = vec![2_i64, 3];
    let mut totals = BTreeMap::<String, i64>::new();
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values)
        .push_collection_mut("totals", &mut totals);
    let mut future = Box::pin(runtime.call_async("blocked", args, CallOptions::unbounded()));
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    let error = match std::future::Future::poll(future.as_mut(), &mut context) {
        std::task::Poll::Ready(result) => {
            result.expect_err("a dead iterator local must remain in the active resource table")
        }
        std::task::Poll::Pending => panic!("the scoped-resource gate must run before polling"),
    };
    drop(future);
    assert!(matches!(
        error.kind(),
        VmErrorKind::Host(vela_host::error::HostErrorKind::UnreleasedScopedResourcesAtAwait { .. })
    ));
    assert!(totals.is_empty());

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values)
        .push_collection_mut("totals", &mut totals);
    let mut future = Box::pin(runtime.call_async("allowed", args, CallOptions::unbounded()));
    let result = loop {
        match std::future::Future::poll(future.as_mut(), &mut context) {
            std::task::Poll::Ready(result) => {
                break result.expect("explicit iterator release should permit await");
            }
            std::task::Poll::Pending => continue,
        }
    };
    drop(future);
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(5)));
    drop(result);
    assert_eq!(totals["sum"], 5);
}

#[test]
fn generated_async_adapters_hold_collection_leases_to_completion() {
    let mut runtime = runtime(
        "async fn free(values: ArrayView<i64>, totals: MapMut<String, i64>) { return collections::merge_async(values, totals).await; } async fn method(service: CollectionService, totals: MapMut<String, i64>) { return service.add_async(totals, 3).await; }",
    );
    let values = vec![2_i64, 3];
    let mut totals = BTreeMap::<String, i64>::new();
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values)
        .push_collection_mut("totals", &mut totals);
    let mut future = Box::pin(runtime.call_async("free", args, CallOptions::unbounded()));
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    let result = loop {
        match std::future::Future::poll(future.as_mut(), &mut context) {
            std::task::Poll::Ready(result) => {
                break result.expect("async free collection adapter should complete");
            }
            std::task::Poll::Pending => continue,
        }
    };
    drop(future);
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(5)));
    drop(result);
    assert_eq!(totals["sum"], 5);

    let service = CollectionService { offset: 4 };
    let mut args = CallArgs::new();
    args.push_host_ref("service", &service)
        .push_collection_mut("totals", &mut totals);
    let mut future = Box::pin(runtime.call_async("method", args, CallOptions::unbounded()));
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    let result = loop {
        match std::future::Future::poll(future.as_mut(), &mut context) {
            std::task::Poll::Ready(result) => {
                break result.expect("async method collection adapter should complete");
            }
            std::task::Poll::Pending => continue,
        }
    };
    drop(future);
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(12)));
    drop(result);
    assert_eq!(totals["sum"], 12);
}

#[test]
fn generated_async_slice_adapter_holds_the_slice_lease_to_completion() {
    let mut runtime = runtime(
        "async fn main(service: CollectionService, values: ArrayView<i64>) { let direct = collections::slice_sum_async(values).await; return direct + service.slice_sum_async(values).await; }",
    );
    let service = CollectionService { offset: 4 };
    let values = [2_i64, 3, 5];
    let mut args = CallArgs::new();
    args.push_host_ref("service", &service)
        .push_slice_ref("values", &values);
    let mut future = Box::pin(runtime.call_async("main", args, CallOptions::unbounded()));
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    let result = loop {
        match std::future::Future::poll(future.as_mut(), &mut context) {
            std::task::Poll::Ready(result) => {
                break result.expect("async slice adapter should complete");
            }
            std::task::Poll::Pending => continue,
        }
    };
    drop(future);
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(24)));
}
