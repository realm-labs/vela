use vela_bytecode::{CacheSiteId, CacheSiteKind, DebugNameId, FieldSlot, MethodDispatchHandle};
use vela_common::{GlobalSlot, HostMethodId, HostTypeId, ShapeId, SourceId};
use vela_def::TypeId;
use vela_host::resolved::{HostAccessOp, HostSchemaEpoch, ResolvedHostAccess};
use vela_vm::{
    HostInlineCacheEntry, HostInlineCacheTarget, MethodInlineCacheEntry, MethodInlineCacheTarget,
    RecordFieldInlineCacheEntry, owned_value::OwnedValue,
};

use crate::engine::Engine;
use crate::runtime::{CallArgs, CallOptions, Runtime, RuntimeImage};

use super::InlineCaches;

#[test]
fn inline_caches_allocate_from_image_cache_site_count() {
    let engine = Engine::builder().build().expect("engine should build");
    let cached_program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
global value: i64;

fn main() {
    return value;
}
"#,
        )
        .expect("program should compile");
    let cached_image = RuntimeImage::new(engine.clone(), cached_program);
    let mut caches = InlineCaches::for_image(&cached_image);

    assert!(cached_image.cache_site_count() > 0);
    assert!(!caches.is_empty());
    assert_eq!(caches.len(), cached_image.cache_site_count());

    let empty_program = engine
        .compile_source_with_id(SourceId::new(2), "fn main() { return 1; }")
        .expect("program should compile");
    let empty_image = RuntimeImage::new(engine, empty_program);
    caches.clear_for_image(&empty_image);

    assert_eq!(empty_image.cache_site_count(), 0);
    assert!(caches.is_empty());
    assert_eq!(caches.len(), 0);
}

#[test]
fn global_read_inline_cache_is_runtime_local_and_site_indexed() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
global first: i64;
global second: i64;

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
        .global_slot("main::first")
        .expect("first global should have slot");
    let second_slot = program
        .global_slot("main::second")
        .expect("second global should have slot");

    let mut runtime = Runtime::new(engine, program);
    let first_site = runtime
        .image
        .program_image()
        .function_by_name("read_first")
        .expect("read_first should exist")
        .cache_sites
        .sites()
        .iter()
        .find(|site| site.kind == CacheSiteKind::GlobalRead)
        .expect("read_first should have global read site")
        .id;
    let second_site = runtime
        .image
        .program_image()
        .function_by_name("read_second")
        .expect("read_second should exist")
        .cache_sites
        .sites()
        .iter()
        .find(|site| site.kind == CacheSiteKind::GlobalRead)
        .expect("read_second should have global read site")
        .id;
    assert_ne!(first_site, second_site);
    runtime
        .insert_global(
            "main::first",
            OwnedValue::Scalar(vela_common::ScalarValue::I64(10)),
        )
        .expect("first global should insert");
    runtime
        .insert_global(
            "main::second",
            OwnedValue::Scalar(vela_common::ScalarValue::I64(20)),
        )
        .expect("second global should insert");

    assert_eq!(
        runtime.state.inline_caches.global_read_slot(first_site),
        None
    );

    let first = runtime
        .call("read_first", CallArgs::new(), CallOptions::unbounded())
        .expect("read_first should run");
    assert_eq!(
        runtime.value_to_owned(&first),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(10)))
    );
    assert_eq!(
        runtime.state.inline_caches.global_read_slot(first_site),
        Some(first_slot)
    );

    let second = runtime
        .call("read_second", CallArgs::new(), CallOptions::unbounded())
        .expect("read_second should run");
    assert_eq!(
        runtime.value_to_owned(&second),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(20)))
    );
    assert_eq!(
        runtime.state.inline_caches.global_read_slot(second_site),
        Some(second_slot)
    );
    assert_eq!(
        runtime.state.inline_caches.global_read_slot(first_site),
        Some(first_slot)
    );

    runtime
        .insert_global(
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
global value: i64;

fn main() {
    return value;
}
"#,
        )
        .expect("program should compile");
    let image = RuntimeImage::new(engine, program);
    let caches = InlineCaches::for_image(&image);
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
fn inline_cache_families_do_not_evict_same_site_entries() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
global value: i64;

fn main() {
    return value;
}
"#,
        )
        .expect("program should compile");
    let image = RuntimeImage::new(engine, program);
    let caches = InlineCaches::for_image(&image);
    let site = CacheSiteId::new(0);
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

    caches.set_method_dispatch(site, method_entry);
    caches.set_host_access(site, host_entry);

    assert_eq!(caches.method_dispatch(site), Some(method_entry));
    assert_eq!(caches.host_access(site), Some(host_entry));
}

#[test]
fn accepted_hot_reload_clears_runtime_inline_caches() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(1),
            r#"
global first: i64;
global second: i64;

fn read_value() {
    return first;
}
"#,
        )
        .expect("initial hot reload source should compile");
    let first_slot = initial
        .global_names()
        .iter()
        .position(|name| name == "main::first")
        .map(GlobalSlot)
        .expect("first global should have a slot");
    let second_slot = initial
        .global_names()
        .iter()
        .position(|name| name == "main::second")
        .map(GlobalSlot)
        .expect("second global should have a slot");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let initial_site = runtime
        .image
        .program_image()
        .function_by_name("read_value")
        .expect("read_value should exist")
        .cache_sites
        .sites()
        .iter()
        .find(|site| site.kind == CacheSiteKind::GlobalRead)
        .expect("read_value should have an initial global read site")
        .id;
    runtime
        .insert_global(
            "main::first",
            OwnedValue::Scalar(vela_common::ScalarValue::I64(10)),
        )
        .expect("first global should insert");
    runtime
        .insert_global(
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
    assert_eq!(
        runtime.state.inline_caches.global_read_slot(initial_site),
        Some(first_slot)
    );

    let update = runtime
        .compile_hot_reload_update_with_id(
            SourceId::new(2),
            r#"
global first: i64;
global second: i64;

fn read_value() {
    return second;
}
"#,
        )
        .expect("runtime should compile hot reload update")
        .expect("global read target change should be accepted");
    let report = runtime
        .apply_hot_update(update)
        .expect("hot reload update should apply");

    assert!(report.accepted);
    let reloaded_site = runtime
        .image
        .program_image()
        .function_by_name("read_value")
        .expect("reloaded read_value should exist")
        .cache_sites
        .sites()
        .iter()
        .find(|site| site.kind == CacheSiteKind::GlobalRead)
        .expect("reloaded read_value should have a global read site")
        .id;
    assert_eq!(
        runtime.state.inline_caches.global_read_slot(reloaded_site),
        None
    );

    let second = runtime
        .call("read_value", CallArgs::new(), CallOptions::unbounded())
        .expect("reloaded read_value should run");
    assert_eq!(
        runtime.value_to_owned(&second),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(20)))
    );
    assert_eq!(
        runtime.state.inline_caches.global_read_slot(reloaded_site),
        Some(second_slot)
    );
}
