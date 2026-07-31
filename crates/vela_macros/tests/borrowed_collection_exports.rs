#![allow(clippy::ptr_arg)] // The boundary contract intentionally distinguishes &Vec from slices.

use std::collections::{BTreeMap, BTreeSet};
use std::future::poll_fn;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::Poll;

use vela_common::{CollectionViewKind, CollectionViewMutation, InteropRepresentation};
use vela_engine::context::NativeCallContext;
use vela_engine::engine::Engine;
use vela_engine::interop::BoundaryMode;
use vela_engine::native::TypeHint;
use vela_engine::permission::Capability;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_macros::{ScriptHost, export, methods};
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

static SLICE_GATE_READY: AtomicBool = AtomicBool::new(false);

#[path = "borrowed_collection_exports/slice_erasure_acceptance.rs"]
mod slice_erasure_acceptance;

#[export(path = "collections::merge")]
pub fn merge(values: &Vec<i64>, totals: &mut BTreeMap<String, i64>) -> i64 {
    let sum = values.iter().sum::<i64>();
    *totals.entry("sum".to_owned()).or_default() += sum;
    totals["sum"]
}

#[export(path = "collections::add")]
pub fn add(totals: &mut BTreeMap<String, i64>, amount: i64) -> i64 {
    *totals.entry("sum".to_owned()).or_default() += amount;
    totals["sum"]
}

#[export(path = "collections::lookup_i32")]
pub fn lookup_i32(scores: &BTreeMap<i32, i64>, key: i32) -> i64 {
    scores[&key]
}

#[export(path = "collections::contains_i32")]
pub fn contains_i32(values: &BTreeSet<i32>, value: i32) -> bool {
    values.contains(&value)
}

#[export(path = "collections::retain_non_two")]
pub fn retain_non_two(value: i64) -> VmResult<bool> {
    if value == 2 {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "retain callback failure fixture",
        }));
    }
    Ok(value > 0)
}

#[export(path = "collections::merge_async")]
pub async fn merge_async(values: &Vec<i64>, totals: &mut BTreeMap<String, i64>) -> i64 {
    merge(values, totals)
}

#[export(path = "collections::fixed_sum")]
pub fn fixed_sum(values: &[i64; 3]) -> i64 {
    values.iter().sum()
}

#[export(path = "collections::fixed_bump")]
pub fn fixed_bump(values: &mut [i64; 3]) -> i64 {
    values[1] += 4;
    values[1]
}

#[export(path = "collections::slice_sum")]
pub fn slice_sum(values: &[i64]) -> i64 {
    values.iter().sum()
}

#[export(path = "collections::slice_bump")]
pub fn slice_bump(values: &mut [i64]) -> i64 {
    values[1] += 4;
    values[1]
}

#[export(path = "collections::byte_slice_sum")]
pub fn byte_slice_sum(values: &[u8]) -> i64 {
    values.iter().map(|value| i64::from(*value)).sum()
}

#[export(path = "collections::byte_slice_bump")]
pub fn byte_slice_bump(values: &mut [u8]) -> i64 {
    values[1] += 4;
    i64::from(values[1])
}

#[export(path = "collections::byte_vec_push")]
pub fn byte_vec_push(values: &mut Vec<u8>, value: u8) -> i64 {
    values.push(value);
    i64::try_from(values.len()).expect("test byte vector length must fit i64")
}

#[export(path = "collections::slice_sum_async")]
pub async fn slice_sum_async(values: &[i64]) -> i64 {
    slice_sum(values)
}

#[export(path = "collections::slice_pair_sum")]
pub fn slice_pair_sum(left: &[i64], right: &[i64]) -> i64 {
    left.iter().sum::<i64>() + right.iter().sum::<i64>()
}

#[export(path = "collections::slice_pair_mut")]
pub fn slice_pair_mut(left: &mut [i64], right: &mut [i64]) -> i64 {
    left[0] += right[0];
    left[0]
}

#[export(path = "collections::slice_len_mut")]
pub fn slice_len_mut(values: &mut [i64]) -> i64 {
    i64::try_from(values.len()).expect("test slice length must fit i64")
}

#[export(path = "collections::slice_fail")]
pub fn slice_fail(values: &mut [i64]) -> VmResult<i64> {
    values[0] += 1;
    Err(VmError::new(VmErrorKind::TypeMismatch {
        operation: "borrowed slice failure fixture",
    }))
}

