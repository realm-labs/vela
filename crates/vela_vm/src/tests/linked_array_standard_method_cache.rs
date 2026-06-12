use super::linked_standard_method_cache_fixtures::*;
use super::linked_standard_method_cache_support::*;
use super::*;
use crate::value::Value as RuntimeValue;

#[test]
fn linked_standard_value_method_caches_array_contains_target() {
    assert_array_bool_cache(
        linked_array_contains_cache_program(),
        StandardMethodInlineCacheTarget::Contains,
    );
}

#[test]
fn linked_standard_value_method_caches_array_first_target() {
    assert_array_option_scalar_cache(
        linked_array_first_cache_program(),
        StandardMethodInlineCacheTarget::First,
        2,
    );
}

#[test]
fn linked_standard_value_method_caches_array_last_target() {
    assert_array_option_scalar_cache(
        linked_array_last_cache_program(),
        StandardMethodInlineCacheTarget::Last,
        4,
    );
}

#[test]
fn linked_standard_value_method_caches_array_index_of_target() {
    assert_array_option_scalar_cache(
        linked_array_index_of_cache_program(),
        StandardMethodInlineCacheTarget::IndexOf,
        1,
    );
}

#[test]
fn linked_standard_value_method_caches_array_slice_target() {
    assert_array_owned_cache(
        linked_array_slice_cache_program(),
        StandardMethodInlineCacheTarget::Slice,
        OwnedValue::Array(vec![
            OwnedValue::Scalar(vela_common::ScalarValue::I64(4)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(6)),
        ]),
    );
}

#[test]
fn linked_standard_value_method_caches_array_reverse_target() {
    assert_array_owned_cache(
        linked_array_reverse_cache_program(),
        StandardMethodInlineCacheTarget::Reverse,
        OwnedValue::Array(vec![
            OwnedValue::Scalar(vela_common::ScalarValue::I64(6)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(4)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
        ]),
    );
}

#[test]
fn linked_standard_value_method_caches_array_distinct_target() {
    assert_array_owned_cache(
        linked_array_distinct_cache_program(),
        StandardMethodInlineCacheTarget::Distinct,
        OwnedValue::Array(vec![
            OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(4)),
        ]),
    );
}

fn assert_array_bool_cache(
    fixture: LinkedMethodCacheFixture,
    target: StandardMethodInlineCacheTarget,
) {
    let (program, site, dispatch, method_id) = fixture;
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::Bool(true))
    );
    assert_array_cache_entry(&caches, site, dispatch, method_id, target);
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::Bool(true))
    );
    assert_eq!(caches.set_count(), 2);
}

fn assert_array_option_scalar_cache(
    fixture: LinkedMethodCacheFixture,
    target: StandardMethodInlineCacheTarget,
    expected: i64,
) {
    let (program, site, dispatch, method_id) = fixture;
    let caches = RecordingMethodCaches::new(1);
    let expected = Ok(owned_option_some(OwnedValue::Scalar(
        vela_common::ScalarValue::I64(expected),
    )));

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        expected
    );
    assert_array_cache_entry(&caches, site, dispatch, method_id, target);
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        expected
    );
    assert_eq!(caches.set_count(), 2);
}

fn assert_array_owned_cache(
    fixture: LinkedMethodCacheFixture,
    target: StandardMethodInlineCacheTarget,
    expected: OwnedValue,
) {
    let (program, site, dispatch, method_id) = fixture;
    let caches = RecordingMethodCaches::new(1);
    let expected = Ok(expected);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        expected
    );
    assert_array_cache_entry(&caches, site, dispatch, method_id, target);
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        expected
    );
    assert_eq!(caches.set_count(), 2);
}

fn assert_array_cache_entry(
    caches: &RecordingMethodCaches,
    site: CacheSiteId,
    dispatch: vela_bytecode::MethodDispatchHandle,
    method_id: MethodId,
    target: StandardMethodInlineCacheTarget,
) {
    let entry = caches
        .entry(site)
        .expect("standard array cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard array cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::Array);
    assert_eq!(standard_method.target, target);
}
