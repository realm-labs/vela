use super::linked_standard_method_cache_support::{
    RecordingMethodCaches, run_linked_method_cache_program,
    run_linked_method_cache_program_with_standard_natives,
};
use super::standard_id_dispatch::std_method_id;
use super::*;
use std::cell::RefCell;
use vela_bytecode::{CacheSiteId, DebugNameId, Linker, MethodDispatchHandle};

#[test]
fn linked_callback_method_id_rejects_receiver_owner_mismatch() {
    let mut program = compile_standard_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let mapped = [1, 2, 3].map(|value| value + 1);
    return mapped[0];
}
"#,
    )
    .expect("standard callback method source should compile")
    .into_bytecode();
    replace_call_method_id(
        &mut program,
        std_method_id("Array", "map"),
        std_method_id("Set", "map"),
    );

    let mut budget = ExecutionBudget::unbounded();
    let error = run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget)
        .expect_err("linked callback dispatch must reject owner-mismatched method ids");

    assert_eq!(
        error.kind(),
        VmErrorKind::UnknownMethod {
            method: "map".to_owned()
        }
    );
}

#[test]
fn linked_callback_methods_forward_inline_caches_to_callback_body() {
    let program = compile_standard_program_source_with_native_functions(
        SourceId::new(1),
        "fn main() { return [1, 2, 3].map(|value| probe::add(value))[0]; }",
        &["probe::add"],
    )
    .expect("compile callback with native body");
    let mut vm = Vm::new();
    vm.register_native("probe::add", |args| {
        let OwnedValue::Scalar(vela_common::ScalarValue::I64(value)) = args[0] else {
            panic!("callback must pass its integer argument");
        };
        Ok(OwnedValue::i64(value + 40))
    });
    let mut linker = Linker::new();
    vm.native_implementation_ids()
        .for_each(|id| linker.add_native_implementation(id));
    let linked = linker
        .link_test_program(&program)
        .expect("link callback native");
    let caches = RecordingNativeCallCaches::new(max_cache_site_len(&linked));
    let mut host = HostExecution { state_values: None };
    let mut budget = ExecutionBudget::unbounded();
    assert_eq!(
        vm.run_linked_program_with_host_budget_and_caches(
            &linked,
            "main",
            &[],
            &mut host,
            &mut budget,
            Some(&caches),
        ),
        Ok(OwnedValue::i64(41))
    );
    assert!(
        caches.entry.borrow().is_some(),
        "callback body populates its native cache"
    );
}

#[test]
fn linked_callback_value_method_caches_array_any_target() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return [1, 2, 3].any(|value| value == 2);
}
"#,
    )
    .expect("standard callback method source should compile");
    let linked = link_test_program(&program);
    let site = linked_method_cache_site(&linked, "main", "any");
    let caches = RecordingMethodCaches::new(max_cache_site_len(&linked));

    assert_eq!(
        run_linked_method_cache_program(&linked, &caches),
        Ok(Value::Bool(true))
    );
    let entry = caches
        .entry(site)
        .expect("standard callback method cache should populate");
    let MethodInlineCacheTarget::CallbackValue {
        method_id,
        callback_method,
    } = entry.target
    else {
        panic!("standard callback method should store callback target");
    };
    assert_eq!(method_id, std_method_id("Array", "any"));
    assert_eq!(callback_method.receiver, StandardMethodReceiver::Array);
    assert_eq!(callback_method.target, CallbackMethodInlineCacheTarget::Any);
    assert_eq!(caches.set_count(), 1);

    assert_eq!(
        run_linked_method_cache_program(&linked, &caches),
        Ok(Value::Bool(true))
    );
    assert_eq!(caches.set_count(), 1);
}