#[export(path = "collections::slice_yield_once")]
pub async fn slice_yield_once(values: &[i64]) -> i64 {
    let mut first_poll = true;
    poll_fn(|context| {
        if std::mem::take(&mut first_poll) {
            context.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    })
    .await;
    slice_sum(values)
}

#[export(path = "collections::slice_wait")]
pub async fn slice_wait(values: &mut [i64]) -> i64 {
    values[0] += 1;
    std::future::pending().await
}

#[export(path = "collections::slice_panic_async")]
pub async fn slice_panic_async(values: &mut [i64]) -> i64 {
    values[0] += 1;
    std::future::ready(()).await;
    panic!("borrowed slice panic fixture")
}

#[export(path = "collections::slice_gate")]
pub async fn slice_gate(values: &[i64]) -> i64 {
    poll_fn(|_| {
        if SLICE_GATE_READY.load(Ordering::SeqCst) {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    })
    .await;
    slice_sum(values)
}

#[export(path = "collections::slice_reenter")]
pub fn slice_reenter(context: &mut NativeCallContext<'_, '_>, values: &[i64]) -> VmResult<i64> {
    let mut args = CallArgs::new();
    args.push_slice_ref("values", values);
    let _ = context.call("slice_nested", args)?;
    Ok(slice_sum(values))
}

#[derive(ScriptHost)]
#[vela(path = "host::CollectionService")]
struct CollectionService {
    offset: i64,
}

#[methods(path = "host::CollectionService")]
impl CollectionService {
    pub fn merge(&self, values: &Vec<i64>, totals: &mut BTreeMap<String, i64>) -> i64 {
        let sum = values.iter().sum::<i64>() + self.offset;
        *totals.entry("sum".to_owned()).or_default() += sum;
        totals["sum"]
    }

    pub async fn add_async(&self, totals: &mut BTreeMap<String, i64>, amount: i64) -> i64 {
        add(totals, amount + self.offset)
    }

    pub fn slice_sum(&self, values: &[i64]) -> i64 {
        values.iter().sum::<i64>() + self.offset
    }

    pub async fn slice_sum_async(&self, values: &[i64]) -> i64 {
        self.slice_sum(values)
    }
}

#[derive(ScriptHost)]
#[vela(path = "host::CollectionOwner")]
struct CollectionOwner {
    values: Vec<i64>,
    totals: BTreeMap<String, i64>,
}

#[methods(path = "host::CollectionOwner")]
impl CollectionOwner {
    pub fn values(&self) -> &Vec<i64> {
        &self.values
    }

    pub fn values_mut(&mut self) -> &mut Vec<i64> {
        &mut self.values
    }

    pub fn totals_mut(&mut self) -> &mut BTreeMap<String, i64> {
        &mut self.totals
    }

    pub fn slice(&self) -> &[i64] {
        self.values.as_slice()
    }

    pub fn slice_mut(&mut self) -> &mut [i64] {
        self.values.as_mut_slice()
    }
}

#[derive(ScriptHost)]
#[vela(path = "host::ByteOwner")]
struct ByteOwner {
    bytes: Vec<u8>,
}

#[methods(path = "host::ByteOwner")]
impl ByteOwner {
    pub fn bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }

    pub fn bytes_mut(&mut self) -> &mut [u8] {
        self.bytes.as_mut_slice()
    }
}

#[test]
fn export_macro_describes_exact_borrowed_collection_representations() {
    let contract = vela_callable_contract_merge();

    assert_eq!(
        contract.parameters[0].ty,
        TypeHint::array_view_of(TypeHint::i64())
    );
    assert_eq!(contract.parameters[0].mode, BoundaryMode::SharedHost);
    assert_eq!(
        contract.parameters[0]
            .binding
            .expect("shared Vec binding contract")
            .representation,
        InteropRepresentation::CollectionView(CollectionViewKind::Array)
    );
    assert_eq!(
        contract.parameters[1].ty,
        TypeHint::map_mut_of(
            TypeHint::string(),
            TypeHint::i64(),
            CollectionViewMutation::Growable,
        )
    );
    assert_eq!(contract.parameters[1].mode, BoundaryMode::ExclusiveHost);
    assert_eq!(
        contract.parameters[1]
            .binding
            .expect("exclusive BTreeMap binding contract")
            .representation,
        InteropRepresentation::CollectionMut {
            kind: CollectionViewKind::Map,
            mutation: CollectionViewMutation::Growable,
        }
    );
}

#[test]
fn generated_free_adapter_reborrows_collections_and_writes_through() {
    let mut runtime = runtime(
        "fn main(values: ArrayView<i64>, totals: MapMut<String, i64>) { return collections::merge(values, totals); }",
    );
    let values = vec![2_i64, 3, 5];
    let mut totals = BTreeMap::from([("sum".to_owned(), 1_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values)
        .push_collection_mut("totals", &mut totals);

    let result = runtime
        .call("main", args, CallOptions::unbounded())
        .expect("generated collection adapter should execute");

    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(11)));
    drop(result);
    assert_eq!(totals["sum"], 11);
}

#[test]
fn fixed_array_views_reborrow_and_preserve_non_growable_mutation() {
    let shared = vela_callable_contract_fixed_sum();
    assert_eq!(
        shared.parameters[0]
            .binding
            .expect("shared fixed-array binding")
            .representation,
        InteropRepresentation::CollectionView(CollectionViewKind::Array)
    );
    let mutable = vela_callable_contract_fixed_bump();
    assert_eq!(
        mutable.parameters[0]
            .binding
            .expect("mutable fixed-array binding")
            .representation,
        InteropRepresentation::CollectionMut {
            kind: CollectionViewKind::Array,
            mutation: CollectionViewMutation::Fixed,
        }
    );

    let mut runtime = runtime(
        "fn read(values: ArrayView<i64>) { return collections::fixed_sum(values) + values[1]; } fn write(values: ArrayMut<i64>) { values[0] = values[0] + 1; return collections::fixed_bump(values) + values[0]; }",
    );
    let values = [2_i64, 3, 5];
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call("read", args, CallOptions::unbounded())
        .expect("shared fixed array should reborrow without materialization");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(13)));
    drop(result);

    let mut values = [2_i64, 3, 5];
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut values);
    let result = runtime
        .call("write", args, CallOptions::unbounded())
        .expect("mutable fixed array should write elements through HostAccess");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(10)));
    drop(result);
    assert_eq!(values, [3, 7, 5]);
}

