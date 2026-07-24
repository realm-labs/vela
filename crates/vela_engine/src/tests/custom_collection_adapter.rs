use std::any::Any;
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

use vela_common::{
    CollectionViewCapabilities, CollectionViewKind, CollectionViewMutation, HostTypeId,
};
use vela_def::TypeId;
use vela_host::error::HostResult;
use vela_host::object::ScriptHostObject;
use vela_host::protocol::{
    HostCollectionMutation, HostCollectionProjection, HostCollectionQuery, HostCollectionSnapshot,
};
use vela_host::resolved::{HostAccessSpec, HostMutationOp, ResolvedHostAccess};
use vela_host::target::HostTargetInstance;
use vela_host::value::HostValue;
use vela_reflect::registry::{
    HostIndexCapability, SchemaHash, TraitDesc, TypeDesc, TypeKey, TypeKind,
};
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::permission::Capability;
use crate::runtime::{CallArgs, CallOptions, Runtime};
use crate::type_binding::TypeBinding;

const TIMELINE_HOST_TYPE: HostTypeId = HostTypeId::new(0x7469_6d65);
const LEDGER_HOST_TYPE: HostTypeId = HostTypeId::new(0x6c65_6467);
const TAG_SET_HOST_TYPE: HostTypeId = HostTypeId::new(0x7461_6773);

thread_local! {
    static HOST_TARGET_RESOLUTIONS: Cell<usize> = const { Cell::new(0) };
}

/// One application-defined collection that deliberately is not a standard
/// Vec binding. Its adapter delegates storage mechanics to Vec while exposing
/// only the semantic collection protocol to the VM.
struct Timeline(Vec<i64>);

struct Ledger(BTreeMap<i32, i64>);

struct TagSet(BTreeSet<i32>);

macro_rules! delegate_collection_protocol {
    ($outer:ty, $inner:ty, $host_type:expr) => {
        impl ScriptHostObject for $outer {
            fn host_type_id(&self) -> HostTypeId {
                $host_type
            }

            fn resolve_host_type_target(
                spec: HostAccessSpec<'_>,
            ) -> HostResult<ResolvedHostAccess> {
                <$inner as ScriptHostObject>::resolve_host_type_target(spec)
            }

            fn lease_any(&self) -> Option<&dyn Any> {
                Some(self)
            }

            fn lease_any_mut(&mut self) -> Option<&mut dyn Any> {
                Some(self)
            }

            fn resolve_host_target(
                &self,
                spec: HostAccessSpec<'_>,
            ) -> HostResult<ResolvedHostAccess> {
                HOST_TARGET_RESOLUTIONS.with(|count| count.set(count.get() + 1));
                self.0.resolve_host_target(spec)
            }

            fn read_resolved_host(
                &self,
                access: ResolvedHostAccess,
                target: HostTargetInstance<'_>,
            ) -> HostResult<HostValue> {
                self.0.read_resolved_host(access, target)
            }

            fn query_collection_resolved_host(
                &self,
                access: ResolvedHostAccess,
                target: HostTargetInstance<'_>,
                query: HostCollectionQuery,
            ) -> HostResult<HostValue> {
                self.0.query_collection_resolved_host(access, target, query)
            }

            fn snapshot_collection_resolved_host(
                &self,
                access: ResolvedHostAccess,
                target: HostTargetInstance<'_>,
                projection: HostCollectionProjection,
            ) -> HostResult<HostCollectionSnapshot> {
                self.0
                    .snapshot_collection_resolved_host(access, target, projection)
            }

            fn mutate_collection_resolved_host(
                &mut self,
                access: ResolvedHostAccess,
                target: HostTargetInstance<'_>,
                mutation: HostCollectionMutation<'_>,
            ) -> HostResult<()> {
                self.0
                    .mutate_collection_resolved_host(access, target, mutation)
            }

            fn write_resolved_host(
                &mut self,
                access: ResolvedHostAccess,
                target: HostTargetInstance<'_>,
                value: HostValue,
            ) -> HostResult<()> {
                self.0.write_resolved_host(access, target, value)
            }

            fn mutate_resolved_host(
                &mut self,
                access: ResolvedHostAccess,
                target: HostTargetInstance<'_>,
                op: HostMutationOp,
                rhs: HostValue,
            ) -> HostResult<()> {
                self.0.mutate_resolved_host(access, target, op, rhs)
            }

            fn remove_resolved_host(
                &mut self,
                access: ResolvedHostAccess,
                target: HostTargetInstance<'_>,
            ) -> HostResult<()> {
                self.0.remove_resolved_host(access, target)
            }
        }
    };
}

delegate_collection_protocol!(Timeline, Vec<i64>, TIMELINE_HOST_TYPE);
delegate_collection_protocol!(Ledger, BTreeMap<i32, i64>, LEDGER_HOST_TYPE);
delegate_collection_protocol!(TagSet, BTreeSet<i32>, TAG_SET_HOST_TYPE);

