#![allow(clippy::result_large_err)]

use std::collections::{BTreeMap, BTreeSet};

use vela_common::{HostMethodId, HostObjectId, stable_id};
use vela_engine::engine::Engine;
use vela_engine::method::NativeMethodDesc;
use vela_engine::native::{EffectSet, FunctionAccess, TypeHint};
use vela_engine::permission::Capability;
use vela_host::access::HostAccess;
use vela_host::mock::MockStateAdapter;
use vela_host::path::HostPath;
use vela_host::path::HostRef;
use vela_host::protocol::{
    HostCollectionMutation, HostCollectionProjection, HostCollectionQuery, HostCollectionSnapshot,
};
use vela_host::proxy::PathProxy;
use vela_host::resolved::{
    HostAccessOp, HostAccessSpec, HostMutationOp, PreparedHostStep, ResolvedHostAccessKind,
};
use vela_host::target::{HostTargetInstance, HostTargetPlan};
use vela_host::value::HostValue;
use vela_macros::{ScriptHost, script_methods};
use vela_reflect::registry::{FieldDesc, TypeDesc, TypeKey, TypeKind};
use vela_vm::HostExecution;
use vela_vm::error::VmResult;

macro_rules! compile_source {
    ($engine:expr, $source:expr, $expect:literal) => {
        $engine.compile_source($source).expect($expect)
    };
}

#[path = "script_methods/metadata.rs"]
mod metadata;
#[path = "script_methods/registration.rs"]
mod registration;

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(path = "game::player::Player")]
struct Player {
    #[script(get, set)]
    level: u32,
}

#[allow(dead_code)]
#[script_methods]
impl Player {
    /// Grants copied experience through the host patch path.
    #[script_method(effect = "write_host", reflect = true, attr = "domain=player")]
    pub fn grant_exp(
        _ctx: &mut vela_engine::context::NativeCallContext<'_, '_>,
        _player: HostRef,
        _amount: i64,
    ) {
    }

    /// Grants copied score through a callable native method.
    #[script_method(effect = "write_host", reflect = true)]
    pub fn grant_score(
        receiver: &HostPath,
        host: &mut HostExecution<'_>,
        amount: i64,
    ) -> VmResult<i64> {
        host.access.write_diagnostic_path(
            host.adapter,
            receiver.clone().field(Player::vela_field_id_level()),
            HostValue::Scalar(vela_common::ScalarValue::I64(amount)),
            None,
        )?;
        Ok(amount)
    }

    /// Previews an optional copied bonus through a callable native method.
    #[script_method(effect = "read_host", reflect = true)]
    pub fn preview_bonus(
        _receiver: &HostPath,
        _host: &mut HostExecution<'_>,
        bonus: Option<i64>,
    ) -> Option<i64> {
        bonus.map(|bonus| bonus + 1)
    }

    /// Sums five copied method values through a callable native method.
    #[script_method(effect = "write_host", reflect = true)]
    pub fn sum_score(
        receiver: &HostPath,
        host: &mut HostExecution<'_>,
        a: i64,
        b: i64,
        c: i64,
        d: i64,
        e: i64,
    ) -> VmResult<i64> {
        let total = a + b + c + d + e;
        host.access.write_diagnostic_path(
            host.adapter,
            receiver.clone().field(Player::vela_field_id_level()),
            HostValue::Scalar(vela_common::ScalarValue::I64(total)),
            None,
        )?;
        Ok(total)
    }

    /// Sums six copied method values through a callable native method.
    #[allow(clippy::too_many_arguments)]
    #[script_method(effect = "write_host", reflect = true)]
    pub fn sum6_score(
        receiver: &HostPath,
        host: &mut HostExecution<'_>,
        a: i64,
        b: i64,
        c: i64,
        d: i64,
        e: i64,
        f: i64,
    ) -> VmResult<i64> {
        let total = a + b + c + d + e + f;
        host.access.write_diagnostic_path(
            host.adapter,
            receiver.clone().field(Player::vela_field_id_level()),
            HostValue::Scalar(vela_common::ScalarValue::I64(total)),
            None,
        )?;
        Ok(total)
    }