#[test]
fn slice_views_preserve_reference_semantics_across_vela_and_rust() {
    let shared = vela_callable_contract_slice_sum();
    assert_eq!(
        shared.parameters[0]
            .binding
            .expect("shared slice binding")
            .representation,
        InteropRepresentation::CollectionView(CollectionViewKind::Array)
    );
    let mutable = vela_callable_contract_slice_bump();
    assert_eq!(
        mutable.parameters[0]
            .binding
            .expect("mutable slice binding")
            .representation,
        InteropRepresentation::CollectionMut {
            kind: CollectionViewKind::Array,
            mutation: CollectionViewMutation::Fixed,
        }
    );

    let mut runtime = runtime(
        "fn read(values: ArrayView<i64>) { let selected = values.filter(|value| value > 2); return collections::slice_sum(values) + selected.len(); } fn write(values: ArrayMut<i64>) { values[0] = values[0] + 1; return collections::slice_bump(values) + values[0]; }",
    );
    let values = [2_i64, 3, 5, 7];
    let mut args = CallArgs::new();
    args.push_slice_ref("values", &values[1..]);
    let result = runtime
        .call("read", args, CallOptions::unbounded())
        .expect("shared slice should be readable and reborrow into Rust");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(18)));
    drop(result);

    let mut values = [2_i64, 3, 5, 7];
    let mut args = CallArgs::new();
    args.push_slice_mut("values", &mut values[1..]);
    let result = runtime
        .call("write", args, CallOptions::unbounded())
        .expect("mutable slice should write through before the Rust reborrow");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(13)));
    drop(result);
    assert_eq!(values, [2, 4, 9, 7]);
}

#[test]
fn byte_views_preserve_bytes_identity_and_host_backed_mutation() {
    let shared = vela_callable_contract_byte_slice_sum();
    assert_eq!(
        shared.parameters[0].ty,
        TypeHint::array_view_of(TypeHint::u8())
    );
    assert_eq!(
        shared.parameters[0]
            .binding
            .expect("shared byte-slice binding")
            .representation,
        InteropRepresentation::CollectionView(CollectionViewKind::Array)
    );
    let mutable = vela_callable_contract_byte_slice_bump();
    assert_eq!(
        mutable.parameters[0]
            .binding
            .expect("mutable byte-slice binding")
            .representation,
        InteropRepresentation::CollectionMut {
            kind: CollectionViewKind::Array,
            mutation: CollectionViewMutation::Fixed,
        }
    );
    let growable = vela_callable_contract_byte_vec_push();
    assert_eq!(
        growable.parameters[0]
            .binding
            .expect("mutable byte-vector binding")
            .representation,
        InteropRepresentation::CollectionMut {
            kind: CollectionViewKind::Array,
            mutation: CollectionViewMutation::Growable,
        }
    );

    let mut runtime = runtime(
        "fn read(values: ArrayView<u8>) { return collections::byte_slice_sum(values); } fn write(values: ArrayMut<u8>) { values[0] = 7u8; return collections::byte_slice_bump(values); } fn grow(values: ArrayMut<u8>) { return collections::byte_vec_push(values, 9u8); } fn returned(owner: ByteOwner) { let values = owner.bytes_mut(); values[0] = 8u8; return collections::byte_slice_bump(values); }",
    );
    let values = [2_u8, 3, 5];
    let mut args = CallArgs::new();
    args.push_slice_ref("values", &values);
    let result = runtime
        .call("read", args, CallOptions::unbounded())
        .expect("shared byte slice should reborrow without copying");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(10)));
    drop(result);

    let mut values = [2_u8, 3, 5];
    let mut args = CallArgs::new();
    args.push_slice_mut("values", &mut values);
    let result = runtime
        .call("write", args, CallOptions::unbounded())
        .expect("mutable byte slice should write through HostAccess");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(7)));
    drop(result);
    assert_eq!(values, [7, 7, 5]);

    let mut values = vec![2_u8, 3];
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut values);
    let result = runtime
        .call("grow", args, CallOptions::unbounded())
        .expect("mutable byte vector should retain growable view capability");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(3)));
    drop(result);
    assert_eq!(values, vec![2, 3, 9]);

    let mut owner = ByteOwner {
        bytes: vec![2, 3, 5],
    };
    let result = runtime
        .call(
            "returned",
            CallArgs::new().with_host_mut("owner", &mut owner),
            CallOptions::unbounded(),
        )
        .expect("returned mutable byte slice should retain and reborrow its parent lease");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(7)));
    drop(result);
    assert_eq!(owner.bytes, vec![8, 7, 5]);
}

