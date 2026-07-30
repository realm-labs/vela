use vela_bytecode::{
    CacheSiteDesc, CacheSiteId, CacheSiteKind, DebugNameId, FieldSlot, InstructionOffset,
    MethodDispatchHandle, linked::InstructionKind,
};
use vela_common::{HostMethodId, HostTypeId, ShapeId, SourceId};
use vela_def::TypeId;
use vela_host::resolved::{HostAccessOp, HostSchemaEpoch, ResolvedHostAccess};
use vela_vm::{
    HostInlineCacheEntry, HostInlineCacheTarget, MethodInlineCacheEntry, MethodInlineCacheTarget,
    RecordFieldInlineCacheEntry, owned_value::OwnedValue,
};

use crate::engine::Engine;
use crate::runtime::{CallArgs, CallOptions, Runtime, RuntimeImage};

use super::GenerationInlineCaches;

#[test]
fn generation_caches_allocate_one_typed_slot_per_linked_cache_site() {
    let engine = Engine::builder().build().expect("engine should build");
    let cached_program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
state value: i64 = 0;

fn main() {
    return value;
}
"#,
        )
        .expect("program should compile");
    let cached_image = RuntimeImage::new_compiled(engine.clone(), cached_program);
    let caches = cached_image.execution_data().inline_caches();

    assert!(cached_image.cache_site_count() > 0);
    assert!(!caches.is_empty());
    assert_eq!(caches.len(), cached_image.cache_site_count());

    let empty_program = engine
        .compile_source_with_id(SourceId::new(2), "fn main() { return 1; }")
        .expect("program should compile");
    let empty_image = RuntimeImage::new_compiled(engine, empty_program);
    let empty_caches = empty_image.execution_data().inline_caches();

    assert_eq!(empty_image.cache_site_count(), 0);
    assert!(empty_caches.is_empty());
    assert_eq!(empty_caches.len(), 0);
}

#[test]
fn declared_state_reads_use_linked_slots_without_mutable_cache_entries() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
state first: i64 = 0;
state second: i64 = 0;

fn read_first() {
    return first;
}

fn read_second() {
    return second;
}
"#,
        )
        .expect("program should compile");
    let first_slot = program
        .state_slot("main::first")
        .expect("first state should have slot");
    let second_slot = program
        .state_slot("main::second")
        .expect("second state should have slot");

    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    assert_eq!(linked_state_slot(&runtime, "read_first"), first_slot);
    assert_eq!(linked_state_slot(&runtime, "read_second"), second_slot);
    runtime
        .set_state(
            "main::first",
            OwnedValue::Scalar(vela_common::ScalarValue::I64(10)),
        )
        .expect("first global should insert");
    runtime
        .set_state(
            "main::second",
            OwnedValue::Scalar(vela_common::ScalarValue::I64(20)),
        )
        .expect("second global should insert");

    let first = runtime
        .call("read_first", CallArgs::new(), CallOptions::unbounded())
        .expect("read_first should run");
    assert_eq!(
        runtime.value_to_owned(&first),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(10)))
    );
    let second = runtime
        .call("read_second", CallArgs::new(), CallOptions::unbounded())
        .expect("read_second should run");
    assert_eq!(
        runtime.value_to_owned(&second),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(20)))
    );
    runtime
        .set_state(
            "main::first",
            OwnedValue::Scalar(vela_common::ScalarValue::I64(30)),
        )
        .expect("first global should update");
    let first_after_update = runtime
        .call("read_first", CallArgs::new(), CallOptions::unbounded())
        .expect("read_first should run after update");
    assert_eq!(
        runtime.value_to_owned(&first_after_update),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(30)))
    );
}

#[test]
fn record_field_inline_cache_is_site_indexed() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
state value: i64 = 0;

fn main() {
    return value;
}
"#,
        )
        .expect("program should compile");
    let _image = RuntimeImage::new_compiled(engine, program);
    let caches = GenerationInlineCaches::for_layout(&[CacheSiteDesc::new(
        CacheSiteId::new(0),
        CacheSiteKind::RecordFieldRead,
        "main",
        InstructionOffset(0),
    )]);
    let site = CacheSiteId::new(0);
    let entry = RecordFieldInlineCacheEntry {
        type_id: TypeId::new(1),
        shape_id: ShapeId::new(2),
        field: FieldSlot::new(3),
    };

    assert_eq!(caches.record_field(site), None);
    caches.set_record_field(site, entry);
    assert_eq!(caches.record_field(site), Some(entry));
}