fn timeline_binding() -> TypeBinding<Timeline> {
    let desc = TypeDesc::new(TypeKey::new(TypeId::new(0x7469_6d65), "host::Timeline"))
        .kind(TypeKind::Array)
        .schema_hash(SchemaHash::new(0x7469_6d65))
        .host_type(TIMELINE_HOST_TYPE)
        .trait_impl(TraitDesc::new("Sequence"))
        .trait_impl(TraitDesc::new("Iterable"))
        .index_capability(
            HostIndexCapability::new()
                .readable(true)
                .writable(true)
                .addable(true)
                .removable(true)
                .key_type("i64")
                .value_type("i64"),
        );
    TypeBinding::host(desc).collection_view_capabilities(CollectionViewCapabilities::mutable(
        CollectionViewKind::Array,
        CollectionViewMutation::Growable,
    ))
}

fn ledger_binding() -> TypeBinding<Ledger> {
    let desc = TypeDesc::new(TypeKey::new(TypeId::new(0x6c65_6467), "host::Ledger"))
        .kind(TypeKind::Map)
        .schema_hash(SchemaHash::new(0x6c65_6467))
        .host_type(LEDGER_HOST_TYPE)
        .trait_impl(TraitDesc::new("MapLike"))
        .index_capability(
            HostIndexCapability::new()
                .readable(true)
                .writable(true)
                .addable(true)
                .removable(true)
                .key_type("i32")
                .value_type("i64"),
        );
    TypeBinding::host(desc).collection_view_capabilities(CollectionViewCapabilities::mutable(
        CollectionViewKind::Map,
        CollectionViewMutation::Growable,
    ))
}

fn tag_set_binding() -> TypeBinding<TagSet> {
    let desc = TypeDesc::new(TypeKey::new(TypeId::new(0x7461_6773), "host::TagSet"))
        .kind(TypeKind::Set)
        .schema_hash(SchemaHash::new(0x7461_6773))
        .host_type(TAG_SET_HOST_TYPE)
        .trait_impl(TraitDesc::new("SetLike"))
        .trait_impl(TraitDesc::new("Iterable"));
    TypeBinding::host(desc).collection_view_capabilities(CollectionViewCapabilities::mutable(
        CollectionViewKind::Set,
        CollectionViewMutation::Growable,
    ))
}

fn runtime(source: &str) -> Runtime {
    let engine = Engine::builder()
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .register_rust_type(timeline_binding())
        .register_rust_type(ledger_binding())
        .register_rust_type(tag_set_binding())
        .build()
        .expect("custom collection binding should seal");
    let program = engine
        .compile_source(source)
        .expect("custom collection protocol fixture should compile");
    Runtime::new(engine, program).expect("custom collection runtime should initialize")
}

#[test]
fn user_defined_sequence_reuses_live_queries_iteration_and_callbacks() {
    let mut runtime = runtime(
        "fn inspect(values) { \
             let selected = values.filter(|value| value >= 5); \
             return values[1] + selected.sum() + values.iter().count(); \
         }",
    );
    let values = Timeline(vec![2, 5, 8]);
    let result = runtime
        .call(
            "inspect",
            CallArgs::new().with_host_ref("values", &values),
            CallOptions::unbounded(),
        )
        .expect("custom shared sequence should use the standard protocol surface");

    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(21)));
    assert_eq!(values.0, vec![2, 5, 8]);
}

#[test]
fn user_defined_sequence_mutations_write_through_and_keep_transactional_retain() {
    let mut runtime = runtime(
        "fn mutate(values) { \
             values[1] += 3; \
             values.extend([13, 21]); \
             values.retain(|value| value >= 8); \
             return values.len() + values[0]; \
         } \
         fn reject_shared(values) { values.clear(); }",
    );
    let mut values = Timeline(vec![2, 5, 8]);
    let result = runtime
        .call(
            "mutate",
            CallArgs::new().with_host_mut("values", &mut values),
            CallOptions::unbounded(),
        )
        .expect("custom exclusive sequence should write through semantic mutations");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(12)));
    drop(result);
    assert_eq!(values.0, vec![8, 8, 13, 21]);

    let shared = Timeline(vec![3, 5]);
    let error = runtime
        .call(
            "reject_shared",
            CallArgs::new().with_host_ref("values", &shared),
            CallOptions::unbounded(),
        )
        .expect_err("shared custom sequence must not expose structural mutation");
    assert!(matches!(
        error.kind(),
        vela_vm::error::VmErrorKind::UnknownMethod { method } if method == "clear"
    ));
    assert_eq!(shared.0, vec![3, 5]);
}

#[test]
fn user_defined_sequence_bulk_budget_failure_precedes_mutation() {
    let mut runtime = runtime("fn extend(values) { values.extend([3, 5, 8]); }");
    let baseline = (0..96)
        .find(|limit| {
            let mut values = Timeline(vec![2]);
            runtime
                .call(
                    "extend",
                    CallArgs::new().with_host_mut("values", &mut values),
                    CallOptions::new(*limit, usize::MAX, usize::MAX),
                )
                .is_ok()
        })
        .expect("custom sequence extension should fit a bounded call");

    let mut values = Timeline(vec![2]);
    let error = runtime
        .call(
            "extend",
            CallArgs::new().with_host_mut("values", &mut values),
            CallOptions::new(baseline - 1, usize::MAX, usize::MAX),
        )
        .expect_err("one unit below the complete bulk budget must reject the call");
    assert!(matches!(
        error.kind(),
        vela_vm::error::VmErrorKind::BudgetExceeded { .. }
    ));
    assert_eq!(
        values.0,
        vec![2],
        "budget failure must happen before the custom adapter mutates"
    );
}