    /// Previews a dynamic copied Result through a callable native method.
    #[script_method(effect = "read_host", reflect = true)]
    pub fn checked_preview(
        _receiver: &HostPath,
        _host: &mut HostExecution<'_>,
        ok: bool,
    ) -> std::result::Result<i64, String> {
        if ok {
            Ok(17)
        } else {
            Err("blocked".to_owned())
        }
    }

    /// Measures an extra copied path proxy argument.
    #[script_method(effect = "read_host", reflect = true)]
    pub fn inspect_path(
        _receiver: &HostPath,
        _host: &mut HostExecution<'_>,
        path: PathProxy,
    ) -> i64 {
        i64::try_from(path.to_diagnostic_path().segments.len()).expect("path depth fits i64")
    }
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(path = "game::counter::DirectCounter")]
struct DirectCounter {
    #[script(get, set)]
    total: i64,
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(path = "game::counter::DirectPeer")]
struct DirectPeer {
    #[script(get, set)]
    total: i64,
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(path = "game::counter::DirectConfig")]
struct DirectConfig {
    #[script(get)]
    bonus: i64,
}

#[script_methods]
impl DirectPeer {}

#[script_methods]
impl DirectConfig {}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(path = "game::counter::DirectWrapper")]
struct DirectWrapper {
    #[script(get)]
    counter: DirectCounter,
}

#[script_methods]
impl DirectWrapper {}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(path = "game::counter::DirectOuter")]
struct DirectOuter {
    #[script(get)]
    wrapper: DirectWrapper,
}

#[script_methods]
impl DirectOuter {}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(path = "game::counter::CollectionLeaf")]
struct CollectionLeaf {
    #[script(get)]
    values: Vec<i64>,
    #[script(get)]
    groups: Vec<Vec<i64>>,
    #[script(get)]
    entries: BTreeMap<String, i64>,
    #[script(get)]
    tags: BTreeSet<String>,
    #[script(get)]
    fixed: [i64; 2],
    #[script(get)]
    counters: Vec<DirectCounter>,
    #[script(get)]
    fixed_counters: [DirectCounter; 1],
}

#[script_methods]
impl CollectionLeaf {}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(path = "game::counter::CollectionOuter")]
struct CollectionOuter {
    #[script(get)]
    leaf: CollectionLeaf,
}

#[script_methods]
impl CollectionOuter {}

#[allow(dead_code)]
#[script_methods]
impl DirectCounter {
    #[script_method(effect = "write_host", reflect = true)]
    pub fn add(&mut self, amount: i64) -> i64 {
        self.total += amount;
        self.total
    }

    #[script_method(effect = "write_host", reflect = true)]
    pub async fn add_async(&mut self, amount: i64) -> i64 {
        let mut pending = true;
        std::future::poll_fn(move |context| {
            if pending {
                pending = false;
                context.waker().wake_by_ref();
                std::task::Poll::Pending
            } else {
                std::task::Poll::Ready(())
            }
        })
        .await;
        self.total += amount;
        self.total
    }

    #[script_method(effect = "read_host", reflect = true)]
    pub async fn read_async(&self) -> i64 {
        self.total
    }

    #[script_method(effect = "read_host")]
    pub async fn read_shared_alias(
        &self,
        context: &mut vela_engine::context::NativeCallContext<'_, '_>,
        other: &DirectCounter,
        raw: HostRef,
    ) -> VmResult<i64> {
        let mut pending = true;
        std::future::poll_fn(move |task| {
            if pending {
                pending = false;
                task.waker().wake_by_ref();
                std::task::Poll::Pending
            } else {
                std::task::Poll::Ready(())
            }
        })
        .await;
        let _ = context
            .call_async(
                "raw_read",
                vela_engine::runtime::CallArgs::new().with_host_handle("counter", raw),
            )
            .await?;
        Ok(self.total + other.total)
    }

    #[script_method(effect = "read_host")]
    pub async fn wait_shared(&self) -> i64 {
        std::future::pending().await
    }

