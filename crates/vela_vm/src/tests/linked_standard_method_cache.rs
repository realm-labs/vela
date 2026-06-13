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
fn linked_standard_value_method_caches_iterator_adapter_targets() {
    assert_iterator_adapter_cache(
        linked_iterator_adapter_cache_program("take"),
        StandardMethodInlineCacheTarget::Take,
        OwnedValue::Array(vec![
            OwnedValue::Scalar(vela_common::ScalarValue::I64(1)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
        ]),
    );
    assert_iterator_adapter_cache(
        linked_iterator_adapter_cache_program("skip"),
        StandardMethodInlineCacheTarget::Skip,
        OwnedValue::Array(vec![
            OwnedValue::Scalar(vela_common::ScalarValue::I64(3)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(4)),
        ]),
    );
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
fn linked_standard_value_method_refreshes_wrong_method_guard() {
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
                method_id: vela_stdlib::std_method_id("Map", "get_or")
                    .expect("Map::get_or method id"),
                standard_method: None,
            },
        },
    );

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::i64(4))
    );
    let entry = caches
        .entry(site)
        .expect("wrong method cache should refresh");
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard method cache should store refreshed value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::String);
    assert_eq!(standard_method.target, StandardMethodInlineCacheTarget::Len);
    assert_eq!(caches.set_count(), 2);
}

fn assert_iterator_adapter_cache(
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
    let entry = caches
        .entry(site)
        .expect("standard iterator cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard iterator cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::Iterator);
    assert_eq!(standard_method.target, target);
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        expected
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_refreshes_wrong_debug_name_guard() {
    let (program, site, dispatch, method_id) = linked_standard_len_cache_program();
    let caches = RecordingMethodCaches::new(1);
    let debug_name = program
        .method_dispatch(dispatch)
        .expect("dispatch should exist")
        .debug_name;
    let stale_debug_name = program
        .functions()
        .find(|(_, code)| program.debug_name(code.debug_name) == "main")
        .map(|(_, code)| code.debug_name)
        .expect("main function should exist");
    assert_ne!(stale_debug_name, debug_name);
    caches.prime(
        site,
        MethodInlineCacheEntry {
            dispatch,
            debug_name: stale_debug_name,
            target: MethodInlineCacheTarget::Value {
                method_id,
                standard_method: Some(StandardMethodInlineCacheEntry {
                    receiver: StandardMethodReceiver::String,
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
        .expect("wrong debug-name cache should refresh");
    assert_eq!(entry.debug_name, debug_name);
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_refreshes_wrong_standard_target_guard() {
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
                    receiver: StandardMethodReceiver::String,
                    target: StandardMethodInlineCacheTarget::Contains,
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
        .expect("wrong standard target cache should refresh");
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard method cache should store refreshed value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::String);
    assert_eq!(standard_method.target, StandardMethodInlineCacheTarget::Len);
    assert_eq!(caches.set_count(), 1);
}

#[test]
fn linked_standard_value_method_caches_collection_membership_targets() {
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
        Ok(RuntimeValue::U8(21))
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
        Ok(RuntimeValue::U8(21))
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
        Ok(RuntimeValue::U32(expected))
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
        Ok(RuntimeValue::U32(expected))
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
fn linked_standard_value_method_cached_bytes_slice_rejects_out_of_bounds() {
    let (program, site, _, _) = linked_bytes_slice_oob_cache_program();
    let caches = RecordingMethodCaches::new(1);

    let error = run_linked_method_cache_owned_program(&program, &caches)
        .expect_err("out-of-bounds bytes slice should fail");
    assert_eq!(
        error.kind(),
        VmErrorKind::IndexOutOfBounds { index: 5, len: 4 }
    );
    assert!(caches.entry(site).is_some());
    let populated_set_count = caches.set_count();

    let error = run_linked_method_cache_owned_program(&program, &caches)
        .expect_err("cached out-of-bounds bytes slice should fail");
    assert_eq!(
        error.kind(),
        VmErrorKind::IndexOutOfBounds { index: 5, len: 4 }
    );
    assert_eq!(caches.set_count(), populated_set_count);
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
    assert_unwrap_or_cache(
        linked_result_unwrap_or_cache_program(),
        StandardMethodReceiver::Result,
        RuntimeValue::i64(17),
    );
}

#[test]
fn linked_standard_value_method_caches_option_unwrap_or_target() {
    assert_unwrap_or_cache(
        linked_option_unwrap_or_cache_program(),
        StandardMethodReceiver::Option,
        RuntimeValue::i64(23),
    );
}

fn assert_unwrap_or_cache(
    fixture: (
        vela_bytecode::LinkedProgram,
        CacheSiteId,
        vela_bytecode::MethodDispatchHandle,
        MethodId,
    ),
    receiver: StandardMethodReceiver,
    expected: RuntimeValue,
) {
    let (program, site, dispatch, method_id) = fixture;
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(expected)
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
    assert_eq!(standard_method.receiver, receiver);
    assert_eq!(
        standard_method.target,
        StandardMethodInlineCacheTarget::UnwrapOr
    );
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(expected)
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