#[test]
fn returned_slices_reborrow_into_other_rust_exports() {
    let mut runtime = runtime(
        "fn shared(owner: CollectionOwner) { let values = owner.slice(); return collections::slice_sum(values); } fn exclusive(owner: CollectionOwner) { let values = owner.slice_mut(); values[0] = 6; return collections::slice_bump(values); }",
    );
    let owner = CollectionOwner {
        values: vec![2_i64, 3, 5],
        totals: BTreeMap::new(),
    };
    let result = runtime
        .call(
            "shared",
            CallArgs::new().with_host_ref("owner", &owner),
            CallOptions::unbounded(),
        )
        .expect("returned shared slice should retain and reborrow its parent lease");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(10)));
    drop(result);

    let mut owner = CollectionOwner {
        values: vec![2_i64, 3, 5],
        totals: BTreeMap::new(),
    };
    let result = runtime
        .call(
            "exclusive",
            CallArgs::new().with_host_mut("owner", &mut owner),
            CallOptions::unbounded(),
        )
        .expect("returned mutable slice should retain and reborrow its parent lease");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(7)));
    drop(result);
    assert_eq!(owner.values, [6, 7, 5]);
}

#[test]
fn generated_method_adapter_reborrows_collection_arguments() {
    let mut runtime = runtime(
        "fn main(service: CollectionService, values: ArrayView<i64>, totals: MapMut<String, i64>) { return service.merge(values, totals); }",
    );
    let service = CollectionService { offset: 4 };
    let values = vec![2_i64, 3];
    let mut totals = BTreeMap::<String, i64>::new();
    let mut args = CallArgs::new();
    args.push_host_ref("service", &service)
        .push_collection_ref("values", &values)
        .push_collection_mut("totals", &mut totals);

    let result = runtime
        .call("main", args, CallOptions::unbounded())
        .expect("generated method adapter should execute");

    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(9)));
    drop(result);
    assert_eq!(totals["sum"], 9);
}

#[test]
fn borrowed_collection_return_reborrows_into_another_rust_export() {
    let mut runtime = runtime(
        "fn shared(owner: CollectionOwner, totals: MapMut<String, i64>) { let values = owner.values(); return collections::merge(values, totals); } fn exclusive(owner: CollectionOwner) { let totals = owner.totals_mut(); return collections::add(totals, 5); }",
    );
    let shared_owner = CollectionOwner {
        values: vec![4_i64, 6],
        totals: BTreeMap::new(),
    };
    let mut external_totals = BTreeMap::<String, i64>::new();
    let mut shared_args = CallArgs::new();
    shared_args
        .push_host_ref("owner", &shared_owner)
        .push_collection_mut("totals", &mut external_totals);

    let result = runtime
        .call("shared", shared_args, CallOptions::unbounded())
        .expect("shared borrowed return should reborrow into a Rust export");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(10)));
    drop(result);
    assert_eq!(external_totals["sum"], 10);

    let mut exclusive_owner = CollectionOwner {
        values: Vec::new(),
        totals: BTreeMap::from([("sum".to_owned(), 2_i64)]),
    };
    let result = runtime
        .call(
            "exclusive",
            CallArgs::new().with_host_mut("owner", &mut exclusive_owner),
            CallOptions::unbounded(),
        )
        .expect("exclusive borrowed return should reborrow into a Rust export");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(7)));
    drop(result);
    assert_eq!(exclusive_owner.totals["sum"], 7);
}

#[test]
fn borrowed_collections_execute_read_only_protocols_from_vela() {
    let mut runtime = runtime(
        "fn direct(values: ArrayView<i64>, totals: MapMut<String, i64>) { return values.len() + totals.len(); } fn empty(values: ArrayView<i64>) { return values.is_empty(); } fn returned(owner: CollectionOwner) { let values = owner.values(); return values.len(); }",
    );
    let values = vec![2_i64, 3, 5];
    let mut totals = BTreeMap::from([("sum".to_owned(), 10_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values)
        .push_collection_mut("totals", &mut totals);

    let result = runtime
        .call("direct", args, CallOptions::unbounded())
        .expect("borrowed collection protocols should execute through HostAccess");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(4)));
    drop(result);

    let empty_values = Vec::<i64>::new();
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &empty_values);
    let result = runtime
        .call("empty", args, CallOptions::unbounded())
        .expect("shared collection is_empty should use the read-only protocol");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
    drop(result);

    let owner = CollectionOwner {
        values: vec![4_i64, 6],
        totals: BTreeMap::new(),
    };
    let result = runtime
        .call(
            "returned",
            CallArgs::new().with_host_ref("owner", &owner),
            CallOptions::unbounded(),
        )
        .expect("owner-frozen borrowed return should keep its collection protocol");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(2)));
}