#[test]
fn linked_callback_value_method_refreshes_wrong_method_guard() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return [1, 2, 3].any(|value| value == 2);
}
"#,
    )
    .expect("standard callback method source should compile");
    let linked = link_test_program(&program);
    let call = linked_method_cache_call(&linked, "main", "any");
    let caches = RecordingMethodCaches::new(max_cache_site_len(&linked));
    caches.prime(
        call.cache_site,
        MethodInlineCacheEntry {
            dispatch: call.dispatch,
            debug_name: call.debug_name,
            target: MethodInlineCacheTarget::CallbackValue {
                method_id: std_method_id("Set", "any"),
                callback_method: CallbackMethodInlineCacheEntry {
                    receiver: StandardMethodReceiver::Array,
                    target: CallbackMethodInlineCacheTarget::Any,
                },
            },
        },
    );

    assert_eq!(
        run_linked_method_cache_program(&linked, &caches),
        Ok(Value::Bool(true))
    );
    assert_callback_cache_entry(
        &caches,
        call.cache_site,
        std_method_id("Array", "any"),
        StandardMethodReceiver::Array,
        CallbackMethodInlineCacheTarget::Any,
    );
    assert_eq!(caches.set_count_for(call.cache_site), 1);

    assert_eq!(
        run_linked_method_cache_program(&linked, &caches),
        Ok(Value::Bool(true))
    );
    assert_eq!(caches.set_count_for(call.cache_site), 1);
}

#[test]
fn linked_callback_value_method_refreshes_wrong_debug_name_guard() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return [1, 2, 3].any(|value| value == 2);
}
"#,
    )
    .expect("standard callback method source should compile");
    let linked = link_test_program(&program);
    let call = linked_method_cache_call(&linked, "main", "any");
    let stale_debug_name = linked
        .functions()
        .find(|(_, code)| linked.debug_name(code.debug_name) == "main")
        .map(|(_, code)| code.debug_name)
        .expect("main function should exist");
    assert_ne!(stale_debug_name, call.debug_name);
    let caches = RecordingMethodCaches::new(max_cache_site_len(&linked));
    caches.prime(
        call.cache_site,
        MethodInlineCacheEntry {
            dispatch: call.dispatch,
            debug_name: stale_debug_name,
            target: MethodInlineCacheTarget::CallbackValue {
                method_id: std_method_id("Array", "any"),
                callback_method: CallbackMethodInlineCacheEntry {
                    receiver: StandardMethodReceiver::Array,
                    target: CallbackMethodInlineCacheTarget::Any,
                },
            },
        },
    );

    assert_eq!(
        run_linked_method_cache_program(&linked, &caches),
        Ok(Value::Bool(true))
    );
    let entry = caches
        .entry(call.cache_site)
        .expect("wrong debug-name callback cache should refresh");
    assert_eq!(entry.debug_name, call.debug_name);
    assert_callback_cache_entry(
        &caches,
        call.cache_site,
        std_method_id("Array", "any"),
        StandardMethodReceiver::Array,
        CallbackMethodInlineCacheTarget::Any,
    );
    assert_eq!(caches.set_count_for(call.cache_site), 1);

    assert_eq!(
        run_linked_method_cache_program(&linked, &caches),
        Ok(Value::Bool(true))
    );
    assert_eq!(caches.set_count_for(call.cache_site), 1);
}

