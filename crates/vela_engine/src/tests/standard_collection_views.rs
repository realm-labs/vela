use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::permission::Capability;
use crate::runtime::{CallArgs, CallOptions, Runtime};

const MATRIX_SOURCE: &str = r#"
fn own_array(values) {
    values.push(9);
    return values;
}

fn own_fixed(values) {
    values[1] += 10;
    return values;
}

fn echo(values) {
    return values;
}

fn own_map(values) {
    values.set("z", 9);
    return values;
}

fn own_set(values) {
    values.add(9);
    return values;
}

fn read_array(values) {
    return values.iter().fold(0, |total, value| total + value);
}

fn read_second(values) {
    return values[1];
}

fn read_map(values) {
    return values.values().fold(0, |total, value| total + value);
}

fn read_set(values) {
    return values.values().fold(0, |total, value| total + value);
}

fn replace_array(values) {
    values[1] += 10;
    return values[1];
}

fn grow_array(values) {
    values.push(9);
    return values.len();
}

fn mutate_bytes(values) {
    values[1] = 9u8;
    values.push(7u8);
    return values.len();
}

fn mutate_map(values) {
    values.set("z", 9);
    values.remove("a");
    return values.len();
}

fn mutate_set(values) {
    values.add(9);
    values.remove(1);
    return values.len();
}

fn push_then_fail(values) {
    values.push(7);
    return values.missing_after_write();
}
"#;

fn matrix_engine() -> Engine {
    Engine::builder()
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .register_rust_value_closure::<Vec<i64>>()
        .register_rust_value_closure::<Vec<u8>>()
        .register_rust_value_closure::<[i64; 3]>()
        .register_rust_slice::<i64>()
        .register_rust_value_closure::<BTreeMap<String, i64>>()
        .register_rust_value_closure::<HashMap<String, i64>>()
        .register_rust_value_closure::<BTreeSet<i64>>()
        .register_rust_value_closure::<HashSet<i64>>()
        .build()
        .expect("standard collection matrix bindings should seal")
}

fn matrix_runtime(engine: Engine) -> Runtime {
    let program = engine
        .compile_source(MATRIX_SOURCE)
        .expect("standard collection matrix fixture should compile");
    Runtime::new(engine, program).expect("standard collection matrix runtime should initialize")
}

