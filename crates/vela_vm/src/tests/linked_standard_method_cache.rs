use super::linked_standard_method_cache_fixtures::*;
use super::*;
use crate::value::Value as RuntimeValue;

#[test]
fn linked_standard_value_method_populates_readonly_inline_cache() {
    let (program, site, dispatch, method_id) = linked_standard_len_cache_program();
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::i64(4))
    );
    let entry = caches
        .entry(site)
        .expect("standard method cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    assert_eq!(caches.set_count(), 2);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard method cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::String);
    assert_eq!(standard_method.target, StandardMethodInlineCacheTarget::Len);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::i64(4))
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_refreshes_wrong_receiver_guard() {
    let (program, site, dispatch, method_id) = linked_standard_len_cache_program();
    let caches = RecordingMethodCaches::new(1);
    let debug_name = program
        .method_dispatch(dispatch)
        .expect("dispatch should exist")
        .debug_name;
    caches.prime(
        site,
        MethodInlineCacheEntry {
            dispatch,
            debug_name,
            target: MethodInlineCacheTarget::Value {
                method_id,
                standard_method: Some(StandardMethodInlineCacheEntry {
                    receiver: StandardMethodReceiver::Array,
                    target: StandardMethodInlineCacheTarget::Len,
                }),
            },
        },
    );

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::i64(4))
    );
    let entry = caches
        .entry(site)
        .expect("standard method cache should refresh");
    let MethodInlineCacheTarget::Value {
        standard_method: Some(standard_method),
        ..
    } = entry.target
    else {
        panic!("standard method cache should store refreshed value target");
    };
    assert_eq!(standard_method.receiver, StandardMethodReceiver::String);
    assert_eq!(standard_method.target, StandardMethodInlineCacheTarget::Len);
    assert_eq!(caches.set_count(), 1);
}