    #[script_method(effect = "write_host")]
    pub async fn add_with_hook(
        &mut self,
        context: &mut vela_engine::context::NativeCallContext<'_, '_>,
        raw: HostRef,
        amount: i64,
    ) -> VmResult<i64> {
        self.total += amount;
        let raw_error = context
            .call_async(
                "raw_read",
                vela_engine::runtime::CallArgs::new().with_host_handle("counter", raw),
            )
            .await
            .expect_err("the raw parent HostRef should remain busy while leased");
        if !matches!(
            raw_error.kind(),
            vela_vm::error::VmErrorKind::Host(
                vela_host::error::HostErrorKind::HostObjectBusy { .. }
            )
        ) {
            return Err(raw_error);
        }
        let _ = context
            .call_async(
                "hook",
                vela_engine::runtime::CallArgs::new().with_host_mut("counter", &mut *self),
            )
            .await?;
        self.total += 1;
        Ok(self.total)
    }

    #[script_method(effect = "write_host")]
    pub async fn update_with(
        &mut self,
        peer: &mut DirectPeer,
        config: &DirectConfig,
        amount: i64,
    ) -> i64 {
        peer.total += amount;
        self.total += peer.total + config.bonus;
        self.total
    }

    #[script_method(effect = "write_host")]
    pub async fn merge(&mut self, other: &mut DirectCounter) -> i64 {
        self.total += other.total;
        self.total
    }

    #[script_method(effect = "write_host")]
    pub async fn wait_async(&mut self) -> i64 {
        std::future::pending().await
    }

    #[script_method(effect = "write_host")]
    pub async fn wait_with_context(
        &mut self,
        _context: &mut vela_engine::context::NativeCallContext<'_, '_>,
    ) -> i64 {
        std::future::pending().await
    }

    #[script_method(effect = "write_host")]
    pub async fn panic_with_context(
        &mut self,
        _context: &mut vela_engine::context::NativeCallContext<'_, '_>,
    ) -> i64 {
        panic!("context direct method panic fixture")
    }
}

fn method_id(name: &str) -> HostMethodId {
    HostMethodId::new(u128::from(stable_id(
        "host_method",
        "game::player::Player",
        name,
    )))
}

#[test]
fn script_methods_resolve_root_methods_to_direct_access() {
    let mut counter = DirectCounter { total: 1 };
    let plan = HostTargetPlan::new(DirectCounter::vela_host_type_id());
    let method = HostMethodId::new(u128::from(stable_id(
        "host_method",
        "game::counter::DirectCounter",
        "add",
    )));

    let access = <DirectCounter as vela_host::object::ScriptHostObject>::resolve_host_target(
        &counter,
        HostAccessSpec::new(HostAccessOp::Call(method), &plan),
    )
    .expect("generated script method resolver should resolve direct self method");

    assert_eq!(access.adapter_kind, ResolvedHostAccessKind::DirectMethod(0));

    let root = HostRef::new(DirectCounter::vela_host_type_id(), HostObjectId::new(7), 1);
    let result = <DirectCounter as vela_host::object::ScriptHostObject>::call_resolved_host(
        &mut counter,
        access,
        HostTargetInstance::new(root, &plan, &[]),
        method,
        &[HostValue::Scalar(vela_common::ScalarValue::I64(4))],
    )
    .expect("generated dense method adapter should execute");
    assert_eq!(result, HostValue::Scalar(vela_common::ScalarValue::I64(5)));
    assert_eq!(counter.total, 5);
}

#[test]
fn nested_script_method_resolution_prepares_inline_field_slots() {
    let mut outer = DirectOuter {
        wrapper: DirectWrapper {
            counter: DirectCounter { total: 2 },
        },
    };
    let plan = HostTargetPlan::new(DirectOuter::vela_host_type_id())
        .field(DirectOuter::vela_field_id_wrapper())
        .field(DirectWrapper::vela_field_id_counter());
    let method = HostMethodId::new(u128::from(stable_id(
        "host_method",
        "game::counter::DirectCounter",
        "add",
    )));
    let access = <DirectOuter as vela_host::object::ScriptHostObject>::resolve_host_target(
        &outer,
        HostAccessSpec::new(HostAccessOp::Call(method), &plan),
    )
    .expect("nested method should resolve through the target-plan cursor");

    assert_eq!(access.adapter_kind, ResolvedHostAccessKind::DirectMethod(0));
    assert_eq!(access.prepared_field_slot(0), Some(0));
    assert_eq!(access.prepared_field_slot(1), Some(0));
    assert_eq!(access.prepared_field_slot(2), None);
    let root = HostRef::new(DirectOuter::vela_host_type_id(), HostObjectId::new(8), 1);
    let result = <DirectOuter as vela_host::object::ScriptHostObject>::call_resolved_host(
        &mut outer,
        access,
        HostTargetInstance::new(root, &plan, &[]),
        method,
        &[HostValue::Scalar(vela_common::ScalarValue::I64(3))],
    )
    .expect("nested direct method should execute without rewriting its plan");

    assert_eq!(result, HostValue::Scalar(vela_common::ScalarValue::I64(5)));
    assert_eq!(outer.wrapper.counter.total, 5);
}