#[test]
fn owned_standard_collection_matrix_round_trips_both_directions() {
    let engine = matrix_engine();
    let bindings = engine.type_bindings();
    let vec_codec = bindings.value_codec::<Vec<i64>>().expect("Vec codec");
    let bytes_codec = bindings.value_codec::<Vec<u8>>().expect("bytes codec");
    let fixed_codec = bindings
        .value_codec::<[i64; 3]>()
        .expect("fixed array codec");
    let ordered_map_codec = bindings
        .value_codec::<BTreeMap<String, i64>>()
        .expect("BTreeMap codec");
    let hashed_map_codec = bindings
        .value_codec::<HashMap<String, i64>>()
        .expect("HashMap codec");
    let ordered_set_codec = bindings
        .value_codec::<BTreeSet<i64>>()
        .expect("BTreeSet codec");
    let hashed_set_codec = bindings
        .value_codec::<HashSet<i64>>()
        .expect("HashSet codec");
    let ordered_map_binding = bindings
        .get_for::<BTreeMap<String, i64>>()
        .expect("BTreeMap binding");
    let hashed_map_binding = bindings
        .get_for::<HashMap<String, i64>>()
        .expect("HashMap binding");
    assert_ne!(ordered_map_binding.id, hashed_map_binding.id);
    assert_ne!(
        ordered_map_binding.abi_fingerprint,
        hashed_map_binding.abi_fingerprint
    );
    let ordered_set_binding = bindings
        .get_for::<BTreeSet<i64>>()
        .expect("BTreeSet binding");
    let hashed_set_binding = bindings.get_for::<HashSet<i64>>().expect("HashSet binding");
    assert_ne!(ordered_set_binding.id, hashed_set_binding.id);
    assert_ne!(
        ordered_set_binding.abi_fingerprint,
        hashed_set_binding.abi_fingerprint
    );
    drop(bindings);
    let mut runtime = matrix_runtime(engine);

    let array = runtime
        .call(
            "own_array",
            CallArgs::from_positional([vec_codec.encode(vec![1, 2])]),
            CallOptions::unbounded(),
        )
        .expect("owned Vec should use growable Array behavior");
    let array = runtime
        .value_to_owned(&array)
        .expect("owned Vec result should materialize");
    assert_eq!(vec_codec.decode(&array), Ok(vec![1, 2, 9]));

    let fixed = runtime
        .call(
            "own_fixed",
            CallArgs::from_positional([fixed_codec.encode([1, 2, 3])]),
            CallOptions::unbounded(),
        )
        .expect("owned fixed array should use Array value behavior");
    let fixed = runtime
        .value_to_owned(&fixed)
        .expect("owned fixed array result should materialize");
    assert_eq!(fixed_codec.decode(&fixed), Ok([1, 12, 3]));

    let bytes = runtime
        .call(
            "echo",
            CallArgs::from_positional([bytes_codec.encode(vec![1, 2, 3])]),
            CallOptions::unbounded(),
        )
        .expect("owned bytes should cross through the registered value codec");
    let bytes = runtime
        .value_to_owned(&bytes)
        .expect("owned bytes result should materialize");
    assert_eq!(bytes_codec.decode(&bytes), Ok(vec![1, 2, 3]));

    let ordered = runtime
        .call(
            "own_map",
            CallArgs::from_positional([
                ordered_map_codec.encode(BTreeMap::from([("a".to_owned(), 1_i64)]))
            ]),
            CallOptions::unbounded(),
        )
        .expect("owned BTreeMap should use MapLike behavior");
    let ordered = runtime
        .value_to_owned(&ordered)
        .expect("owned BTreeMap result should materialize");
    assert_eq!(
        ordered_map_codec.decode(&ordered),
        Ok(BTreeMap::from([("a".to_owned(), 1), ("z".to_owned(), 9)]))
    );

    let hashed = runtime
        .call(
            "own_map",
            CallArgs::from_positional([
                hashed_map_codec.encode(HashMap::from([("a".to_owned(), 3_i64)]))
            ]),
            CallOptions::unbounded(),
        )
        .expect("owned HashMap should share MapLike behavior");
    let hashed = runtime
        .value_to_owned(&hashed)
        .expect("owned HashMap result should materialize");
    let hashed = hashed_map_codec
        .decode(&hashed)
        .expect("HashMap result should decode");
    assert_eq!(hashed.get("a"), Some(&3));
    assert_eq!(hashed.get("z"), Some(&9));

    let ordered = runtime
        .call(
            "own_set",
            CallArgs::from_positional([ordered_set_codec.encode(BTreeSet::from([1, 2]))]),
            CallOptions::unbounded(),
        )
        .expect("owned BTreeSet should use SetLike behavior");
    let ordered = runtime
        .value_to_owned(&ordered)
        .expect("owned BTreeSet result should materialize");
    assert_eq!(
        ordered_set_codec.decode(&ordered),
        Ok(BTreeSet::from([1, 2, 9]))
    );

    let hashed = runtime
        .call(
            "own_set",
            CallArgs::from_positional([hashed_set_codec.encode(HashSet::from([1, 3]))]),
            CallOptions::unbounded(),
        )
        .expect("owned HashSet should share SetLike behavior");
    let hashed = runtime
        .value_to_owned(&hashed)
        .expect("owned HashSet result should materialize");
    assert_eq!(
        hashed_set_codec.decode(&hashed),
        Ok(HashSet::from([1, 3, 9]))
    );
}

