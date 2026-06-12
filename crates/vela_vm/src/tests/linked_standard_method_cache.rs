use super::linked_standard_method_cache_fixtures::*;
use super::linked_standard_method_cache_support::*;
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
    assert_string_predicate_cache(
        linked_string_contains_cache_program(),
        StandardMethodInlineCacheTarget::Contains,
    );
    assert_string_predicate_cache(
        linked_string_one_arg_cache_program("starts_with", "event:quest", "event:"),
        StandardMethodInlineCacheTarget::StartsWith,
    );
    assert_string_predicate_cache(
        linked_string_one_arg_cache_program("ends_with", "quest.done", ".done"),
        StandardMethodInlineCacheTarget::EndsWith,
    );
}

fn assert_string_predicate_cache(
    fixture: LinkedMethodCacheFixture,
    target: StandardMethodInlineCacheTarget,
) {
    let (program, site, dispatch, method_id) = fixture;
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
    assert_eq!(standard_method.target, target);
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::Bool(true))
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_collection_membership_targets() {
    assert_membership_cache(
        linked_array_contains_cache_program(),
        StandardMethodReceiver::Array,
        StandardMethodInlineCacheTarget::Contains,
    );
    assert_membership_cache(
        linked_map_has_cache_program(),
        StandardMethodReceiver::Map,
        StandardMethodInlineCacheTarget::Has,
    );
    assert_membership_cache(
        linked_set_has_cache_program(),
        StandardMethodReceiver::Set,
        StandardMethodInlineCacheTarget::Has,
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
    let entry = caches
        .entry(site)
        .expect("standard array option-scalar cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard array option-scalar cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::Array);
    assert_eq!(standard_method.target, target);
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        expected
    );
    assert_eq!(caches.set_count(), 2);
}

fn assert_membership_cache(
    fixture: LinkedMethodCacheFixture,
    receiver: StandardMethodReceiver,
    target: StandardMethodInlineCacheTarget,
) {
    let (program, site, dispatch, method_id) = fixture;
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_program_with_standard_natives(&program, &caches),
        Ok(RuntimeValue::Bool(true))
    );
    let entry = caches
        .entry(site)
        .expect("standard collection membership cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard collection membership cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, receiver);
    assert_eq!(standard_method.target, target);
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_program_with_standard_natives(&program, &caches),
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
fn linked_standard_value_method_caches_bytes_endian_read_targets() {
    assert_bytes_read_cache(
        "read_u32_le",
        StandardMethodInlineCacheTarget::ReadU32Le,
        0x0403_0201,
    );
    assert_bytes_read_cache(
        "read_u32_be",
        StandardMethodInlineCacheTarget::ReadU32Be,
        0x0102_0304,
    );
}

fn assert_bytes_read_cache(method: &str, target: StandardMethodInlineCacheTarget, expected: u32) {
    let (program, site, dispatch, method_id) = linked_bytes_read_u32_cache_program(method);
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::Scalar(vela_common::ScalarValue::U32(
            expected
        )))
    );
    let entry = caches
        .entry(site)
        .expect("standard bytes endian read cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard bytes endian read cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::Bytes);
    assert_eq!(standard_method.target, target);
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::Scalar(vela_common::ScalarValue::U32(
            expected
        )))
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
    assert_enum_predicate_cache(
        linked_option_is_some_cache_program(),
        StandardMethodReceiver::Option,
        StandardMethodInlineCacheTarget::IsSome,
        true,
    );
    assert_enum_predicate_cache(
        linked_option_predicate_cache_program("is_none"),
        StandardMethodReceiver::Option,
        StandardMethodInlineCacheTarget::IsNone,
        false,
    );
}

#[test]
fn linked_standard_value_method_caches_result_predicate_targets() {
    assert_enum_predicate_cache(
        linked_result_predicate_cache_program("is_ok"),
        StandardMethodReceiver::Result,
        StandardMethodInlineCacheTarget::IsOk,
        false,
    );
    assert_enum_predicate_cache(
        linked_result_predicate_cache_program("is_err"),
        StandardMethodReceiver::Result,
        StandardMethodInlineCacheTarget::IsErr,
        true,
    );
}