#[test]
fn linked_callback_value_method_refreshes_wrong_receiver_guard() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return [1, 2, 3].any(|value| value == 2);
}
"#,
    )
    .expect("standard callback method source should compile");
    let linked = link_test_program(&program);
    let call = linked_method_cache_call(&linked, "main", "any");
    let caches = RecordingMethodCaches::new(max_cache_site_len(&linked));
    caches.prime(
        call.cache_site,
        MethodInlineCacheEntry {
            dispatch: call.dispatch,
            debug_name: call.debug_name,
            target: MethodInlineCacheTarget::CallbackValue {
                method_id: std_method_id("Array", "any"),
                callback_method: CallbackMethodInlineCacheEntry {
                    receiver: StandardMethodReceiver::Set,
                    target: CallbackMethodInlineCacheTarget::Any,
                },
            },
        },
    );

    assert_eq!(
        run_linked_method_cache_program(&linked, &caches),
        Ok(Value::Bool(true))
    );
    assert_callback_cache_entry(
        &caches,
        call.cache_site,
        std_method_id("Array", "any"),
        StandardMethodReceiver::Array,
        CallbackMethodInlineCacheTarget::Any,
    );
    assert_eq!(caches.set_count_for(call.cache_site), 1);

    assert_eq!(
        run_linked_method_cache_program(&linked, &caches),
        Ok(Value::Bool(true))
    );
    assert_eq!(caches.set_count_for(call.cache_site), 1);
}

#[test]
fn linked_callback_value_method_refreshes_wrong_callback_target_guard() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return [1, 2, 3].any(|value| value == 2);
}
"#,
    )
    .expect("standard callback method source should compile");
    let linked = link_test_program(&program);
    let call = linked_method_cache_call(&linked, "main", "any");
    let caches = RecordingMethodCaches::new(max_cache_site_len(&linked));
    caches.prime(
        call.cache_site,
        MethodInlineCacheEntry {
            dispatch: call.dispatch,
            debug_name: call.debug_name,
            target: MethodInlineCacheTarget::CallbackValue {
                method_id: std_method_id("Array", "any"),
                callback_method: CallbackMethodInlineCacheEntry {
                    receiver: StandardMethodReceiver::Array,
                    target: CallbackMethodInlineCacheTarget::Map,
                },
            },
        },
    );

    assert_eq!(
        run_linked_method_cache_program(&linked, &caches),
        Ok(Value::Bool(true))
    );
    assert_callback_cache_entry(
        &caches,
        call.cache_site,
        std_method_id("Array", "any"),
        StandardMethodReceiver::Array,
        CallbackMethodInlineCacheTarget::Any,
    );
    assert_eq!(caches.set_count_for(call.cache_site), 1);

    assert_eq!(
        run_linked_method_cache_program(&linked, &caches),
        Ok(Value::Bool(true))
    );
    assert_eq!(caches.set_count_for(call.cache_site), 1);
}

#[test]
fn linked_callback_value_method_caches_array_targets() {
    assert_callback_value_method_cache(
        r#"
fn main() {
    let mapped = [1, 2, 3].map(|value| value + 1);
    return mapped[1];
}
"#,
        "map",
        "Array",
        "map",
        StandardMethodReceiver::Array,
        CallbackMethodInlineCacheTarget::Map,
        Value::i64(3),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    let filtered = [1, 2, 3].filter(|value| value > 1);
    return filtered[0];
}
"#,
        "filter",
        "Array",
        "filter",
        StandardMethodReceiver::Array,
        CallbackMethodInlineCacheTarget::Filter,
        Value::i64(2),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    let values = [1, 2, 3, 4];
    values.retain(|value| value % 2 == 0);
    return values.sum();
}
"#,
        "retain",
        "Array",
        "retain",
        StandardMethodReceiver::Array,
        CallbackMethodInlineCacheTarget::Retain,
        Value::i64(6),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return option::unwrap_or([1, 2, 3].find(|value| value == 2), 0);
}
"#,
        "find",
        "Array",
        "find",
        StandardMethodReceiver::Array,
        CallbackMethodInlineCacheTarget::Find,
        Value::i64(2),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return [1, 2, 3].all(|value| value > 0);
}
"#,
        "all",
        "Array",
        "all",
        StandardMethodReceiver::Array,
        CallbackMethodInlineCacheTarget::All,
        Value::Bool(true),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return [1, 2, 3].count(|value| value > 1);
}
"#,
        "count",
        "Array",
        "count",
        StandardMethodReceiver::Array,
        CallbackMethodInlineCacheTarget::Count,
        Value::i64(2),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return [1, 2, 3].sum(|value| value + 1);
}
"#,
        "sum",
        "Array",
        "sum",
        StandardMethodReceiver::Array,
        CallbackMethodInlineCacheTarget::Sum,
        Value::i64(9),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    let groups = [1, 2, 3, 4].group_by(|value| if value % 2 == 0 { "even" } else { "odd" });
    return groups["even"][1];
}
"#,
        "group_by",
        "Array",
        "group_by",
        StandardMethodReceiver::Array,
        CallbackMethodInlineCacheTarget::GroupBy,
        Value::i64(4),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    let sorted = [21, 11, 10, 12].sort_by(|value| value % 10);
    return sorted[2];
}
"#,
        "sort_by",
        "Array",
        "sort_by",
        StandardMethodReceiver::Array,
        CallbackMethodInlineCacheTarget::SortBy,
        Value::i64(11),
    );
}