#[test]
fn borrowed_collection_indexes_read_and_write_through_host_access() {
    let mut runtime = runtime(
        "fn array_read(values: ArrayView<i64>) { return values[1]; } fn array_write(values: ArrayMut<i64>) { values[1] = values[0] + 5; return values[1]; } fn shared_write(values: ArrayView<i64>) { values[0] = 99; } fn map_read(totals: MapView<String, i64>) { return totals[\"sum\"]; } fn map_write(totals: MapMut<String, i64>) { totals[\"sum\"] = totals[\"sum\"] + 3; return totals[\"sum\"]; } fn returned_read(owner: CollectionOwner) { let values = owner.values(); return values[1]; } fn returned_write(owner: CollectionOwner) { let totals = owner.totals_mut(); totals[\"sum\"] = totals[\"sum\"] + 4; return totals[\"sum\"]; }",
    );

    let values = vec![2_i64, 3];
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call("array_read", args, CallOptions::unbounded())
        .expect("shared array index should read through HostAccess");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(3)));
    drop(result);

    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    assert!(
        runtime
            .call("shared_write", args, CallOptions::unbounded())
            .is_err(),
        "shared array index assignment must fail closed"
    );
    assert_eq!(values, vec![2, 3]);

    let mut values = vec![2_i64, 3];
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut values);
    let result = runtime
        .call("array_write", args, CallOptions::unbounded())
        .expect("exclusive array index should write through HostAccess");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(7)));
    drop(result);
    assert_eq!(values, vec![2, 7]);

    let totals = BTreeMap::from([("sum".to_owned(), 8_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_ref("totals", &totals);
    let result = runtime
        .call("map_read", args, CallOptions::unbounded())
        .expect("shared map index should read through HostAccess");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(8)));
    drop(result);

    let mut totals = totals;
    let mut args = CallArgs::new();
    args.push_collection_mut("totals", &mut totals);
    let result = runtime
        .call("map_write", args, CallOptions::unbounded())
        .expect("exclusive map index should write through HostAccess");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(11)));
    drop(result);
    assert_eq!(totals["sum"], 11);

    let owner = CollectionOwner {
        values: vec![4_i64, 6],
        totals: BTreeMap::new(),
    };
    let result = runtime
        .call(
            "returned_read",
            CallArgs::new().with_host_ref("owner", &owner),
            CallOptions::unbounded(),
        )
        .expect("retained shared collection index should read through HostAccess");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(6)));
    drop(result);

    let mut owner = CollectionOwner {
        values: Vec::new(),
        totals: BTreeMap::from([("sum".to_owned(), 2_i64)]),
    };
    let result = runtime
        .call(
            "returned_write",
            CallArgs::new().with_host_mut("owner", &mut owner),
            CallOptions::unbounded(),
        )
        .expect("retained exclusive collection index should write through HostAccess");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(6)));
    drop(result);
    assert_eq!(owner.totals["sum"], 6);
}

#[test]
fn borrowed_map_indexes_preserve_exact_integer_key_types() {
    let mut runtime = runtime(
        "fn read(scores: MapView<i32, i64>) { return scores[7i32]; } fn write(scores: MapMut<i32, i64>) { scores[7i32] = scores[7i32] + 5; return scores[7i32]; }",
    );

    let scores = BTreeMap::from([(7_i32, 11_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_ref("scores", &scores);
    let result = runtime
        .call("read", args, CallOptions::unbounded())
        .expect("i32 map key should retain its exact host boundary type");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(11)));
    drop(result);

    let mut scores = scores;
    let mut args = CallArgs::new();
    args.push_collection_mut("scores", &mut scores);
    let result = runtime
        .call("write", args, CallOptions::unbounded())
        .expect("i32 map key assignment should write through HostAccess");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(16)));
    drop(result);
    assert_eq!(scores[&7], 16);
}