#[test]
fn user_defined_map_group_by_charges_the_prepared_live_traversal() {
    let mut runtime = runtime(
        "fn group(values) { \
             return values.group_by(|key, value| \
                 if key <= 8i32 && value >= 8 { \"selected\" } else { \"other\" }).len(); \
         }",
    );
    let values = Ledger(BTreeMap::from([(3, 8), (8, 13), (13, 21)]));
    let baseline = (0..160)
        .find(|limit| {
            runtime
                .call(
                    "group",
                    CallArgs::new().with_host_ref("values", &values),
                    CallOptions::new(*limit, usize::MAX, usize::MAX),
                )
                .is_ok()
        })
        .expect("prepared host Map grouping should fit a bounded call");

    let error = runtime
        .call(
            "group",
            CallArgs::new().with_host_ref("values", &values),
            CallOptions::new(baseline - 1, usize::MAX, usize::MAX),
        )
        .expect_err("one unit below the complete grouping budget must reject the call");
    assert!(matches!(
        error.kind(),
        vela_vm::error::VmErrorKind::BudgetExceeded { .. }
    ));
    assert_eq!(
        values.0,
        BTreeMap::from([(3, 8), (8, 13), (13, 21)]),
        "read-only grouping must not mutate the host map"
    );
}

#[test]
fn user_defined_map_and_set_share_keyed_callbacks_and_bulk_mutations() {
    let mut runtime = runtime(
        "fn update_map(values, extra) { \
             values.extend(extra); \
             values.retain(|key, value| key >= 3i32 && value >= 5); \
             let selected = values.filter(|key, value| key <= 8i32 && value >= 8) \
                 .values().collect_array().sum(); \
             let grouped = values.group_by(|key, value| \
                 if key <= 8i32 && value >= 8 { \"selected\" } else { \"other\" }); \
             return selected + grouped[\"selected\"].values().collect_array().sum(); \
         } \
         fn update_set(values, extra) { \
             values.extend(extra); \
             values.retain(|value| value >= 3i32); \
             return values.filter(|value| value <= 8i32).len(); \
         }",
    );

    let mut ledger = Ledger(BTreeMap::from([(1, 2), (3, 5)]));
    let mut args = CallArgs::new();
    args.push_host_mut("values", &mut ledger);
    args.push_value(
        "extra",
        OwnedValue::map([(3_i32, 8_i64), (8, 13), (13, 21)]),
    );
    let result = runtime
        .call("update_map", args, CallOptions::unbounded())
        .expect("custom MapLike should share keyed callbacks and bulk mutation");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(42)));
    drop(result);
    assert_eq!(
        ledger.0,
        BTreeMap::from([(3_i32, 8_i64), (8, 13), (13, 21)])
    );

    let mut tags = TagSet(BTreeSet::from([1_i32, 3]));
    let mut args = CallArgs::new();
    args.push_host_mut("values", &mut tags);
    args.push_value("extra", OwnedValue::set([3_i32, 5, 8, 13]));
    let result = runtime
        .call("update_set", args, CallOptions::unbounded())
        .expect("custom SetLike should share callbacks and bulk mutation");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(3)));
    drop(result);
    assert_eq!(tags.0, BTreeSet::from([3_i32, 5, 8, 13]));
}

#[test]
fn prepared_host_traversals_resolve_targets_independently_of_element_count() {
    fn resolution_count(values: Vec<i64>) -> usize {
        let mut runtime = runtime(
            "fn traverse(values) { \
                 let selected = values.filter(|value| value % 2 == 0); \
                 let grouped = values.group_by(|value| \
                     if value % 2 == 0 { \"even\" } else { \"odd\" }); \
                 let folded = values.iter().fold(0, |total, value| total + value); \
                 let collected = values.iter().collect_array(); \
                 return selected.len() + grouped.len() + folded + collected.len(); \
             }",
        );
        let values = Timeline(values);
        HOST_TARGET_RESOLUTIONS.with(|count| count.set(0));
        runtime
            .call(
                "traverse",
                CallArgs::new().with_host_ref("values", &values),
                CallOptions::unbounded(),
            )
            .expect("prepared host traversals should run");
        HOST_TARGET_RESOLUTIONS.with(Cell::get)
    }

    let short = resolution_count(vec![1, 2, 3]);
    let long = resolution_count((0..96).collect());

    assert!(
        short > 0,
        "the adapter should observe cold target resolution"
    );
    assert_eq!(
        long, short,
        "filter, group_by, fold, and collect must reuse prepared targets instead of resolving per element"
    );
}
