#![allow(clippy::ptr_arg)] // The boundary contract intentionally distinguishes &Vec from slices.

use std::collections::BTreeMap;

use vela_common::{CollectionViewKind, CollectionViewMutation, InteropRepresentation};
use vela_engine::engine::Engine;
use vela_engine::interop::BoundaryMode;
use vela_engine::native::TypeHint;
use vela_engine::permission::Capability;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_macros::{ScriptHost, export, methods};
use vela_vm::owned_value::OwnedValue;

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

#[export(path = "collections::merge_async")]
pub async fn merge_async(values: &Vec<i64>, totals: &mut BTreeMap<String, i64>) -> i64 {
    merge(values, totals)
}

#[derive(ScriptHost)]
#[script(path = "host::CollectionService")]
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
}

#[derive(ScriptHost)]
#[script(path = "host::CollectionOwner")]
struct CollectionOwner {
    values: Vec<i64>,
    totals: BTreeMap<String, i64>,
}

#[methods(path = "host::CollectionOwner")]
impl CollectionOwner {
    pub fn values(&self) -> &Vec<i64> {
        &self.values
    }

    pub fn totals_mut(&mut self) -> &mut BTreeMap<String, i64> {
        &mut self.totals
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

fn runtime(source: &str) -> Runtime {
    let engine = Engine::builder()
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .register_host_type::<CollectionService>()
        .register_host_type::<CollectionOwner>()
        .register_exports(vela_export_bundle_merge())
        .register_exports(vela_export_bundle_add())
        .register_exports(vela_export_bundle_merge_async())
        .register_exports(CollectionService::vela_inherent_exports())
        .register_exports(CollectionOwner::vela_inherent_exports())
        .build()
        .expect("collection bindings should be registered transitively");
    let program = engine
        .compile_source(source)
        .expect("borrowed collection call should compile");
    Runtime::new(engine, program).expect("runtime should initialize")
}