#[test]
fn linked_callback_value_method_caches_map_targets() {
    assert_callback_value_method_cache(
        r#"
fn main() {
    let mapped = {"gold": 4}.map_values(|value| value + 1);
    return mapped["gold"];
}
"#,
        "map_values",
        "Map",
        "map_values",
        StandardMethodReceiver::Map,
        CallbackMethodInlineCacheTarget::MapValues,
        Value::i64(5),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    let filtered = {"gold": 4, "xp": 6}.filter(|key, value| key == "xp" && value == 6);
    return filtered["xp"];
}
"#,
        "filter",
        "Map",
        "filter",
        StandardMethodReceiver::Map,
        CallbackMethodInlineCacheTarget::Filter,
        Value::i64(6),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    let grouped = {"gold": 4, "xp": 6}.group_by(|value| if value >= 6 { "high" } else { "low" });
    return grouped["high"]["xp"];
}
"#,
        "group_by",
        "Map",
        "group_by",
        StandardMethodReceiver::Map,
        CallbackMethodInlineCacheTarget::GroupBy,
        Value::i64(6),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    let rewards = {"gold": 4, "xp": 6, "quest": 8};
    rewards.retain(|key, value| key != "gold" && value >= 6);
    return rewards.values().collect_array().sum();
}
"#,
        "retain",
        "Map",
        "retain",
        StandardMethodReceiver::Map,
        CallbackMethodInlineCacheTarget::Retain,
        Value::i64(14),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    let found = {"gold": 4, "xp": 6}.find(|key, value| key == "xp" && value == 6);
    let entry = option::unwrap_or(found, MapEntry { key: "", value: 0 });
    return entry.value;
}
"#,
        "find",
        "Map",
        "find",
        StandardMethodReceiver::Map,
        CallbackMethodInlineCacheTarget::Find,
        Value::i64(6),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return {"gold": 4, "xp": 6}.any(|key, value| key == "gold" && value == 4);
}
"#,
        "any",
        "Map",
        "any",
        StandardMethodReceiver::Map,
        CallbackMethodInlineCacheTarget::Any,
        Value::Bool(true),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return {"gold": 4, "xp": 6}.all(|key, value| key != "" && value >= 4);
}
"#,
        "all",
        "Map",
        "all",
        StandardMethodReceiver::Map,
        CallbackMethodInlineCacheTarget::All,
        Value::Bool(true),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return {"gold": 4, "xp": 6, "quest": 8}.count(|key, value| key != "gold" && value > 4);
}
"#,
        "count",
        "Map",
        "count",
        StandardMethodReceiver::Map,
        CallbackMethodInlineCacheTarget::Count,
        Value::i64(2),
    );
}

#[test]
fn linked_callback_value_method_caches_iterator_fold_target() {
    assert_callback_value_method_cache(
        r#"
fn main() {
    return [2, 3, 5].iter().fold(10, |acc, value| acc + value);
}
"#,
        "fold",
        "Iterator",
        "fold",
        StandardMethodReceiver::Iterator,
        CallbackMethodInlineCacheTarget::Fold,
        Value::i64(20),
    );
}