#[test]
fn linked_standard_value_method_caches_predicate_target() {
    let (program, site, dispatch, method_id) = linked_string_contains_cache_program();
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::Bool(true))
    );
    let entry = caches
        .entry(site)
        .expect("standard predicate cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard predicate cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::String);
    assert_eq!(
        standard_method.target,
        StandardMethodInlineCacheTarget::Contains
    );
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::Bool(true))
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_bytes_accessor_target() {
    let (program, site, dispatch, method_id) = linked_bytes_get_cache_program();
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::Scalar(vela_common::ScalarValue::U8(21)))
    );
    let entry = caches
        .entry(site)
        .expect("standard bytes accessor cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard bytes accessor cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::Bytes);
    assert_eq!(standard_method.target, StandardMethodInlineCacheTarget::Get);
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::Scalar(vela_common::ScalarValue::U8(21)))
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_bytes_slice_target() {
    let (program, site, dispatch, method_id) = linked_bytes_slice_cache_program();
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(OwnedValue::Bytes(vec![21, 34]))
    );
    let entry = caches
        .entry(site)
        .expect("standard bytes slice cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard bytes slice cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::Bytes);
    assert_eq!(
        standard_method.target,
        StandardMethodInlineCacheTarget::Slice
    );
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(OwnedValue::Bytes(vec![21, 34]))
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_bytes_to_hex_target() {
    let (program, site, dispatch, method_id) = linked_bytes_to_hex_cache_program();
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(OwnedValue::String("0d1522".to_owned()))
    );
    let entry = caches
        .entry(site)
        .expect("standard bytes to_hex cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard bytes to_hex cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::Bytes);
    assert_eq!(
        standard_method.target,
        StandardMethodInlineCacheTarget::ToHex
    );
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(OwnedValue::String("0d1522".to_owned()))
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_option_predicate_target() {
    let (program, site, dispatch, method_id) = linked_option_is_some_cache_program();
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::Bool(true))
    );
    let entry = caches
        .entry(site)
        .expect("standard option cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard option cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::Option);
    assert_eq!(
        standard_method.target,
        StandardMethodInlineCacheTarget::IsSome
    );
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::Bool(true))
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_result_unwrap_or_target() {
    let (program, site, dispatch, method_id) = linked_result_unwrap_or_cache_program();
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::i64(17))
    );
    let entry = caches
        .entry(site)
        .expect("standard result cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard result cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::Result);
    assert_eq!(
        standard_method.target,
        StandardMethodInlineCacheTarget::UnwrapOr
    );
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::i64(17))
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_map_get_or_target() {
    let (program, site, dispatch, method_id) = linked_map_get_or_cache_program();
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::i64(8))
    );
    let entry = caches
        .entry(site)
        .expect("standard map get_or cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard map get_or cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::Map);
    assert_eq!(
        standard_method.target,
        StandardMethodInlineCacheTarget::GetOr
    );
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::i64(8))
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_string_transform_target() {
    let (program, site, dispatch, method_id) =
        linked_string_no_arg_cache_program("to_upper", "wolf");
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(OwnedValue::String("WOLF".to_owned()))
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
    assert_eq!(
        standard_method.target,
        StandardMethodInlineCacheTarget::ToUpper
    );
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(OwnedValue::String("WOLF".to_owned()))
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_string_parse_targets() {
    assert_string_no_arg_option_cache(
        "parse_int",
        "42",
        StandardMethodInlineCacheTarget::ParseInt,
        OwnedValue::Scalar(vela_common::ScalarValue::I64(42)),
    );
    assert_string_no_arg_option_cache(
        "parse_float",
        "1.5",
        StandardMethodInlineCacheTarget::ParseFloat,
        OwnedValue::Scalar(vela_common::ScalarValue::F64(1.5)),
    );
    assert_string_no_arg_option_cache(
        "parse_bool",
        "true",
        StandardMethodInlineCacheTarget::ParseBool,
        OwnedValue::Bool(true),
    );
}

#[test]
fn linked_standard_value_method_caches_string_char_at_target() {
    let (program, site, dispatch, method_id) = linked_string_one_constant_arg_cache_program(
        "char_at",
        "quest",
        Constant::Scalar(vela_common::ScalarValue::I64(1)),
    );
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(owned_option_some(OwnedValue::String("u".to_owned())))
    );
    let entry = caches
        .entry(site)
        .expect("standard string char_at cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard string char_at cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::String);
    assert_eq!(
        standard_method.target,
        StandardMethodInlineCacheTarget::CharAt
    );
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(owned_option_some(OwnedValue::String("u".to_owned())))
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_string_split_once_target() {
    let (program, site, dispatch, method_id) =
        linked_string_one_arg_cache_program("split_once", "count=3", "=");
    let caches = RecordingMethodCaches::new(1);
    let expected = owned_option_some(OwnedValue::Array(vec![
        OwnedValue::String("count".to_owned()),
        OwnedValue::String("3".to_owned()),
    ]));

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(expected.clone())
    );
    let entry = caches
        .entry(site)
        .expect("standard string split_once cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard string split_once cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::String);
    assert_eq!(
        standard_method.target,
        StandardMethodInlineCacheTarget::SplitOnce
    );
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(expected)
    );
    assert_eq!(caches.set_count(), 2);
}

fn assert_string_no_arg_option_cache(
    method: &str,
    receiver: &str,
    target: StandardMethodInlineCacheTarget,
    expected_payload: OwnedValue,
) {
    let (program, site, dispatch, method_id) = linked_string_no_arg_cache_program(method, receiver);
    let caches = RecordingMethodCaches::new(1);
    let expected = owned_option_some(expected_payload);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(expected.clone())
    );
    let entry = caches
        .entry(site)
        .expect("standard string parse cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard string parse cache should store value target");
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

#[test]
fn linked_standard_value_method_caches_string_find_target() {
    let (program, site, dispatch, method_id) =
        linked_string_one_arg_cache_program("find", "daily_quest", "quest");
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(owned_option_some(OwnedValue::Scalar(
            vela_common::ScalarValue::I64(6)
        )))
    );
    let entry = caches
        .entry(site)
        .expect("standard string find cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard string find cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::String);
    assert_eq!(
        standard_method.target,
        StandardMethodInlineCacheTarget::Find
    );
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(owned_option_some(OwnedValue::Scalar(
            vela_common::ScalarValue::I64(6)
        )))
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_string_strip_prefix_target() {
    let (program, site, dispatch, method_id) =
        linked_string_one_arg_cache_program("strip_prefix", "event:quest", "event:");
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(owned_option_some(OwnedValue::String("quest".to_owned())))
    );
    let entry = caches
        .entry(site)
        .expect("standard string strip_prefix cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard string strip_prefix cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::String);
    assert_eq!(
        standard_method.target,
        StandardMethodInlineCacheTarget::StripPrefix
    );
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(owned_option_some(OwnedValue::String("quest".to_owned())))
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_string_strip_suffix_target() {
    let (program, site, dispatch, method_id) =
        linked_string_one_arg_cache_program("strip_suffix", "quest.done", ".done");
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(owned_option_some(OwnedValue::String("quest".to_owned())))
    );
    let entry = caches
        .entry(site)
        .expect("standard string strip_suffix cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard string strip_suffix cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::String);
    assert_eq!(
        standard_method.target,
        StandardMethodInlineCacheTarget::StripSuffix
    );
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(owned_option_some(OwnedValue::String("quest".to_owned())))
    );
    assert_eq!(caches.set_count(), 2);
}
