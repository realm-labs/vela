use std::cell::RefCell;

use vela_bytecode::CacheSiteId;
use vela_common::GlobalSlot;
use vela_vm::HostInlineCacheEntry;

use super::image::RuntimeImage;

#[derive(Debug, Default)]
pub(super) struct InlineCaches {
    entries: RefCell<Vec<InlineCacheEntry>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum InlineCacheEntry {
    Empty,
    GlobalRead { slot: GlobalSlot },
    HostAccess(HostInlineCacheEntry),
}

impl InlineCaches {
    pub(super) fn for_image(image: &RuntimeImage) -> Self {
        Self {
            entries: RefCell::new(vec![InlineCacheEntry::Empty; image.cache_site_count()]),
        }
    }

    pub(super) fn clear_for_image(&mut self, image: &RuntimeImage) {
        *self = Self::for_image(image);
    }

    pub(super) fn len(&self) -> usize {
        self.entries.borrow().len()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.entries.borrow().is_empty()
    }

    pub(super) fn global_read_slot(&self, site: CacheSiteId) -> Option<GlobalSlot> {
        match self.entries.borrow().get(site.index()) {
            Some(InlineCacheEntry::GlobalRead { slot }) => Some(*slot),
            _ => None,
        }
    }

    pub(super) fn set_global_read_slot(&self, site: CacheSiteId, slot: GlobalSlot) {
        if let Some(entry) = self.entries.borrow_mut().get_mut(site.index()) {
            *entry = InlineCacheEntry::GlobalRead { slot };
        }
    }

    pub(super) fn host_access(&self, site: CacheSiteId) -> Option<HostInlineCacheEntry> {
        match self.entries.borrow().get(site.index()) {
            Some(InlineCacheEntry::HostAccess(entry)) => Some(*entry),
            _ => None,
        }
    }

    pub(super) fn set_host_access(&self, site: CacheSiteId, entry: HostInlineCacheEntry) {
        if let Some(slot) = self.entries.borrow_mut().get_mut(site.index()) {
            *slot = InlineCacheEntry::HostAccess(entry);
        }
    }
}

impl vela_vm::VmInlineCaches for InlineCaches {
    fn len(&self) -> usize {
        self.len()
    }

    fn is_empty(&self) -> bool {
        self.is_empty()
    }

    fn global_read_slot(&self, site: CacheSiteId) -> Option<GlobalSlot> {
        self.global_read_slot(site)
    }

    fn set_global_read_slot(&self, site: CacheSiteId, slot: GlobalSlot) {
        self.set_global_read_slot(site, slot);
    }

    fn host_access(&self, site: CacheSiteId) -> Option<HostInlineCacheEntry> {
        self.host_access(site)
    }

    fn set_host_access(&self, site: CacheSiteId, entry: HostInlineCacheEntry) {
        self.set_host_access(site, entry);
    }
}

#[cfg(test)]
mod tests {
    use vela_bytecode::CacheSiteKind;
    use vela_common::{HostObjectId, HostTypeId, SourceId};
    use vela_def::{FieldId, TypeId};
    use vela_host::access::HostAccess;
    use vela_host::mock::MockStateAdapter;
    use vela_host::path::{HostPath, HostRef};
    use vela_host::resolved::{HostAccessOp, ResolvedHostAccessKind};
    use vela_host::value::HostValue;
    use vela_reflect::registry::{FieldDesc, TypeDesc, TypeKey};
    use vela_vm::owned_value::OwnedValue;

    use crate::engine::Engine;
    use crate::runtime::{CallArgs, CallOptions, Runtime, RuntimeImage};

    use super::InlineCaches;

    #[test]
    fn inline_caches_allocate_from_image_cache_site_count() {
        let engine = Engine::builder().build().expect("engine should build");
        let cached_program = engine
            .compile_source(
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
            .compile_source(SourceId::new(2), "fn main() { return 1; }")
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
            .compile_source(
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
    fn accepted_hot_reload_clears_runtime_inline_caches() {
        let engine = Engine::builder().build().expect("engine should build");
        let initial = engine
            .compile_hot_reload_initial(
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
            .map(vela_common::GlobalSlot)
            .expect("first global should have a slot");
        let second_slot = initial
            .global_names()
            .iter()
            .position(|name| name == "main::second")
            .map(vela_common::GlobalSlot)
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
            .compile_hot_reload_update(
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

    #[test]
    fn host_access_inline_cache_records_resolved_target_guard() {
        let engine = Engine::builder()
            .register_type(
                TypeDesc::new(TypeKey::new(TypeId::new(1), "CachedHostPlayer"))
                    .host_type(HostTypeId::new(1))
                    .field(FieldDesc::new(FieldId::new(1), "level")),
            )
            .build()
            .expect("engine should build");
        let program = engine
            .compile_source(
                SourceId::new(1),
                r#"
fn read_level(player: CachedHostPlayer) {
    return player.level;
}
"#,
            )
            .expect("program should compile");
        let function = program
            .function("read_level")
            .expect("read_level should exist");
        let cache_site = function
            .cache_sites
            .sites()
            .iter()
            .find(|site| site.kind == CacheSiteKind::HostPathRead)
            .expect("read_level should have host read site")
            .id;
        let host_target = function
            .host_targets
            .first()
            .expect("read_level should have host target")
            .clone();
        let mut runtime = Runtime::new(engine, program);
        let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
        let host_path = HostPath::new(host_ref).field(FieldId::new(1));
        let mut adapter = MockStateAdapter::new();
        adapter.insert_diagnostic_path_value(
            host_path,
            HostValue::Scalar(vela_common::ScalarValue::I64(12)),
        );
        let mut access = HostAccess::new();

        assert_eq!(runtime.state.inline_caches.host_access(cache_site), None);

        let value = runtime
            .call_raw(
                "read_level",
                &[OwnedValue::HostRef(host_ref)],
                CallOptions::unbounded(),
                &mut adapter,
                &mut access,
            )
            .expect("read_level should run");

        assert_eq!(value, OwnedValue::Scalar(vela_common::ScalarValue::I64(12)));
        let entry = runtime
            .state
            .inline_caches
            .host_access(cache_site)
            .expect("host read should populate cache");
        assert_eq!(entry.root_type, HostTypeId::new(1));
        assert_eq!(entry.plan_id.index(), 0);
        assert_eq!(entry.op, HostAccessOp::Read);
        assert_eq!(entry.schema_epoch.get(), 0);
        assert_eq!(
            entry.resolved.adapter_kind,
            ResolvedHostAccessKind::GenericTarget
        );
        assert_eq!(entry.resolved.schema_epoch.get(), 0);
        assert_eq!(host_target.root_type, HostTypeId::new(1));
    }
}