#[test]
fn linked_callback_value_method_caches_set_targets() {
    assert_callback_value_method_cache(
        r#"
fn main() {
    return set::from_array([1, 2, 3]).map(|value| value + 1).values().collect_array().sum();
}
"#,
        "map",
        "Set",
        "map",
        StandardMethodReceiver::Set,
        CallbackMethodInlineCacheTarget::Map,
        Value::i64(9),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return set::from_array([1, 2, 3]).filter(|value| value > 1).values().collect_array().sum();
}
"#,
        "filter",
        "Set",
        "filter",
        StandardMethodReceiver::Set,
        CallbackMethodInlineCacheTarget::Filter,
        Value::i64(5),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    let values = set::from_array([1, 2, 3, 4]);
    values.retain(|value| value > 2);
    return values.values().collect_array().sum();
}
"#,
        "retain",
        "Set",
        "retain",
        StandardMethodReceiver::Set,
        CallbackMethodInlineCacheTarget::Retain,
        Value::i64(7),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return option::unwrap_or(set::from_array([1, 2, 3]).find(|value| value == 2), 0);
}
"#,
        "find",
        "Set",
        "find",
        StandardMethodReceiver::Set,
        CallbackMethodInlineCacheTarget::Find,
        Value::i64(2),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return set::from_array([1, 2, 3]).any(|value| value == 3);
}
"#,
        "any",
        "Set",
        "any",
        StandardMethodReceiver::Set,
        CallbackMethodInlineCacheTarget::Any,
        Value::Bool(true),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return set::from_array([1, 2, 3]).all(|value| value > 0);
}
"#,
        "all",
        "Set",
        "all",
        StandardMethodReceiver::Set,
        CallbackMethodInlineCacheTarget::All,
        Value::Bool(true),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return set::from_array([1, 2, 3]).count(|value| value > 1);
}
"#,
        "count",
        "Set",
        "count",
        StandardMethodReceiver::Set,
        CallbackMethodInlineCacheTarget::Count,
        Value::i64(2),
    );
}

#[test]
fn linked_callback_value_method_caches_option_targets() {
    assert_callback_value_method_cache(
        r#"
fn main() {
    return option::some(4).map(|value| value + 1).unwrap_or(0);
}
"#,
        "map",
        "Option",
        "map",
        StandardMethodReceiver::Option,
        CallbackMethodInlineCacheTarget::Map,
        Value::i64(5),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return option::none().map(|value| value + 1).unwrap_or(9);
}
"#,
        "map",
        "Option",
        "map",
        StandardMethodReceiver::Option,
        CallbackMethodInlineCacheTarget::Map,
        Value::i64(9),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return option::some(4).filter(|value| value > 3).unwrap_or(0);
}
"#,
        "filter",
        "Option",
        "filter",
        StandardMethodReceiver::Option,
        CallbackMethodInlineCacheTarget::Filter,
        Value::i64(4),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return option::none().filter(|value| value > 3).unwrap_or(8);
}
"#,
        "filter",
        "Option",
        "filter",
        StandardMethodReceiver::Option,
        CallbackMethodInlineCacheTarget::Filter,
        Value::i64(8),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return option::some(4).and_then(|value| option::some(value + 1)).unwrap_or(0);
}
"#,
        "and_then",
        "Option",
        "and_then",
        StandardMethodReceiver::Option,
        CallbackMethodInlineCacheTarget::AndThen,
        Value::i64(5),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return option::none().and_then(|value| option::some(value + 1)).unwrap_or(8);
}
"#,
        "and_then",
        "Option",
        "and_then",
        StandardMethodReceiver::Option,
        CallbackMethodInlineCacheTarget::AndThen,
        Value::i64(8),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return option::none().or_else(| | option::some(7)).unwrap_or(0);
}
"#,
        "or_else",
        "Option",
        "or_else",
        StandardMethodReceiver::Option,
        CallbackMethodInlineCacheTarget::OrElse,
        Value::i64(7),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return option::some(4).or_else(| | option::some(7)).unwrap_or(0);
}
"#,
        "or_else",
        "Option",
        "or_else",
        StandardMethodReceiver::Option,
        CallbackMethodInlineCacheTarget::OrElse,
        Value::i64(4),
    );
}