#[test]
fn nested_field_access_executes_prepared_inline_field_slots() {
    let mut outer = DirectOuter {
        wrapper: DirectWrapper {
            counter: DirectCounter { total: 2 },
        },
    };
    let plan = HostTargetPlan::new(DirectOuter::vela_host_type_id())
        .field(DirectOuter::vela_field_id_wrapper())
        .field(DirectWrapper::vela_field_id_counter())
        .field(DirectCounter::vela_field_id_total());
    let root = HostRef::new(DirectOuter::vela_host_type_id(), HostObjectId::new(9), 1);
    let target = HostTargetInstance::new(root, &plan, &[]);

    let read_access = <DirectOuter as vela_host::object::ScriptHostObject>::resolve_host_target(
        &outer,
        HostAccessSpec::new(HostAccessOp::Read, &plan),
    )
    .expect("nested field read should resolve");
    assert_eq!(read_access.prepared_field_slot(0), Some(0));
    assert_eq!(read_access.prepared_field_slot(1), Some(0));
    assert_eq!(read_access.prepared_field_slot(2), None);
    let value = <DirectOuter as vela_host::object::ScriptHostObject>::read_resolved_host(
        &outer,
        read_access,
        target,
    )
    .expect("prepared nested field read should execute");
    assert_eq!(value, HostValue::Scalar(vela_common::ScalarValue::I64(2)));

    let write_access = <DirectOuter as vela_host::object::ScriptHostObject>::resolve_host_target(
        &outer,
        HostAccessSpec::new(HostAccessOp::Write, &plan),
    )
    .expect("nested field write should resolve");
    <DirectOuter as vela_host::object::ScriptHostObject>::write_resolved_host(
        &mut outer,
        write_access,
        target,
        HostValue::Scalar(vela_common::ScalarValue::I64(7)),
    )
    .expect("prepared nested field write should execute");

    let mutate_access = <DirectOuter as vela_host::object::ScriptHostObject>::resolve_host_target(
        &outer,
        HostAccessSpec::new(HostAccessOp::Mutate(HostMutationOp::Add), &plan),
    )
    .expect("nested field mutation should resolve");
    <DirectOuter as vela_host::object::ScriptHostObject>::mutate_resolved_host(
        &mut outer,
        mutate_access,
        target,
        HostMutationOp::Add,
        HostValue::Scalar(vela_common::ScalarValue::I64(2)),
    )
    .expect("prepared nested field mutation should execute");

    assert_eq!(outer.wrapper.counter.total, 9);
}

