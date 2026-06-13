use super::linked_standard_method_cache_fixtures::*;
use super::linked_standard_method_cache_support::*;
use super::*;

#[test]
fn linked_standard_value_method_refreshes_wrong_string_target_guard() {
    let (program, site, dispatch, method_id) = linked_standard_len_cache_program();
    let caches = RecordingMethodCaches::new(1);
    let debug_name = program
        .method_dispatch(dispatch)
        .expect("method dispatch should exist")
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
                    target: StandardMethodInlineCacheTarget::IsEmpty,
                }),
            },
        },
    );

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(Value::i64(4))
    );
    let entry = caches
        .entry(site)
        .expect("stale standard method target should refresh");
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("refreshed cache should store value target");
    };
    assert_eq!(entry.dispatch, dispatch);
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::String);
    assert_eq!(standard_method.target, StandardMethodInlineCacheTarget::Len);
    assert_eq!(caches.set_count_for(site), 1);
}

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

#[test]
fn linked_standard_value_method_caches_string_predicate_targets() {
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
        Ok(Value::Bool(true))
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
        Ok(Value::Bool(true))
    );
    assert_eq!(caches.set_count(), 2);
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
        "parse_i8",
        "42",
        StandardMethodInlineCacheTarget::ParseI8,
        OwnedValue::Scalar(vela_common::ScalarValue::I8(42)),
    );
    assert_string_no_arg_option_cache(
        "parse_i16",
        "42",
        StandardMethodInlineCacheTarget::ParseI16,
        OwnedValue::Scalar(vela_common::ScalarValue::I16(42)),
    );
    assert_string_no_arg_option_cache(
        "parse_i32",
        "42",
        StandardMethodInlineCacheTarget::ParseI32,
        OwnedValue::Scalar(vela_common::ScalarValue::I32(42)),
    );
    assert_string_no_arg_option_cache(
        "parse_i64",
        "42",
        StandardMethodInlineCacheTarget::ParseI64,
        OwnedValue::Scalar(vela_common::ScalarValue::I64(42)),
    );
    assert_string_no_arg_option_cache(
        "parse_u8",
        "42",
        StandardMethodInlineCacheTarget::ParseU8,
        OwnedValue::Scalar(vela_common::ScalarValue::U8(42)),
    );
    assert_string_no_arg_option_cache(
        "parse_u16",
        "42",
        StandardMethodInlineCacheTarget::ParseU16,
        OwnedValue::Scalar(vela_common::ScalarValue::U16(42)),
    );
    assert_string_no_arg_option_cache(
        "parse_u32",
        "42",
        StandardMethodInlineCacheTarget::ParseU32,
        OwnedValue::Scalar(vela_common::ScalarValue::U32(42)),
    );
    assert_string_no_arg_option_cache(
        "parse_u64",
        "42",
        StandardMethodInlineCacheTarget::ParseU64,
        OwnedValue::Scalar(vela_common::ScalarValue::U64(42)),
    );
    assert_string_no_arg_option_cache(
        "parse_f32",
        "1.5",
        StandardMethodInlineCacheTarget::ParseF32,
        OwnedValue::Scalar(vela_common::ScalarValue::F32(1.5)),
    );
    assert_string_no_arg_option_cache(
        "parse_f64",
        "1.5",
        StandardMethodInlineCacheTarget::ParseF64,
        OwnedValue::Scalar(vela_common::ScalarValue::F64(1.5)),
    );
    assert_string_no_arg_option_cache(
        "parse_bool",
        "true",
        StandardMethodInlineCacheTarget::ParseBool,
        OwnedValue::Bool(true),
    );
    assert_string_no_arg_option_cache(
        "parse_char",
        "奖",
        StandardMethodInlineCacheTarget::ParseChar,
        OwnedValue::Char('奖'),
    );
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