#[test]
fn borrowed_collection_lookup_methods_use_host_paths_without_materializing() {
    let mut runtime = runtime(
        "fn array_lookup(values: ArrayView<i64>) { return values.first().unwrap_or(4) == 11 && values.last().unwrap_or(4) == 13; } fn array_empty(values: ArrayView<i64>) { return values.first().unwrap_or(4) == 4 && values.last().unwrap_or(6) == 6; } fn retained(owner: CollectionOwner) { let values = owner.values(); return values.first().unwrap_or(0) + values.last().unwrap_or(0); } fn map_lookup(scores: MapView<i32, i64>) { return scores.contains_key(7i32) && !scores.contains_key(9i32) && scores.has(7i32) && scores.get(7i32).unwrap_or(0) == 11 && scores.get(9i32).unwrap_or(4) == 4 && scores.get_or(9i32, 6) == 6; } fn set_lookup(values: SetView<i32>) { return values.contains(7i32) && !values.contains(9i32) && values.has(7i32); }",
    );

    let values = vec![11_i64, 12, 13];
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call("array_lookup", args, CallOptions::unbounded())
        .expect("borrowed array first/last should execute through HostAccess");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
    drop(result);

    let owner = CollectionOwner {
        values: vec![11, 12, 13],
        totals: BTreeMap::new(),
    };
    let result = runtime
        .call(
            "retained",
            CallArgs::new().with_host_ref("owner", &owner),
            CallOptions::unbounded(),
        )
        .expect("retained borrowed array first/last should use the parent lease");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(24)));
    drop(result);

    let values = Vec::<i64>::new();
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call("array_empty", args, CallOptions::unbounded())
        .expect("empty borrowed array first/last should return Option::None");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
    drop(result);

    let scores = BTreeMap::from([(7_i32, 11_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_ref("scores", &scores);
    let result = runtime
        .call("map_lookup", args, CallOptions::unbounded())
        .expect("borrowed map lookup methods should execute through HostAccess");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
    drop(result);

    let values = BTreeSet::from([7_i32]);
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call("set_lookup", args, CallOptions::unbounded())
        .expect("borrowed set has should execute through HostAccess");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
}

#[test]
fn growable_borrowed_collection_methods_write_through_host_paths() {
    let mut runtime = runtime(
        "fn array_remove(owner: CollectionOwner) { let values = owner.values_mut(); let result = values.remove_at(1).unwrap_or(0); host::release(values); return result; } fn array_missing(owner: CollectionOwner) { let values = owner.values_mut(); let result = values.remove_at(9).unwrap_or(4); host::release(values); return result; } fn array_push(owner: CollectionOwner) { let values = owner.values_mut(); values.push(13); let result = values.last().unwrap_or(0); host::release(values); return result; } fn array_pop(owner: CollectionOwner) { let values = owner.values_mut(); let result = values.pop().unwrap_or(4); host::release(values); return result; } fn array_insert(owner: CollectionOwner) { let values = owner.values_mut(); values.insert(1, 7); values.insert(values.len(), 17); let result = values.len(); host::release(values); return result; } fn array_insert_missing(owner: CollectionOwner) { let values = owner.values_mut(); values.insert(9, 23); host::release(values); } fn map_set(scores: MapMut<i32, i64>) { scores.set(7i32, 12); scores.set(9i32, 6); scores[10i32] = 8; if !scores.contains_key(9i32) || scores.contains_key(8i32) { return 0; } return scores.remove(7i32).unwrap_or(0) + scores.remove(8i32).unwrap_or(4) + scores[9i32] + scores[10i32]; } fn set_mutate(values: SetMut<i32>) { return values.insert(9i32) && values.contains(9i32) && !values.contains(8i32) && !values.insert(7i32) && values.remove(7i32) && !values.remove(8i32); }",
    );

    let mut owner = CollectionOwner {
        values: vec![5, 7, 11],
        totals: BTreeMap::new(),
    };
    let result = runtime
        .call(
            "array_remove",
            CallArgs::new().with_host_mut("owner", &mut owner),
            CallOptions::unbounded(),
        )
        .expect("growable borrowed array remove_at should write through HostAccess");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(7)));
    drop(result);
    assert_eq!(owner.values, vec![5, 11]);

    let result = runtime
        .call(
            "array_missing",
            CallArgs::new().with_host_mut("owner", &mut owner),
            CallOptions::unbounded(),
        )
        .expect("out-of-range borrowed array remove_at should return Option::None");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(4)));
    drop(result);
    assert_eq!(owner.values, vec![5, 11]);

    let result = runtime
        .call(
            "array_push",
            CallArgs::new().with_host_mut("owner", &mut owner),
            CallOptions::unbounded(),
        )
        .expect("growable borrowed array push should write through HostAccess");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(13)));
    drop(result);
    assert_eq!(owner.values, vec![5, 11, 13]);

    let result = runtime
        .call(
            "array_pop",
            CallArgs::new().with_host_mut("owner", &mut owner),
            CallOptions::unbounded(),
        )
        .expect("growable borrowed array pop should write through HostAccess");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(13)));
    drop(result);
    assert_eq!(owner.values, vec![5, 11]);

    let result = runtime
        .call(
            "array_insert",
            CallArgs::new().with_host_mut("owner", &mut owner),
            CallOptions::unbounded(),
        )
        .expect("growable borrowed array insert should write through HostAccess");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(4)));
    drop(result);
    assert_eq!(owner.values, vec![5, 7, 11, 17]);

    let error = runtime
        .call(
            "array_insert_missing",
            CallArgs::new().with_host_mut("owner", &mut owner),
            CallOptions::unbounded(),
        )
        .expect_err("sparse borrowed array insert must fail");
    assert!(matches!(
        error.kind(),
        VmErrorKind::IndexOutOfBounds { index: 9, len: 4 }
    ));
    assert_eq!(owner.values, vec![5, 7, 11, 17]);

    owner.values.clear();
    let result = runtime
        .call(
            "array_pop",
            CallArgs::new().with_host_mut("owner", &mut owner),
            CallOptions::unbounded(),
        )
        .expect("empty borrowed array pop should return Option::None");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(4)));
    drop(result);
    assert!(owner.values.is_empty());

    let mut scores = BTreeMap::from([(7_i32, 11_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_mut("scores", &mut scores);
    let result = runtime
        .call("map_set", args, CallOptions::unbounded())
        .expect("growable borrowed map set should write through HostAccess");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(30)));
    drop(result);
    assert_eq!(scores, BTreeMap::from([(9, 6), (10, 8)]));

    let mut values = BTreeSet::from([7_i32]);
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut values);
    let result = runtime
        .call("set_mutate", args, CallOptions::unbounded())
        .expect("growable borrowed set methods should write through HostAccess");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
    drop(result);
    assert_eq!(values, BTreeSet::from([9]));
}

