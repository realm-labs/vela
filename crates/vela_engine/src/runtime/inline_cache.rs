use std::cell::RefCell;

use vela_bytecode::CacheSiteId;
use vela_common::GlobalSlot;
use vela_vm::{
    HostInlineCacheEntry, MethodInlineCacheEntry, NativeInlineCacheEntry,
    RecordFieldInlineCacheEntry,
};

use super::image::RuntimeImage;

#[derive(Debug, Default)]
pub(super) struct InlineCaches {
    entries: RefCell<Vec<InlineCacheEntry>>,
}

#[derive(Clone, Debug)]
pub(super) enum InlineCacheEntry {
    Empty,
    GlobalRead { slot: GlobalSlot },
    HostAccess(HostInlineCacheEntry),
    RecordField(RecordFieldInlineCacheEntry),
    MethodDispatch(MethodInlineCacheEntry),
    NativeCall(NativeInlineCacheEntry),
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

    pub(super) fn record_field(&self, site: CacheSiteId) -> Option<RecordFieldInlineCacheEntry> {
        match self.entries.borrow().get(site.index()) {
            Some(InlineCacheEntry::RecordField(entry)) => Some(*entry),
            _ => None,
        }
    }

    pub(super) fn set_record_field(&self, site: CacheSiteId, entry: RecordFieldInlineCacheEntry) {
        if let Some(slot) = self.entries.borrow_mut().get_mut(site.index()) {
            *slot = InlineCacheEntry::RecordField(entry);
        }
    }

    pub(super) fn method_dispatch(&self, site: CacheSiteId) -> Option<MethodInlineCacheEntry> {
        match self.entries.borrow().get(site.index()) {
            Some(InlineCacheEntry::MethodDispatch(entry)) => Some(*entry),
            _ => None,
        }
    }