fn assert_enum_predicate_cache(
    fixture: LinkedMethodCacheFixture,
    receiver: StandardMethodReceiver,
    target: StandardMethodInlineCacheTarget,
    expected: bool,
) {
    let (program, site, dispatch, method_id) = fixture;
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::Bool(expected))
    );
    let entry = caches
        .entry(site)
        .expect("standard enum predicate cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard enum predicate cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, receiver);
    assert_eq!(standard_method.target, target);
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::Bool(expected))
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
fn linked_standard_value_method_caches_map_get_target() {
    let (program, site, dispatch, method_id) = linked_map_get_cache_program();
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(owned_option_some(OwnedValue::Scalar(
            vela_common::ScalarValue::I64(8)
        )))
    );
    let entry = caches
        .entry(site)
        .expect("standard map get cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard map get cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::Map);
    assert_eq!(standard_method.target, StandardMethodInlineCacheTarget::Get);
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(owned_option_some(OwnedValue::Scalar(
            vela_common::ScalarValue::I64(8)
        )))
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_set_relation_targets() {
    assert_set_relation_cache(
        "is_subset",
        &[2],
        &[2, 4],
        StandardMethodInlineCacheTarget::IsSubset,
    );
    assert_set_relation_cache(
        "is_superset",
        &[2, 4],
        &[2],
        StandardMethodInlineCacheTarget::IsSuperset,
    );
    assert_set_relation_cache(
        "is_disjoint",
        &[2],
        &[4],
        StandardMethodInlineCacheTarget::IsDisjoint,
    );
}

fn assert_set_relation_cache(
    method: &str,
    receiver_values: &[i64],
    other_values: &[i64],
    target: StandardMethodInlineCacheTarget,
) {
    let (program, site, dispatch, method_id) =
        linked_set_relation_cache_program(method, receiver_values, other_values);
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_program_with_standard_natives(&program, &caches),
        Ok(RuntimeValue::Bool(true))
    );
    let entry = caches
        .entry(site)
        .expect("standard set relation cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard set relation cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::Set);
    assert_eq!(standard_method.target, target);
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_program_with_standard_natives(&program, &caches),
        Ok(RuntimeValue::Bool(true))
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
fn linked_standard_value_method_caches_string_split_target() {
    let (program, site, dispatch, method_id) =
        linked_string_one_arg_cache_program("split", "alpha,beta", ",");
    let caches = RecordingMethodCaches::new(1);
    let expected = OwnedValue::Array(vec![
        OwnedValue::String("alpha".to_owned()),
        OwnedValue::String("beta".to_owned()),
    ]);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(expected.clone())
    );
    let entry = caches
        .entry(site)
        .expect("standard string split cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard string split cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::String);
    assert_eq!(
        standard_method.target,
        StandardMethodInlineCacheTarget::Split
    );
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(expected)
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_string_slice_target() {
    let (program, site, dispatch, method_id) = linked_string_two_constant_arg_cache_program(
        "slice",
        "hello",
        Constant::Scalar(vela_common::ScalarValue::I64(1)),
        Constant::Scalar(vela_common::ScalarValue::I64(4)),
    );
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(OwnedValue::String("ell".to_owned()))
    );
    let entry = caches
        .entry(site)
        .expect("standard string slice cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard string slice cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::String);
    assert_eq!(
        standard_method.target,
        StandardMethodInlineCacheTarget::Slice
    );
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(OwnedValue::String("ell".to_owned()))
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_string_repeat_target() {
    let (program, site, dispatch, method_id) = linked_string_one_constant_arg_cache_program(
        "repeat",
        "ab",
        Constant::Scalar(vela_common::ScalarValue::I64(3)),
    );
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(OwnedValue::String("ababab".to_owned()))
    );
    let entry = caches
        .entry(site)
        .expect("standard string repeat cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard string repeat cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::String);
    assert_eq!(
        standard_method.target,
        StandardMethodInlineCacheTarget::Repeat
    );
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(OwnedValue::String("ababab".to_owned()))
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_string_replace_target() {
    let (program, site, dispatch, method_id) = linked_string_two_constant_arg_cache_program(
        "replace",
        "event.done",
        Constant::String(".".to_owned()),
        Constant::String("_".to_owned()),
    );
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(OwnedValue::String("event_done".to_owned()))
    );
    let entry = caches
        .entry(site)
        .expect("standard string replace cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard string replace cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::String);
    assert_eq!(
        standard_method.target,
        StandardMethodInlineCacheTarget::Replace
    );
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(OwnedValue::String("event_done".to_owned()))
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

#[test]
fn linked_standard_value_method_caches_string_split_materialization_targets() {
    assert_string_no_arg_owned_cache(
        "split_lines",
        "alpha\nbeta",
        StandardMethodInlineCacheTarget::SplitLines,
        OwnedValue::Array(vec![
            OwnedValue::String("alpha".to_owned()),
            OwnedValue::String("beta".to_owned()),
        ]),
    );
    assert_string_no_arg_owned_cache(
        "split_whitespace",
        "alpha beta",
        StandardMethodInlineCacheTarget::SplitWhitespace,
        OwnedValue::Array(vec![
            OwnedValue::String("alpha".to_owned()),
            OwnedValue::String("beta".to_owned()),
        ]),
    );
}

fn assert_string_no_arg_owned_cache(
    method: &str,
    receiver: &str,
    target: StandardMethodInlineCacheTarget,
    expected: OwnedValue,
) {
    let (program, site, dispatch, method_id) = linked_string_no_arg_cache_program(method, receiver);
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        Ok(expected.clone())
    );
    let entry = caches
        .entry(site)
        .expect("standard string materialization cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard string materialization cache should store value target");
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
