use super::linked_standard_method_cache_fixtures::*;
use super::linked_standard_method_cache_support::*;
use super::*;

#[test]
fn linked_standard_value_method_caches_string_no_arg_transform_targets() {
    assert_string_no_arg_transform_cache(
        "to_lower",
        "WOLF",
        StandardMethodInlineCacheTarget::ToLower,
        "wolf",
    );
    assert_string_no_arg_transform_cache(
        "trim",
        "  wolf  ",
        StandardMethodInlineCacheTarget::Trim,
        "wolf",
    );
    assert_string_no_arg_transform_cache(
        "trim_start",
        "  wolf",
        StandardMethodInlineCacheTarget::TrimStart,
        "wolf",
    );
    assert_string_no_arg_transform_cache(
        "trim_end",
        "wolf  ",
        StandardMethodInlineCacheTarget::TrimEnd,
        "wolf",
    );
}

fn assert_string_no_arg_transform_cache(
    method: &str,
    receiver: &str,
    target: StandardMethodInlineCacheTarget,
    expected: &str,
) {
    let (program, site, dispatch, method_id) = linked_string_no_arg_cache_program(method, receiver);
    let caches = RecordingMethodCaches::new(1);
    let expected = OwnedValue::String(expected.to_owned());

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(expected.clone())
    );
    let entry = caches
        .entry(site)
        .expect("standard string transform cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard string transform cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::String);
    assert_eq!(standard_method.target, target);
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(expected)
    );
    assert_eq!(caches.set_count(), 2);
}