#[test]
fn nested_collection_protocols_execute_prepared_field_slots() {
    let mut outer = CollectionOuter {
        leaf: CollectionLeaf {
            values: vec![1, 2],
            groups: vec![vec![4, 5]],
            entries: BTreeMap::from([("x".to_owned(), 8)]),
            tags: BTreeSet::from(["ready".to_owned()]),
            fixed: [13, 21],
            counters: vec![DirectCounter { total: 34 }],
            fixed_counters: [DirectCounter { total: 55 }],
        },
    };
    let plan = HostTargetPlan::new(CollectionOuter::vela_host_type_id())
        .field(CollectionOuter::vela_field_id_leaf())
        .field(CollectionLeaf::vela_field_id_values());
    let root = HostRef::new(
        CollectionOuter::vela_host_type_id(),
        HostObjectId::new(10),
        1,
    );
    let target = HostTargetInstance::new(root, &plan, &[]);
    let read_access =
        <CollectionOuter as vela_host::object::ScriptHostObject>::resolve_host_target(
            &outer,
            HostAccessSpec::new(HostAccessOp::Read, &plan),
        )
        .expect("nested collection should resolve through prepared fields");

    let len =
        <CollectionOuter as vela_host::object::ScriptHostObject>::query_collection_resolved_host(
            &outer,
            read_access,
            target,
            HostCollectionQuery::Len,
        )
        .expect("nested collection length should execute");
    assert_eq!(len, HostValue::Scalar(vela_common::ScalarValue::I64(2)));

    let snapshot =
        <CollectionOuter as vela_host::object::ScriptHostObject>::snapshot_collection_resolved_host(
            &outer,
            read_access,
            target,
            HostCollectionProjection::Values,
        )
        .expect("nested collection snapshot should execute");
    assert_eq!(
        snapshot,
        HostCollectionSnapshot::Items(vec![
            HostValue::Scalar(vela_common::ScalarValue::I64(1)),
            HostValue::Scalar(vela_common::ScalarValue::I64(2)),
        ])
    );

    let mutation_access =
        <CollectionOuter as vela_host::object::ScriptHostObject>::resolve_host_target(
            &outer,
            HostAccessSpec::new(HostAccessOp::Mutate(HostMutationOp::Push), &plan),
        )
        .expect("nested collection mutation should resolve");
    let extension = [HostValue::Scalar(vela_common::ScalarValue::I64(3))];
    <CollectionOuter as vela_host::object::ScriptHostObject>::mutate_collection_resolved_host(
        &mut outer,
        mutation_access,
        target,
        HostCollectionMutation::ExtendSequence(&extension),
    )
    .expect("nested collection mutation should execute");

    assert_eq!(outer.leaf.values, vec![1, 2, 3]);

    let indexed_plan = HostTargetPlan::new(CollectionOuter::vela_host_type_id())
        .field(CollectionOuter::vela_field_id_leaf())
        .field(CollectionLeaf::vela_field_id_groups())
        .const_index(0);
    let indexed_target = HostTargetInstance::new(root, &indexed_plan, &[]);
    let indexed_access =
        <CollectionOuter as vela_host::object::ScriptHostObject>::resolve_host_target(
            &outer,
            HostAccessSpec::new(HostAccessOp::Read, &indexed_plan),
        )
        .expect("indexed nested sequence should resolve through an adapter-local slot");
    assert_eq!(
        indexed_access.adapter_kind,
        ResolvedHostAccessKind::AdapterLocal(0)
    );
    assert_eq!(indexed_access.prepared_field_slot(0), Some(0));
    assert_eq!(indexed_access.prepared_field_slot(1), Some(1));
    let indexed_len =
        <CollectionOuter as vela_host::object::ScriptHostObject>::query_collection_resolved_host(
            &outer,
            indexed_access,
            indexed_target,
            HostCollectionQuery::Len,
        )
        .expect("indexed nested collection should use validated fallback");
    assert_eq!(
        indexed_len,
        HostValue::Scalar(vela_common::ScalarValue::I64(2))
    );

    let remove_plan = HostTargetPlan::new(CollectionOuter::vela_host_type_id())
        .field(CollectionOuter::vela_field_id_leaf())
        .field(CollectionLeaf::vela_field_id_entries())
        .const_key("x");
    let remove_target = HostTargetInstance::new(root, &remove_plan, &[]);
    let keyed_access =
        <CollectionOuter as vela_host::object::ScriptHostObject>::resolve_host_target(
            &outer,
            HostAccessSpec::new(HostAccessOp::Read, &remove_plan),
        )
        .expect("keyed nested map should resolve through an adapter-local slot");
    assert_eq!(
        keyed_access.adapter_kind,
        ResolvedHostAccessKind::AdapterLocal(0)
    );
    assert_eq!(keyed_access.prepared_field_slot(0), Some(0));
    assert_eq!(keyed_access.prepared_field_slot(1), Some(2));
    let keyed_value = <CollectionOuter as vela_host::object::ScriptHostObject>::read_resolved_host(
        &outer,
        keyed_access,
        remove_target,
    )
    .expect("prepared keyed read should reach the map adapter");
    assert_eq!(
        keyed_value,
        HostValue::Scalar(vela_common::ScalarValue::I64(8))
    );

    let set_plan = HostTargetPlan::new(CollectionOuter::vela_host_type_id())
        .field(CollectionOuter::vela_field_id_leaf())
        .field(CollectionLeaf::vela_field_id_tags())
        .const_key("ready");
    let set_target = HostTargetInstance::new(root, &set_plan, &[]);
    let set_access = <CollectionOuter as vela_host::object::ScriptHostObject>::resolve_host_target(
        &outer,
        HostAccessSpec::new(HostAccessOp::Read, &set_plan),
    )
    .expect("keyed nested set should resolve through an adapter-local slot");
    assert_eq!(
        set_access.adapter_kind,
        ResolvedHostAccessKind::AdapterLocal(0)
    );
    let membership = <CollectionOuter as vela_host::object::ScriptHostObject>::read_resolved_host(
        &outer, set_access, set_target,
    )
    .expect("prepared keyed read should reach the set adapter");
    assert_eq!(membership, HostValue::Bool(true));

    let fixed_plan = HostTargetPlan::new(CollectionOuter::vela_host_type_id())
        .field(CollectionOuter::vela_field_id_leaf())
        .field(CollectionLeaf::vela_field_id_fixed())
        .const_index(1);
    let fixed_target = HostTargetInstance::new(root, &fixed_plan, &[]);
    let fixed_access =
        <CollectionOuter as vela_host::object::ScriptHostObject>::resolve_host_target(
            &outer,
            HostAccessSpec::new(HostAccessOp::Read, &fixed_plan),
        )
        .expect("indexed nested fixed array should resolve through an adapter-local slot");
    assert_eq!(
        fixed_access.adapter_kind,
        ResolvedHostAccessKind::AdapterLocal(0)
    );
    assert_eq!(fixed_access.prepared_field_slot(0), Some(0));
    assert_eq!(fixed_access.prepared_field_slot(1), Some(4));
    let fixed_value = <CollectionOuter as vela_host::object::ScriptHostObject>::read_resolved_host(
        &outer,
        fixed_access,
        fixed_target,
    )
    .expect("prepared indexed read should reach the fixed-array adapter");
    assert_eq!(
        fixed_value,
        HostValue::Scalar(vela_common::ScalarValue::I64(21))
    );

    let element_field_plan = HostTargetPlan::new(CollectionOuter::vela_host_type_id())
        .field(CollectionOuter::vela_field_id_leaf())
        .field(CollectionLeaf::vela_field_id_counters())
        .const_index(0)
        .field(DirectCounter::vela_field_id_total());
    let element_field_target = HostTargetInstance::new(root, &element_field_plan, &[]);
    let element_field_access =
        <CollectionOuter as vela_host::object::ScriptHostObject>::resolve_host_target(
            &outer,
            HostAccessSpec::new(HostAccessOp::Read, &element_field_plan),
        )
        .expect("indexed element field should resolve through a mixed prepared chain");
    assert_eq!(
        element_field_access.adapter_kind,
        ResolvedHostAccessKind::DirectField(0)
    );
    assert_eq!(element_field_access.prepared_field_slot(0), Some(0));
    assert_eq!(element_field_access.prepared_field_slot(1), Some(5));
    assert_eq!(
        element_field_access.prepared_step(2),
        Some(PreparedHostStep::AdapterLocal(0))
    );
    let element_field_value =
        <CollectionOuter as vela_host::object::ScriptHostObject>::read_resolved_host(
            &outer,
            element_field_access,
            element_field_target,
        )
        .expect("mixed prepared chain should execute the dense element field read");
    assert_eq!(
        element_field_value,
        HostValue::Scalar(vela_common::ScalarValue::I64(34))
    );
    let element_write_access =
        <CollectionOuter as vela_host::object::ScriptHostObject>::resolve_host_target(
            &outer,
            HostAccessSpec::new(HostAccessOp::Write, &element_field_plan),
        )
        .expect("indexed element field write should reuse the mixed prepared chain");
    <CollectionOuter as vela_host::object::ScriptHostObject>::write_resolved_host(
        &mut outer,
        element_write_access,
        element_field_target,
        HostValue::Scalar(vela_common::ScalarValue::I64(40)),
    )
    .expect("mixed prepared chain should execute the dense element field write");
    let element_mutate_access =
        <CollectionOuter as vela_host::object::ScriptHostObject>::resolve_host_target(
            &outer,
            HostAccessSpec::new(
                HostAccessOp::Mutate(HostMutationOp::Add),
                &element_field_plan,
            ),
        )
        .expect("indexed element field mutation should reuse the mixed prepared chain");
    <CollectionOuter as vela_host::object::ScriptHostObject>::mutate_resolved_host(
        &mut outer,
        element_mutate_access,
        element_field_target,
        HostMutationOp::Add,
        HostValue::Scalar(vela_common::ScalarValue::I64(2)),
    )
    .expect("mixed prepared chain should execute the dense element field mutation");
    assert_eq!(outer.leaf.counters[0].total, 42);

    let fixed_element_plan = HostTargetPlan::new(CollectionOuter::vela_host_type_id())
        .field(CollectionOuter::vela_field_id_leaf())
        .field(CollectionLeaf::vela_field_id_fixed_counters())
        .const_index(0)
        .field(DirectCounter::vela_field_id_total());
    let fixed_element_target = HostTargetInstance::new(root, &fixed_element_plan, &[]);
    let fixed_element_access =
        <CollectionOuter as vela_host::object::ScriptHostObject>::resolve_host_target(
            &outer,
            HostAccessSpec::new(
                HostAccessOp::Mutate(HostMutationOp::Add),
                &fixed_element_plan,
            ),
        )
        .expect("fixed-array element field should resolve through a mixed prepared chain");
    assert_eq!(
        fixed_element_access.prepared_step(2),
        Some(PreparedHostStep::AdapterLocal(0))
    );
    <CollectionOuter as vela_host::object::ScriptHostObject>::mutate_resolved_host(
        &mut outer,
        fixed_element_access,
        fixed_element_target,
        HostMutationOp::Add,
        HostValue::Scalar(vela_common::ScalarValue::I64(3)),
    )
    .expect("fixed-array mixed chain should execute the dense element mutation");
    assert_eq!(outer.leaf.fixed_counters[0].total, 58);

    let remove_access =
        <CollectionOuter as vela_host::object::ScriptHostObject>::resolve_host_target(
            &outer,
            HostAccessSpec::new(HostAccessOp::Remove, &remove_plan),
        )
        .expect("indexed nested removal should resolve");
    <CollectionOuter as vela_host::object::ScriptHostObject>::remove_resolved_host(
        &mut outer,
        remove_access,
        remove_target,
    )
    .expect("indexed nested removal should reach the collection adapter");
    assert!(outer.leaf.entries.is_empty());
}