#[test]
fn linked_callback_value_method_caches_result_targets() {
    assert_callback_value_method_cache(
        r#"
fn main() {
    return result::unwrap_or(result::ok(4).map(|value| value + 1), 0);
}
"#,
        "map",
        "Result",
        "map",
        StandardMethodReceiver::Result,
        CallbackMethodInlineCacheTarget::Map,
        Value::i64(5),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return option::unwrap_or(result::err(8).map(|value| value + 1).to_error_option(), 0);
}
"#,
        "map",
        "Result",
        "map",
        StandardMethodReceiver::Result,
        CallbackMethodInlineCacheTarget::Map,
        Value::i64(8),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return option::unwrap_or(result::err(4).map_err(|error| error + 1).to_error_option(), 0);
}
"#,
        "map_err",
        "Result",
        "map_err",
        StandardMethodReceiver::Result,
        CallbackMethodInlineCacheTarget::MapErr,
        Value::i64(5),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return result::unwrap_or(result::ok(4).map_err(|error| error + 1), 0);
}
"#,
        "map_err",
        "Result",
        "map_err",
        StandardMethodReceiver::Result,
        CallbackMethodInlineCacheTarget::MapErr,
        Value::i64(4),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return result::unwrap_or(result::ok(4).and_then(|value| result::ok(value + 1)), 0);
}
"#,
        "and_then",
        "Result",
        "and_then",
        StandardMethodReceiver::Result,
        CallbackMethodInlineCacheTarget::AndThen,
        Value::i64(5),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return option::unwrap_or(result::err(8).and_then(|value| result::ok(value + 1)).to_error_option(), 0);
}
"#,
        "and_then",
        "Result",
        "and_then",
        StandardMethodReceiver::Result,
        CallbackMethodInlineCacheTarget::AndThen,
        Value::i64(8),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return result::unwrap_or(result::err(4).or_else(|error| result::ok(error + 1)), 0);
}
"#,
        "or_else",
        "Result",
        "or_else",
        StandardMethodReceiver::Result,
        CallbackMethodInlineCacheTarget::OrElse,
        Value::i64(5),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return result::unwrap_or(result::ok(4).or_else(|error| result::ok(error + 1)), 0);
}
"#,
        "or_else",
        "Result",
        "or_else",
        StandardMethodReceiver::Result,
        CallbackMethodInlineCacheTarget::OrElse,
        Value::i64(4),
    );
}

struct RecordingNativeCallCaches {
    len: usize,
    entry: RefCell<Option<NativeInlineCacheEntry>>,
}

impl RecordingNativeCallCaches {
    fn new(len: usize) -> Self {
        Self {
            len,
            entry: RefCell::new(None),
        }
    }
}

fn linked_method_cache_site(
    program: &vela_bytecode::LinkedProgram,
    function: &str,
    method: &str,
) -> CacheSiteId {
    linked_method_cache_call(program, function, method).cache_site
}

#[derive(Clone, Copy)]
struct LinkedMethodCacheCall {
    cache_site: CacheSiteId,
    dispatch: MethodDispatchHandle,
    debug_name: DebugNameId,
}