    pub(super) fn set_method_dispatch(&self, site: CacheSiteId, entry: MethodInlineCacheEntry) {
        if let Some(slot) = self.entries.borrow_mut().get_mut(site.index()) {
            *slot = InlineCacheEntry::MethodDispatch(entry);
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

    fn record_field(&self, site: CacheSiteId) -> Option<RecordFieldInlineCacheEntry> {
        self.record_field(site)
    }

    fn set_record_field(&self, site: CacheSiteId, entry: RecordFieldInlineCacheEntry) {
        self.set_record_field(site, entry);
    }

    fn method_dispatch(&self, site: CacheSiteId) -> Option<MethodInlineCacheEntry> {
        self.method_dispatch(site)
    }

    fn set_method_dispatch(&self, site: CacheSiteId, entry: MethodInlineCacheEntry) {
        self.set_method_dispatch(site, entry);
    }

    fn native_call(&self, site: CacheSiteId) -> Option<NativeInlineCacheEntry> {
        match self.entries.borrow().get(site.index()) {
            Some(InlineCacheEntry::NativeCall(entry)) => Some(entry.clone()),
            _ => None,
        }
    }

    fn set_native_call(&self, site: CacheSiteId, entry: NativeInlineCacheEntry) {
        if let Some(slot) = self.entries.borrow_mut().get_mut(site.index()) {
            *slot = InlineCacheEntry::NativeCall(entry);
        }
    }
}

#[cfg(test)]
#[path = "inline_cache_core_tests.rs"]
mod core_tests;
#[cfg(test)]
#[path = "inline_cache_host_tests.rs"]
mod host_tests;
#[cfg(test)]
#[path = "inline_cache_hot_reload_tests.rs"]
mod hot_reload_tests;
#[cfg(test)]
#[path = "inline_cache_method_tests.rs"]
mod method_tests;
#[cfg(test)]
#[path = "inline_cache_native_tests.rs"]
mod native_tests;

#[cfg(test)]
mod tests {
    use vela_bytecode::{CacheSiteKind, HostTargetPlanId};
    use vela_common::{HostMethodId, HostObjectId, HostTypeId, SourceId};
    use vela_def::{FieldId, TypeId};
    use vela_host::access::HostAccess;
    use vela_host::mock::MockStateAdapter;
    use vela_host::path::{HostPath, HostRef};
    use vela_host::resolved::{
        HostAccessOp, HostMutationOp, HostSchemaEpoch, ResolvedHostAccess, ResolvedHostAccessKind,
    };
    use vela_host::value::HostValue;
    use vela_reflect::registry::{FieldDesc, MethodDesc, TypeDesc, TypeKey};
    use vela_vm::{HostInlineCacheEntry, owned_value::OwnedValue};

    use crate::engine::Engine;
    use crate::runtime::{CallOptions, Runtime};

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

    #[test]
    fn host_access_inline_cache_refreshes_on_schema_epoch_change() {
        let engine = Engine::builder()
            .register_type(
                TypeDesc::new(TypeKey::new(TypeId::new(1), "EpochHostPlayer"))
                    .host_type(HostTypeId::new(1))
                    .field(FieldDesc::new(FieldId::new(1), "level")),
            )
            .build()
            .expect("engine should build");
        let program = engine
            .compile_source(
                SourceId::new(1),
                r#"
fn read_level(player: EpochHostPlayer) {
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
        let mut runtime = Runtime::new(engine, program);
        let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
        let host_path = HostPath::new(host_ref).field(FieldId::new(1));
        let mut adapter = MockStateAdapter::new();
        adapter.insert_diagnostic_path_value(
            host_path.clone(),
            HostValue::Scalar(vela_common::ScalarValue::I64(12)),
        );
        let mut access = HostAccess::new();

        let value = runtime
            .call_raw(
                "read_level",
                &[OwnedValue::HostRef(host_ref)],
                CallOptions::unbounded(),
                &mut adapter,
                &mut access,
            )
            .expect("first read_level should run");
        assert_eq!(value, OwnedValue::Scalar(vela_common::ScalarValue::I64(12)));
        assert_eq!(
            runtime
                .state
                .inline_caches
                .host_access(cache_site)
                .expect("host read should populate cache")
                .schema_epoch,
            HostSchemaEpoch::new(0)
        );

        adapter.set_schema_epoch(HostSchemaEpoch::new(1));
        adapter.insert_diagnostic_path_value(
            host_path,
            HostValue::Scalar(vela_common::ScalarValue::I64(21)),
        );
        let value = runtime
            .call_raw(
                "read_level",
                &[OwnedValue::HostRef(host_ref)],
                CallOptions::unbounded(),
                &mut adapter,
                &mut access,
            )
            .expect("second read_level should run through refreshed cache");

        assert_eq!(value, OwnedValue::Scalar(vela_common::ScalarValue::I64(21)));
        let entry = runtime
            .state
            .inline_caches
            .host_access(cache_site)
            .expect("host read should refresh cache");
        assert_eq!(entry.schema_epoch, HostSchemaEpoch::new(1));
        assert_eq!(entry.resolved.schema_epoch, HostSchemaEpoch::new(1));
    }

    #[test]
    fn host_write_inline_cache_refreshes_on_schema_epoch_change() {
        let engine = Engine::builder()
            .register_type(
                TypeDesc::new(TypeKey::new(TypeId::new(1), "EpochWriteHostPlayer"))
                    .host_type(HostTypeId::new(1))
                    .field(FieldDesc::new(FieldId::new(1), "level").writable(true)),
            )
            .build()
            .expect("engine should build");
        let program = engine
            .compile_source(
                SourceId::new(1),
                r#"
fn write_level(player: EpochWriteHostPlayer, value: i64) {
    player.level = value;
}
"#,
            )
            .expect("program should compile");
        let function = program
            .function("write_level")
            .expect("write_level should exist");
        let cache_site = function
            .cache_sites
            .sites()
            .iter()
            .find(|site| site.kind == CacheSiteKind::HostPathWrite)
            .expect("write_level should have host write site")
            .id;
        let mut runtime = Runtime::new(engine, program);
        let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
        let host_path = HostPath::new(host_ref).field(FieldId::new(1));
        let mut adapter = MockStateAdapter::new();
        let mut access = HostAccess::new();

        runtime
            .call_raw(
                "write_level",
                &[
                    OwnedValue::HostRef(host_ref),
                    OwnedValue::Scalar(vela_common::ScalarValue::I64(12)),
                ],
                CallOptions::unbounded(),
                &mut adapter,
                &mut access,
            )
            .expect("first write_level should run");
        assert_eq!(
            adapter.read_diagnostic_path(&host_path),
            Ok(HostValue::Scalar(vela_common::ScalarValue::I64(12)))
        );
        let entry = runtime
            .state
            .inline_caches
            .host_access(cache_site)
            .expect("host write should populate cache");
        assert_eq!(entry.root_type, HostTypeId::new(1));
        assert_eq!(entry.plan_id.index(), 0);
        assert_eq!(entry.op, HostAccessOp::Write);
        assert_eq!(entry.schema_epoch, HostSchemaEpoch::new(0));

        adapter.set_schema_epoch(HostSchemaEpoch::new(1));
        runtime
            .call_raw(
                "write_level",
                &[
                    OwnedValue::HostRef(host_ref),
                    OwnedValue::Scalar(vela_common::ScalarValue::I64(21)),
                ],
                CallOptions::unbounded(),
                &mut adapter,
                &mut access,
            )
            .expect("second write_level should run through refreshed cache");

        assert_eq!(
            adapter.read_diagnostic_path(&host_path),
            Ok(HostValue::Scalar(vela_common::ScalarValue::I64(21)))
        );
        let entry = runtime
            .state
            .inline_caches
            .host_access(cache_site)
            .expect("host write should refresh cache");
        assert_eq!(entry.op, HostAccessOp::Write);
        assert_eq!(entry.schema_epoch, HostSchemaEpoch::new(1));
        assert_eq!(entry.resolved.schema_epoch, HostSchemaEpoch::new(1));
    }

    #[test]
    fn host_access_inline_cache_misses_wrong_operation_guard() {
        let engine = Engine::builder()
            .register_type(
                TypeDesc::new(TypeKey::new(TypeId::new(1), "GuardedHostPlayer"))
                    .host_type(HostTypeId::new(1))
                    .field(FieldDesc::new(FieldId::new(1), "level").writable(true)),
            )
            .build()
            .expect("engine should build");
        let program = engine
            .compile_source(
                SourceId::new(1),
                r#"
fn write_level(player: GuardedHostPlayer, value: i64) {
    player.level = value;
}
"#,
            )
            .expect("program should compile");
        let function = program
            .function("write_level")
            .expect("write_level should exist");
        let cache_site = function
            .cache_sites
            .sites()
            .iter()
            .find(|site| site.kind == CacheSiteKind::HostPathWrite)
            .expect("write_level should have host write site")
            .id;
        let mut runtime = Runtime::new(engine, program);
        runtime.state.inline_caches.set_host_access(
            cache_site,
            HostInlineCacheEntry {
                root_type: HostTypeId::new(1),
                plan_id: HostTargetPlanId::new(0),
                op: HostAccessOp::Read,
                schema_epoch: HostSchemaEpoch::new(0),
                resolved: ResolvedHostAccess::generic_target(HostSchemaEpoch::new(0)),
            },
        );

        let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
        let host_path = HostPath::new(host_ref).field(FieldId::new(1));
        let mut adapter = MockStateAdapter::new();
        let mut access = HostAccess::new();

        runtime
            .call_raw(
                "write_level",
                &[
                    OwnedValue::HostRef(host_ref),
                    OwnedValue::Scalar(vela_common::ScalarValue::I64(12)),
                ],
                CallOptions::unbounded(),
                &mut adapter,
                &mut access,
            )
            .expect("write_level should miss wrong-op cache and run");

        assert_eq!(
            adapter.read_diagnostic_path(&host_path),
            Ok(HostValue::Scalar(vela_common::ScalarValue::I64(12)))
        );
        let entry = runtime
            .state
            .inline_caches
            .host_access(cache_site)
            .expect("wrong-op cache entry should be replaced");
        assert_eq!(entry.root_type, HostTypeId::new(1));
        assert_eq!(entry.plan_id.index(), 0);
        assert_eq!(entry.op, HostAccessOp::Write);
        assert_eq!(entry.schema_epoch, HostSchemaEpoch::new(0));
        assert_eq!(entry.resolved.schema_epoch, HostSchemaEpoch::new(0));
    }

    #[test]
    fn host_access_inline_cache_misses_wrong_target_guards() {
        let engine = Engine::builder()
            .register_type(
                TypeDesc::new(TypeKey::new(TypeId::new(1), "TargetGuardHostPlayer"))
                    .host_type(HostTypeId::new(1))
                    .field(FieldDesc::new(FieldId::new(1), "level").writable(true)),
            )
            .build()
            .expect("engine should build");
        let program = engine
            .compile_source(
                SourceId::new(1),
                r#"
fn write_level(player: TargetGuardHostPlayer, value: i64) {
    player.level = value;
}
"#,
            )
            .expect("program should compile");
        let function = program
            .function("write_level")
            .expect("write_level should exist");
        let cache_site = function
            .cache_sites
            .sites()
            .iter()
            .find(|site| site.kind == CacheSiteKind::HostPathWrite)
            .expect("write_level should have host write site")
            .id;
        let mut runtime = Runtime::new(engine, program);
        let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
        let host_path = HostPath::new(host_ref).field(FieldId::new(1));
        let mut adapter = MockStateAdapter::new();
        let mut access = HostAccess::new();

        runtime.state.inline_caches.set_host_access(
            cache_site,
            HostInlineCacheEntry {
                root_type: HostTypeId::new(2),
                plan_id: HostTargetPlanId::new(0),
                op: HostAccessOp::Write,
                schema_epoch: HostSchemaEpoch::new(0),
                resolved: ResolvedHostAccess::generic_target(HostSchemaEpoch::new(0)),
            },
        );
        runtime
            .call_raw(
                "write_level",
                &[
                    OwnedValue::HostRef(host_ref),
                    OwnedValue::Scalar(vela_common::ScalarValue::I64(12)),
                ],
                CallOptions::unbounded(),
                &mut adapter,
                &mut access,
            )
            .expect("write_level should miss wrong-root cache and run");
        assert_eq!(
            adapter.read_diagnostic_path(&host_path),
            Ok(HostValue::Scalar(vela_common::ScalarValue::I64(12)))
        );
        let entry = runtime
            .state
            .inline_caches
            .host_access(cache_site)
            .expect("wrong-root cache entry should be replaced");
        assert_eq!(entry.root_type, HostTypeId::new(1));
        assert_eq!(entry.plan_id.index(), 0);
        assert_eq!(entry.op, HostAccessOp::Write);

        runtime.state.inline_caches.set_host_access(
            cache_site,
            HostInlineCacheEntry {
                root_type: HostTypeId::new(1),
                plan_id: HostTargetPlanId::new(1),
                op: HostAccessOp::Write,
                schema_epoch: HostSchemaEpoch::new(0),
                resolved: ResolvedHostAccess::generic_target(HostSchemaEpoch::new(0)),
            },
        );
        runtime
            .call_raw(
                "write_level",
                &[
                    OwnedValue::HostRef(host_ref),
                    OwnedValue::Scalar(vela_common::ScalarValue::I64(21)),
                ],
                CallOptions::unbounded(),
                &mut adapter,
                &mut access,
            )
            .expect("write_level should miss wrong-plan cache and run");

        assert_eq!(
            adapter.read_diagnostic_path(&host_path),
            Ok(HostValue::Scalar(vela_common::ScalarValue::I64(21)))
        );
        let entry = runtime
            .state
            .inline_caches
            .host_access(cache_site)
            .expect("wrong-plan cache entry should be replaced");
        assert_eq!(entry.root_type, HostTypeId::new(1));
        assert_eq!(entry.plan_id.index(), 0);
        assert_eq!(entry.op, HostAccessOp::Write);
        assert_eq!(entry.schema_epoch, HostSchemaEpoch::new(0));
        assert_eq!(entry.resolved.schema_epoch, HostSchemaEpoch::new(0));
    }

    #[test]
    fn host_mutate_inline_cache_refreshes_on_schema_epoch_change() {
        let engine = Engine::builder()
            .register_type(
                TypeDesc::new(TypeKey::new(TypeId::new(1), "EpochMutateHostPlayer"))
                    .host_type(HostTypeId::new(1))
                    .field(FieldDesc::new(FieldId::new(1), "level").writable(true)),
            )
            .build()
            .expect("engine should build");
        let program = engine
            .compile_source(
                SourceId::new(1),
                r#"
fn gain_level(player: EpochMutateHostPlayer, amount: i64) {
    player.level += amount;
}
"#,
            )
            .expect("program should compile");
        let function = program
            .function("gain_level")
            .expect("gain_level should exist");
        let cache_site = function
            .cache_sites
            .sites()
            .iter()
            .find(|site| site.kind == CacheSiteKind::HostPathMutate)
            .expect("gain_level should have host mutate site")
            .id;
        let mut runtime = Runtime::new(engine, program);
        let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
        let host_path = HostPath::new(host_ref).field(FieldId::new(1));
        let mut adapter = MockStateAdapter::new();
        adapter.insert_diagnostic_path_value(
            host_path.clone(),
            HostValue::Scalar(vela_common::ScalarValue::I64(10)),
        );
        let mut access = HostAccess::new();

        runtime
            .call_raw(
                "gain_level",
                &[
                    OwnedValue::HostRef(host_ref),
                    OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
                ],
                CallOptions::unbounded(),
                &mut adapter,
                &mut access,
            )
            .expect("first gain_level should run");
        assert_eq!(
            adapter.read_diagnostic_path(&host_path),
            Ok(HostValue::Scalar(vela_common::ScalarValue::I64(12)))
        );
        let entry = runtime
            .state
            .inline_caches
            .host_access(cache_site)
            .expect("host mutate should populate cache");
        assert_eq!(entry.root_type, HostTypeId::new(1));
        assert_eq!(entry.plan_id.index(), 0);
        assert_eq!(entry.op, HostAccessOp::Mutate(HostMutationOp::Add));
        assert_eq!(entry.schema_epoch, HostSchemaEpoch::new(0));

        adapter.set_schema_epoch(HostSchemaEpoch::new(1));
        runtime
            .call_raw(
                "gain_level",
                &[
                    OwnedValue::HostRef(host_ref),
                    OwnedValue::Scalar(vela_common::ScalarValue::I64(3)),
                ],
                CallOptions::unbounded(),
                &mut adapter,
                &mut access,
            )
            .expect("second gain_level should run through refreshed cache");

        assert_eq!(
            adapter.read_diagnostic_path(&host_path),
            Ok(HostValue::Scalar(vela_common::ScalarValue::I64(15)))
        );
        let entry = runtime
            .state
            .inline_caches
            .host_access(cache_site)
            .expect("host mutate should refresh cache");
        assert_eq!(entry.op, HostAccessOp::Mutate(HostMutationOp::Add));
        assert_eq!(entry.schema_epoch, HostSchemaEpoch::new(1));
        assert_eq!(entry.resolved.schema_epoch, HostSchemaEpoch::new(1));
    }

    #[test]
    fn host_call_inline_cache_refreshes_on_schema_epoch_change() {
        let method = HostMethodId::new(9);
        let engine = Engine::builder()
            .register_type(
                TypeDesc::new(TypeKey::new(TypeId::new(1), "EpochCallHostPlayer"))
                    .host_type(HostTypeId::new(1))
                    .method(MethodDesc::new(method, "award")),
            )
            .build()
            .expect("engine should build");
        let program = engine
            .compile_source(
                SourceId::new(1),
                r#"
fn award(player: EpochCallHostPlayer, amount: i64) {
    return player.award(amount);
}
"#,
            )
            .expect("program should compile");
        let function = program.function("award").expect("award should exist");
        let cache_site = function
            .cache_sites
            .sites()
            .iter()
            .find(|site| site.kind == CacheSiteKind::HostPathCall)
            .expect("award should have host call site")
            .id;
        let mut runtime = Runtime::new(engine, program);
        let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
        let mut adapter = MockStateAdapter::new();
        adapter.insert_method_return(method, HostValue::Scalar(vela_common::ScalarValue::I64(12)));
        let mut access = HostAccess::new();

        let value = runtime
            .call_raw(
                "award",
                &[
                    OwnedValue::HostRef(host_ref),
                    OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
                ],
                CallOptions::unbounded(),
                &mut adapter,
                &mut access,
            )
            .expect("first award should run");
        assert_eq!(value, OwnedValue::Scalar(vela_common::ScalarValue::I64(12)));
        assert_eq!(
            adapter.method_calls(),
            &[(
                HostPath::new(host_ref),
                method,
                vec![HostValue::Scalar(vela_common::ScalarValue::I64(2))]
            )]
        );
        let entry = runtime
            .state
            .inline_caches
            .host_access(cache_site)
            .expect("host call should populate cache");
        assert_eq!(entry.root_type, HostTypeId::new(1));
        assert_eq!(entry.plan_id.index(), 0);
        assert_eq!(entry.op, HostAccessOp::Call(method));
        assert_eq!(entry.schema_epoch, HostSchemaEpoch::new(0));

        adapter.set_schema_epoch(HostSchemaEpoch::new(1));
        adapter.insert_method_return(method, HostValue::Scalar(vela_common::ScalarValue::I64(21)));
        let value = runtime
            .call_raw(
                "award",
                &[
                    OwnedValue::HostRef(host_ref),
                    OwnedValue::Scalar(vela_common::ScalarValue::I64(3)),
                ],
                CallOptions::unbounded(),
                &mut adapter,
                &mut access,
            )
            .expect("second award should run through refreshed cache");

        assert_eq!(value, OwnedValue::Scalar(vela_common::ScalarValue::I64(21)));
        assert_eq!(
            adapter.method_calls()[1],
            (
                HostPath::new(host_ref),
                method,
                vec![HostValue::Scalar(vela_common::ScalarValue::I64(3))]
            )
        );
        let entry = runtime
            .state
            .inline_caches
            .host_access(cache_site)
            .expect("host call should refresh cache");
        assert_eq!(entry.op, HostAccessOp::Call(method));
        assert_eq!(entry.schema_epoch, HostSchemaEpoch::new(1));
        assert_eq!(entry.resolved.schema_epoch, HostSchemaEpoch::new(1));
    }

    #[test]
    fn host_remove_inline_cache_refreshes_on_schema_epoch_change() {
        let inventory = FieldId::new(8);
        let items = FieldId::new(9);
        let engine = Engine::builder()
            .register_type(
                TypeDesc::new(TypeKey::new(TypeId::new(1), "EpochRemoveHostPlayer"))
                    .host_type(HostTypeId::new(1))
                    .field(FieldDesc::new(inventory, "inventory").type_hint("EpochInventory")),
            )
            .register_type(
                TypeDesc::new(TypeKey::new(TypeId::new(2), "EpochInventory"))
                    .host_type(HostTypeId::new(2))
                    .field(FieldDesc::new(items, "items").writable(true)),
            )
            .build()
            .expect("engine should build");
        let program = engine
            .compile_source(
                SourceId::new(1),
                r#"
fn remove_item(player: EpochRemoveHostPlayer, item_id: string) {
    player.inventory.items[item_id].remove();
}
"#,
            )
            .expect("program should compile");
        let function = program
            .function("remove_item")
            .expect("remove_item should exist");
        let cache_site = function
            .cache_sites
            .sites()
            .iter()
            .find(|site| site.kind == CacheSiteKind::HostPathRemove)
            .expect("remove_item should have host remove site")
            .id;
        let mut runtime = Runtime::new(engine, program);
        let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
        let host_path = HostPath::new(host_ref)
            .field(inventory)
            .field(items)
            .key("gold");
        let mut adapter = MockStateAdapter::new();
        adapter.insert_diagnostic_path_value(host_path.clone(), HostValue::String("gold".into()));
        let mut access = HostAccess::new();

        runtime
            .call_raw(
                "remove_item",
                &[
                    OwnedValue::HostRef(host_ref),
                    OwnedValue::String("gold".into()),
                ],
                CallOptions::unbounded(),
                &mut adapter,
                &mut access,
            )
            .expect("first remove_item should run");
        assert!(adapter.read_diagnostic_path(&host_path).is_err());
        let entry = runtime
            .state
            .inline_caches
            .host_access(cache_site)
            .expect("host remove should populate cache");
        assert_eq!(entry.root_type, HostTypeId::new(1));
        assert_eq!(entry.plan_id.index(), 0);
        assert_eq!(entry.op, HostAccessOp::Remove);
        assert_eq!(entry.schema_epoch, HostSchemaEpoch::new(0));

        adapter.set_schema_epoch(HostSchemaEpoch::new(1));
        adapter.insert_diagnostic_path_value(host_path.clone(), HostValue::String("gold".into()));
        runtime
            .call_raw(
                "remove_item",
                &[
                    OwnedValue::HostRef(host_ref),
                    OwnedValue::String("gold".into()),
                ],
                CallOptions::unbounded(),
                &mut adapter,
                &mut access,
            )
            .expect("second remove_item should run through refreshed cache");

        assert!(adapter.read_diagnostic_path(&host_path).is_err());
        let entry = runtime
            .state
            .inline_caches
            .host_access(cache_site)
            .expect("host remove should refresh cache");
        assert_eq!(entry.op, HostAccessOp::Remove);
        assert_eq!(entry.schema_epoch, HostSchemaEpoch::new(1));
        assert_eq!(entry.resolved.schema_epoch, HostSchemaEpoch::new(1));
    }
}