#[test]
fn shared_standard_collection_matrix_reads_and_rejects_mutation() {
    let mut runtime = matrix_runtime(matrix_engine());
    let array = vec![2_i64, 3];
    let bytes = vec![1_u8, 2, 3];
    let fixed = [4_i64, 5, 6];
    let slice = [7_i64, 8];
    let ordered_map = BTreeMap::from([("a".to_owned(), 11_i64), ("b".to_owned(), 13)]);
    let hashed_map = HashMap::from([("a".to_owned(), 17_i64), ("b".to_owned(), 19)]);
    let ordered_set = BTreeSet::from([2_i64, 5]);
    let hashed_set = HashSet::from([3_i64, 7]);

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &array);
    let result = runtime
        .call("read_array", args, CallOptions::unbounded())
        .expect("shared Vec should support live reads");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(5)));

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &bytes);
    let result = runtime
        .call("read_second", args, CallOptions::unbounded())
        .expect("shared Vec<u8> should retain Bytes identity and Array view access");
    assert_eq!(
        runtime.value_to_owned(&result),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::U8(2)))
    );

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &fixed);
    let result = runtime
        .call("read_array", args, CallOptions::unbounded())
        .expect("shared fixed array should support live reads");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(15)));

    let mut args = CallArgs::new();
    args.push_slice_ref("values", &slice);
    let result = runtime
        .call("read_array", args, CallOptions::unbounded())
        .expect("shared slice should support live reads without a Vec copy");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(15)));

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &ordered_map);
    let result = runtime
        .call("read_map", args, CallOptions::unbounded())
        .expect("shared BTreeMap should support live reads");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(24)));

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &hashed_map);
    let result = runtime
        .call("read_map", args, CallOptions::unbounded())
        .expect("shared HashMap should support the same live reads");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(36)));

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &ordered_set);
    let result = runtime
        .call("read_set", args, CallOptions::unbounded())
        .expect("shared BTreeSet should support live reads");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(7)));

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &hashed_set);
    let result = runtime
        .call("read_set", args, CallOptions::unbounded())
        .expect("shared HashSet should support the same live reads");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(10)));

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &array);
    runtime
        .call("grow_array", args, CallOptions::unbounded())
        .expect_err("shared Vec must not expose structural mutation");
    assert_eq!(array, vec![2, 3]);
    assert_eq!(bytes, vec![1, 2, 3]);

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &ordered_map);
    runtime
        .call("mutate_map", args, CallOptions::unbounded())
        .expect_err("shared Map must not expose keyed mutation");
    assert_eq!(
        ordered_map,
        BTreeMap::from([("a".to_owned(), 11), ("b".to_owned(), 13)])
    );

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &ordered_set);
    runtime
        .call("mutate_set", args, CallOptions::unbounded())
        .expect_err("shared Set must not expose membership mutation");
    assert_eq!(ordered_set, BTreeSet::from([2, 5]));
}

#[test]
fn exclusive_standard_collection_matrix_preserves_fixed_and_growable_writes() {
    let mut runtime = matrix_runtime(matrix_engine());

    let mut array = vec![1_i64, 2];
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut array);
    let result = runtime
        .call("grow_array", args, CallOptions::unbounded())
        .expect("exclusive Vec should expose growable Array mutation");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(3)));
    drop(result);
    assert_eq!(array, vec![1, 2, 9]);

    let mut bytes = vec![1_u8, 2, 3];
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut bytes);
    let result = runtime
        .call("mutate_bytes", args, CallOptions::unbounded())
        .expect("exclusive Vec<u8> should use a growable Array MutView");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(4)));
    drop(result);
    assert_eq!(bytes, vec![1, 9, 3, 7]);

    let mut fixed = [1_i64, 2, 3];
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut fixed);
    let result = runtime
        .call("replace_array", args, CallOptions::unbounded())
        .expect("exclusive fixed array should expose indexed replacement");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(12)));
    drop(result);
    assert_eq!(fixed, [1, 12, 3]);
    let before = fixed;
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut fixed);
    runtime
        .call("grow_array", args, CallOptions::unbounded())
        .expect_err("fixed array MutView must reject structural growth");
    assert_eq!(fixed, before);

    let mut slice_storage = [4_i64, 5, 6];
    let mut args = CallArgs::new();
    args.push_slice_mut("values", &mut slice_storage);
    let result = runtime
        .call("replace_array", args, CallOptions::unbounded())
        .expect("exclusive slice should write through indexed replacement");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(15)));
    drop(result);
    assert_eq!(slice_storage, [4, 15, 6]);
    let before = slice_storage;
    let mut args = CallArgs::new();
    args.push_slice_mut("values", &mut slice_storage);
    runtime
        .call("grow_array", args, CallOptions::unbounded())
        .expect_err("slice MutView must reject structural growth");
    assert_eq!(slice_storage, before);

    let mut ordered_map = BTreeMap::from([("a".to_owned(), 1_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut ordered_map);
    let result = runtime
        .call("mutate_map", args, CallOptions::unbounded())
        .expect("exclusive BTreeMap should expose growable Map mutation");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(1)));
    drop(result);
    assert_eq!(ordered_map, BTreeMap::from([("z".to_owned(), 9)]));

    let mut hashed_map = HashMap::from([("a".to_owned(), 2_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut hashed_map);
    runtime
        .call("mutate_map", args, CallOptions::unbounded())
        .expect("exclusive HashMap should share growable Map mutation");
    assert_eq!(hashed_map, HashMap::from([("z".to_owned(), 9)]));

    let mut ordered_set = BTreeSet::from([1_i64, 2]);
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut ordered_set);
    runtime
        .call("mutate_set", args, CallOptions::unbounded())
        .expect("exclusive BTreeSet should expose growable Set mutation");
    assert_eq!(ordered_set, BTreeSet::from([2, 9]));

    let mut hashed_set = HashSet::from([1_i64, 3]);
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut hashed_set);
    runtime
        .call("mutate_set", args, CallOptions::unbounded())
        .expect("exclusive HashSet should share growable Set mutation");
    assert_eq!(hashed_set, HashSet::from([3, 9]));

    let mut write_through = vec![1_i64];
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut write_through);
    runtime
        .call("push_then_fail", args, CallOptions::unbounded())
        .expect_err("a later trap must not imply mutable copy-out rollback");
    assert_eq!(
        write_through,
        vec![1, 7],
        "host mutation must be immediate rather than deferred copy-out"
    );
}