fn linked_method_cache_call(
    program: &vela_bytecode::LinkedProgram,
    function: &str,
    method: &str,
) -> LinkedMethodCacheCall {
    let (_, code) = program
        .functions()
        .find(|(_, code)| program.debug_name(code.debug_name) == function)
        .expect("linked function should exist");
    code.instructions
        .iter()
        .find_map(|instruction| {
            let vela_bytecode::linked::InstructionKind::CallMethod {
                debug_name,
                cache_site,
                dispatch,
                ..
            } = instruction.kind
            else {
                return None;
            };
            if program.debug_name(debug_name) == method {
                cache_site.map(|cache_site| LinkedMethodCacheCall {
                    cache_site,
                    dispatch,
                    debug_name,
                })
            } else {
                None
            }
        })
        .expect("linked method call should have cache site")
}

fn max_cache_site_len(program: &vela_bytecode::LinkedProgram) -> usize {
    program
        .functions()
        .map(|(_, code)| code.cache_sites.len())
        .max()
        .unwrap_or(0)
}

fn assert_callback_value_method_cache(
    source: &str,
    site_method: &str,
    method_owner: &str,
    method_name: &str,
    receiver: StandardMethodReceiver,
    target: CallbackMethodInlineCacheTarget,
    expected: Value,
) {
    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("standard callback method source should compile");
    let linked = link_standard_native_test_program(&program);
    let site = linked_method_cache_site(&linked, "main", site_method);
    let caches = RecordingMethodCaches::new(max_cache_site_len(&linked));

    assert_eq!(
        run_linked_method_cache_program_with_standard_natives(&linked, &caches),
        Ok(expected)
    );
    assert_callback_cache_entry(
        &caches,
        site,
        std_method_id(method_owner, method_name),
        receiver,
        target,
    );
    let warmed_site_set_count = caches.set_count_for(site);

    assert_eq!(
        run_linked_method_cache_program_with_standard_natives(&linked, &caches),
        Ok(expected)
    );
    assert_callback_cache_entry(
        &caches,
        site,
        std_method_id(method_owner, method_name),
        receiver,
        target,
    );
    assert_eq!(caches.set_count_for(site), warmed_site_set_count);
}

fn assert_callback_cache_entry(
    caches: &RecordingMethodCaches,
    site: CacheSiteId,
    expected_method_id: MethodId,
    expected_receiver: StandardMethodReceiver,
    expected_target: CallbackMethodInlineCacheTarget,
) {
    let entry = caches
        .entry(site)
        .expect("standard callback method cache should populate");
    let MethodInlineCacheTarget::CallbackValue {
        method_id,
        callback_method,
    } = entry.target
    else {
        panic!("standard callback method should store callback target");
    };
    assert_eq!(method_id, expected_method_id);
    assert_eq!(callback_method.receiver, expected_receiver);
    assert_eq!(callback_method.target, expected_target);
}

fn link_standard_native_test_program(program: &UnlinkedProgram) -> Arc<LinkedArtifact> {
    let vm = Vm::new().with_standard_natives();
    let mut linker = Linker::new();
    for id in vm.native_implementation_ids() {
        linker.add_native_implementation(id);
    }
    linker
        .link_test_program(program)
        .expect("standard native test program should link")
}

impl VmInlineCaches for RecordingNativeCallCaches {
    fn len(&self) -> usize {
        self.len
    }

    fn native_call(&self, _site: CacheSiteId) -> Option<NativeInlineCacheEntry> {
        self.entry.borrow().clone()
    }

    fn set_native_call(&self, _site: CacheSiteId, entry: NativeInlineCacheEntry) {
        *self.entry.borrow_mut() = Some(entry);
    }
}

fn replace_call_method_id(
    program: &mut UnlinkedProgram,
    expected_method: MethodId,
    replacement_method: MethodId,
) {
    let code = program
        .function_mut("main")
        .expect("test function should exist");
    for instruction in &mut code.instructions {
        if let UnlinkedInstructionKind::CallMethodId { method_id, .. } = &mut instruction.kind
            && *method_id == expected_method
        {
            *method_id = replacement_method;
            return;
        }
    }
    panic!("test method call should exist");
}