#[test]
fn generation_cache_slots_reject_entries_from_the_wrong_family() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
state value: i64 = 0;

fn main() {
    return value;
}
"#,
        )
        .expect("program should compile");
    let _image = RuntimeImage::new_compiled(engine, program);
    let caches = GenerationInlineCaches::for_layout(&[
        CacheSiteDesc::new(
            CacheSiteId::new(0),
            CacheSiteKind::MethodCall,
            "main",
            InstructionOffset(0),
        ),
        CacheSiteDesc::new(
            CacheSiteId::new(1),
            CacheSiteKind::HostPathCall,
            "main",
            InstructionOffset(1),
        ),
    ]);
    let method_site = CacheSiteId::new(0);
    let host_site = CacheSiteId::new(1);
    let method_id = HostMethodId::new(7);
    let method_entry = MethodInlineCacheEntry {
        dispatch: MethodDispatchHandle::new(0),
        debug_name: DebugNameId::new(0),
        target: MethodInlineCacheTarget::Host { method_id },
    };
    let host_entry = HostInlineCacheEntry {
        root_type: HostTypeId::new(1),
        target: HostInlineCacheTarget::RootObject,
        op: HostAccessOp::Call(method_id),
        schema_epoch: HostSchemaEpoch::new(0),
        resolved: ResolvedHostAccess::generic_target(HostSchemaEpoch::new(0)),
    };

    caches.set_method_dispatch(method_site, method_entry);
    caches.set_host_access(method_site, host_entry);
    caches.set_host_access(host_site, host_entry);

    assert_eq!(caches.method_dispatch(method_site), Some(method_entry));
    assert_eq!(caches.host_access(method_site), None);
    assert_eq!(caches.host_access(host_site), Some(host_entry));
}

#[test]
fn accepted_hot_reload_uses_the_new_generation_linked_state_slot() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(1),
            r#"
state first: i64 = 0;
state second: i64 = 0;

fn read_value() {
    return first;
}
"#,
        )
        .expect("initial hot reload source should compile");
    let second_slot = initial
        .states()
        .iter()
        .position(|state| state.qualified_name == "main::second")
        .map(vela_common::StateSlot)
        .expect("second global should have a slot");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    runtime
        .set_state(
            "main::first",
            OwnedValue::Scalar(vela_common::ScalarValue::I64(10)),
        )
        .expect("first global should insert");
    runtime
        .set_state(
            "main::second",
            OwnedValue::Scalar(vela_common::ScalarValue::I64(20)),
        )
        .expect("second global should insert");

    let first = runtime
        .call("read_value", CallArgs::new(), CallOptions::unbounded())
        .expect("initial read_value should run");
    assert_eq!(
        runtime.value_to_owned(&first),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(10)))
    );
    let update = runtime
        .compile_reload_with_id(
            SourceId::new(2),
            r#"
state first: i64 = 0;
state second: i64 = 0;

fn read_value() {
    return second;
}
"#,
        )
        .expect("runtime should compile hot reload update")
        .expect("global read target change should be accepted");
    let report = runtime
        .apply_reload_update_for_test(update)
        .expect("hot reload update should apply");

    assert!(report.accepted);
    assert_eq!(linked_state_slot(&runtime, "read_value"), second_slot);

    let second = runtime
        .call("read_value", CallArgs::new(), CallOptions::unbounded())
        .expect("reloaded read_value should run");
    assert_eq!(
        runtime.value_to_owned(&second),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(20)))
    );
}

fn linked_state_slot(runtime: &Runtime, function: &str) -> vela_common::StateSlot {
    let program = runtime.image.linked_program();
    runtime
        .image
        .linked_program()
        .functions()
        .find(|(_, code)| program.debug_name(code.debug_name) == function)
        .map(|(_, code)| code)
        .expect("linked function should exist")
        .instructions
        .iter()
        .find_map(|instruction| match instruction.kind {
            InstructionKind::LoadState { slot, .. } => Some(slot),
            _ => None,
        })
        .expect("function should contain a linked state load")
}