#[test]
fn complex_map_group_by_keeps_live_child_host_refs() {
    let engine = Engine::builder()
        .capability(Capability::HostRead)
        .register_rust_value_closure::<BTreeMap<String, Vec<i64>>>()
        .build()
        .expect("nested standard Map binding should seal");
    let program = engine
        .compile_source(
            "fn group(values) { \
                 let grouped = values.group_by(|key, value| \
                     if value.len() >= 2 { \"many\" } else { key }); \
                 return grouped[\"many\"][\"alpha\"][1] + grouped[\"beta\"][\"beta\"][0]; \
             }",
        )
        .expect("complex Map group_by fixture should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let values = BTreeMap::from([
        ("alpha".to_owned(), vec![1_i64, 2]),
        ("beta".to_owned(), vec![3]),
    ]);
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);

    let result = runtime
        .call("group", args, CallOptions::unbounded())
        .expect("grouping should preserve complex child HostRefs");

    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(5)));
    assert_eq!(values["alpha"], vec![1, 2]);
    assert_eq!(values["beta"], vec![3]);
}

#[test]
fn borrowed_map_iterator_fold_uses_live_complex_child_views() {
    let engine = Engine::builder()
        .capability(Capability::HostRead)
        .register_rust_value_closure::<BTreeMap<String, Vec<i64>>>()
        .build()
        .expect("nested standard Map binding should seal");
    let program = engine
        .compile_source(
            "fn fold(values) { \
                 return values.values().fold(10, |total, value| total + value.len()); \
             }",
        )
        .expect("borrowed Map iterator fold fixture should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let values = BTreeMap::from([
        ("alpha".to_owned(), vec![1_i64, 2]),
        ("beta".to_owned(), vec![3]),
    ]);
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);

    let result = runtime
        .call("fold", args, CallOptions::unbounded())
        .expect("fold should consume live complex child HostRefs");

    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(13)));
    assert_eq!(values["alpha"], vec![1, 2]);
    assert_eq!(values["beta"], vec![3]);

    let baseline = (0..160)
        .find(|limit| {
            let mut args = CallArgs::new();
            args.push_collection_ref("values", &values);
            runtime
                .call(
                    "fold",
                    args,
                    CallOptions::new(*limit, usize::MAX, usize::MAX),
                )
                .is_ok()
        })
        .expect("live borrowed fold should fit a bounded call");
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let error = runtime
        .call(
            "fold",
            args,
            CallOptions::new(baseline - 1, usize::MAX, usize::MAX),
        )
        .expect_err("one unit below the complete fold budget must reject the call");
    assert!(matches!(
        error.kind(),
        vela_vm::error::VmErrorKind::BudgetExceeded { .. }
    ));
    assert_eq!(values["alpha"], vec![1, 2]);
    assert_eq!(values["beta"], vec![3]);
}