#[test]
fn borrowed_array_push_charges_before_mutation() {
    let mut runtime = runtime(
        "fn push(owner: CollectionOwner) { let values = owner.values_mut(); values.push(13); host::release(values); }",
    );
    let minimum = minimum_owner_call_limit(&mut runtime, "push");

    let mut owner = CollectionOwner {
        values: vec![5],
        totals: BTreeMap::new(),
    };
    assert!(
        runtime
            .call(
                "push",
                CallArgs::new().with_host_mut("owner", &mut owner),
                CallOptions::new(minimum - 2, usize::MAX, usize::MAX),
            )
            .is_err(),
        "the budget before mutation and explicit release must reject the push"
    );
    assert_eq!(
        owner.values,
        vec![5],
        "budget failure must precede mutation"
    );

    runtime
        .call(
            "push",
            CallArgs::new().with_host_mut("owner", &mut owner),
            CallOptions::new(minimum, usize::MAX, usize::MAX),
        )
        .expect("the minimum observed push budget should succeed");
    assert_eq!(owner.values, vec![5, 13]);
}

#[test]
fn borrowed_array_insert_charges_before_mutation() {
    let mut runtime = runtime(
        "fn insert(owner: CollectionOwner) { let values = owner.values_mut(); values.insert(0, 13); host::release(values); }",
    );
    let minimum = minimum_owner_call_limit(&mut runtime, "insert");
    let mut owner = CollectionOwner {
        values: vec![5],
        totals: BTreeMap::new(),
    };

    assert!(
        runtime
            .call(
                "insert",
                CallArgs::new().with_host_mut("owner", &mut owner),
                CallOptions::new(minimum - 2, usize::MAX, usize::MAX),
            )
            .is_err(),
        "the budget before insertion and explicit release must reject the insertion"
    );
    assert_eq!(
        owner.values,
        vec![5],
        "budget failure must precede insertion"
    );

    runtime
        .call(
            "insert",
            CallArgs::new().with_host_mut("owner", &mut owner),
            CallOptions::new(minimum, usize::MAX, usize::MAX),
        )
        .expect("the minimum observed insertion budget should succeed");
    assert_eq!(owner.values, vec![13, 5]);
}

fn minimum_owner_call_limit(runtime: &mut Runtime, function: &str) -> u64 {
    (2..128)
        .find(|limit| {
            let mut owner = CollectionOwner {
                values: vec![5],
                totals: BTreeMap::new(),
            };
            runtime
                .call(
                    function,
                    CallArgs::new().with_host_mut("owner", &mut owner),
                    CallOptions::new(*limit, usize::MAX, usize::MAX),
                )
                .is_ok()
        })
        .expect("one host array growth operation should fit a small bounded call")
}

#[test]
fn growable_borrowed_collections_clear_through_one_host_mutation() {
    let mut runtime = runtime(
        "fn clear_map(values: MapMut<i32, i64>) { let before = values.len(); values.clear(); return before + values.len(); } fn clear_set(values: SetMut<i32>) { let before = values.len(); values.clear(); return before + values.len(); }",
    );

    let mut map = BTreeMap::from([(3_i32, 5_i64), (8, 13)]);
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut map);
    let result = runtime
        .call("clear_map", args, CallOptions::unbounded())
        .expect("borrowed map clear should use the host collection mutation protocol");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(2)));
    drop(result);
    assert!(map.is_empty());

    let mut set = BTreeSet::from([3_i32, 5, 8]);
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut set);
    let result = runtime
        .call("clear_set", args, CallOptions::unbounded())
        .expect("borrowed set clear should use the host collection mutation protocol");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(3)));
    drop(result);
    assert!(set.is_empty());
}

#[test]
fn borrowed_collection_clear_charges_size_before_mutation() {
    let mut runtime = runtime("fn clear_map(values: MapMut<i32, i64>) { values.clear(); }");
    let base_limit = (0..64)
        .find(|limit| {
            let mut values = BTreeMap::<i32, i64>::new();
            let mut args = CallArgs::new();
            args.push_collection_mut("values", &mut values);
            runtime
                .call(
                    "clear_map",
                    args,
                    CallOptions::new(*limit, usize::MAX, usize::MAX),
                )
                .is_ok()
        })
        .expect("empty host clear should fit a small bounded call");

    let expected = BTreeMap::from([(3_i32, 5_i64), (8, 13), (21, 34)]);
    let mut values = expected.clone();
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut values);
    assert!(
        runtime
            .call(
                "clear_map",
                args,
                CallOptions::new(base_limit + 2, usize::MAX, usize::MAX),
            )
            .is_err(),
        "three removed entries must cost three execution units"
    );
    assert_eq!(
        values, expected,
        "budget failure must precede host mutation"
    );

    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut values);
    runtime
        .call(
            "clear_map",
            args,
            CallOptions::new(base_limit + 3, usize::MAX, usize::MAX),
        )
        .expect("exact size-aware clear budget should succeed");
    assert!(values.is_empty());
}