#[test]
fn borrowed_slice_element_fields_execute_mixed_prepared_steps() {
    let mut values = [DirectCounter { total: 8 }];
    let slice: &mut [DirectCounter] = &mut values;
    let plan = HostTargetPlan::new(vela_common::HostTypeId::new(0))
        .const_index(0)
        .field(DirectCounter::vela_field_id_total());
    let root = HostRef::new(vela_common::HostTypeId::new(0), HostObjectId::new(11), 1);
    let target = HostTargetInstance::new(root, &plan, &[]);
    let access = <[DirectCounter] as vela_host::object::ScriptHostObject>::resolve_host_target(
        slice,
        HostAccessSpec::new(HostAccessOp::Mutate(HostMutationOp::Add), &plan),
    )
    .expect("borrowed slice element field should resolve through a mixed prepared chain");

    assert_eq!(
        access.prepared_step(0),
        Some(PreparedHostStep::AdapterLocal(0))
    );
    <[DirectCounter] as vela_host::object::ScriptHostObject>::mutate_resolved_host(
        slice,
        access,
        target,
        HostMutationOp::Add,
        HostValue::Scalar(vela_common::ScalarValue::I64(5)),
    )
    .expect("borrowed slice mixed chain should execute the dense element mutation");
    assert_eq!(values[0].total, 13);
}