#[test]
fn growable_borrowed_collections_extend_from_owned_values_in_one_batch() {
    let mut runtime = runtime(
        "fn extend_map(values: MapMut<i32, i64>) { let extension = [MapEntry { key: 3i32, value: 8 }, MapEntry { key: 5i32, value: 13 }].iter().collect_map(); values.extend(extension); return values[3i32] + values[5i32] + values.len(); } fn extend_set(values: SetMut<i32>) { values.extend(set::from_array([3i32, 5i32])); return values.has(2i32) && values.has(3i32) && values.has(5i32) && values.len() == 3; }",
    );

    let mut map = BTreeMap::from([(3_i32, 5_i64)]);
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut map);
    let result = runtime
        .call("extend_map", args, CallOptions::unbounded())
        .expect("borrowed map extend should cross one host mutation batch");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(23)));
    drop(result);
    assert_eq!(map, BTreeMap::from([(3, 8), (5, 13)]));

    let mut set = BTreeSet::from([2_i32, 3]);
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut set);
    let result = runtime
        .call("extend_set", args, CallOptions::unbounded())
        .expect("borrowed set extend should cross one host mutation batch");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
    drop(result);
    assert_eq!(set, BTreeSet::from([2, 3, 5]));
}

#[test]
fn borrowed_collection_extend_charges_batch_size_before_mutation() {
    let mut runtime = runtime(
        "fn extend_map(values: MapMut<i32, i64>, extension: Map<i32, i64>) { values.extend(extension); }",
    );
    let base_limit = (0..64)
        .find(|limit| {
            let mut values = BTreeMap::<i32, i64>::new();
            let mut args = CallArgs::new();
            args.push_collection_mut("values", &mut values);
            args.push_value("extension", OwnedValue::map(Vec::<(i32, i64)>::new()));
            runtime
                .call(
                    "extend_map",
                    args,
                    CallOptions::new(*limit, usize::MAX, usize::MAX),
                )
                .is_ok()
        })
        .expect("empty host extension should fit a small bounded call");

    let extension = || OwnedValue::map([(3_i32, 5_i64), (8, 13), (21, 34)]);
    let mut values = BTreeMap::from([(1_i32, 2_i64)]);
    let expected = values.clone();
    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut values);
    args.push_value("extension", extension());
    assert!(
        runtime
            .call(
                "extend_map",
                args,
                CallOptions::new(base_limit + 2, usize::MAX, usize::MAX),
            )
            .is_err(),
        "three inserted entries must cost three execution units"
    );
    assert_eq!(
        values, expected,
        "budget failure must precede host extension"
    );

    let mut args = CallArgs::new();
    args.push_collection_mut("values", &mut values);
    args.push_value("extension", extension());
    runtime
        .call(
            "extend_map",
            args,
            CallOptions::new(base_limit + 3, usize::MAX, usize::MAX),
        )
        .expect("exact size-aware extension budget should succeed");
    assert_eq!(values, BTreeMap::from([(1, 2), (3, 5), (8, 13), (21, 34)]));
}

#[path = "borrowed_collection_exports/projection_tests.rs"]
mod borrowed_collection_projection_tests;
#[path = "borrowed_collection_exports/array_callback_tests.rs"]
mod borrowed_host_array_callback_tests;
#[path = "borrowed_collection_exports/array_search_tests.rs"]
mod borrowed_host_array_search_tests;
#[path = "borrowed_collection_exports/host_iterator_tests.rs"]
mod borrowed_host_iterator_tests;
#[path = "borrowed_collection_exports/keyed_callback_tests.rs"]
mod borrowed_host_keyed_callback_tests;

#[path = "borrowed_collection_exports/async_tests.rs"]
mod borrowed_collection_async_tests;

fn runtime(source: &str) -> Runtime {
    let engine = collection_engine();
    let program = engine
        .compile_source(source)
        .expect("borrowed collection call should compile");
    Runtime::new(engine, program).expect("runtime should initialize")
}

fn collection_engine() -> Engine {
    Engine::builder()
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .register_type::<CollectionService>()
        .register_type::<CollectionOwner>()
        .register_type::<ByteOwner>()
        .register_exports(vela_export_bundle_merge())
        .register_exports(vela_export_bundle_add())
        .register_exports(vela_export_bundle_lookup_i32())
        .register_exports(vela_export_bundle_contains_i32())
        .register_exports(vela_export_bundle_retain_non_two())
        .register_exports(vela_export_bundle_merge_async())
        .register_exports(vela_export_bundle_fixed_sum())
        .register_exports(vela_export_bundle_fixed_bump())
        .register_exports(vela_export_bundle_slice_sum())
        .register_exports(vela_export_bundle_slice_bump())
        .register_exports(vela_export_bundle_byte_slice_sum())
        .register_exports(vela_export_bundle_byte_slice_bump())
        .register_exports(vela_export_bundle_byte_vec_push())
        .register_exports(vela_export_bundle_slice_sum_async())
        .register_exports(vela_export_bundle_slice_pair_sum())
        .register_exports(vela_export_bundle_slice_pair_mut())
        .register_exports(vela_export_bundle_slice_len_mut())
        .register_exports(vela_export_bundle_slice_fail())
        .register_exports(vela_export_bundle_slice_yield_once())
        .register_exports(vela_export_bundle_slice_wait())
        .register_exports(vela_export_bundle_slice_panic_async())
        .register_exports(vela_export_bundle_slice_gate())
        .register_exports(vela_export_bundle_slice_reenter())
        .register_exports(CollectionService::vela_inherent_exports())
        .register_exports(CollectionOwner::vela_inherent_exports())
        .register_exports(ByteOwner::vela_inherent_exports())
        .build()
        .expect("collection bindings should be registered transitively")
}
