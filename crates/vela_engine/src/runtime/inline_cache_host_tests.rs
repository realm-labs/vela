use vela_bytecode::{CacheSiteId, CacheSiteKind};
use vela_common::{HostObjectId, HostTypeId, ScalarValue, SourceId};
use vela_def::{FieldId, TypeId};
use vela_host::access::HostAccess;
use vela_host::mock::MockStateAdapter;
use vela_host::path::{HostPath, HostRef};
use vela_host::value::HostValue;
use vela_reflect::registry::{FieldDesc, TypeDesc, TypeKey};
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::runtime::{CallOptions, Runtime};

#[test]
fn accepted_hot_reload_clears_host_access_inline_caches() {
    let level = FieldId::new(1);
    let engine = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "ReloadHostPlayer"))
                .host_type(HostTypeId::new(1))
                .field(FieldDesc::new(level, "level")),
        )
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(1),
            r#"
fn read_level(player: ReloadHostPlayer) {
    return player.level;
}
"#,
        )
        .expect("initial source should compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let initial_site = host_read_site(&runtime, "read_level");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let host_path = HostPath::new(host_ref).field(level);
    let mut adapter = MockStateAdapter::new();
    adapter
        .insert_diagnostic_path_value(host_path.clone(), HostValue::Scalar(ScalarValue::I64(12)));
    let mut access = HostAccess::new();

    let first = runtime
        .call_raw(
            "read_level",
            &[OwnedValue::HostRef(host_ref)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut access,
        )
        .expect("initial read_level should run");
    assert_eq!(first, OwnedValue::Scalar(ScalarValue::I64(12)));
    assert!(
        runtime
            .state
            .inline_caches
            .host_access(initial_site)
            .is_some(),
        "initial host read should populate its inline cache"
    );

    let update = runtime
        .compile_hot_reload_update_with_id(
            SourceId::new(2),
            r#"
fn read_level(player: ReloadHostPlayer) {
    return player.level + 1;
}
"#,
        )
        .expect("runtime should compile host read hot reload update")
        .expect("host read body update should be accepted");
    let report = runtime
        .apply_hot_update(update)
        .expect("host read hot reload update should apply");
    assert!(report.accepted);

    let reloaded_site = host_read_site(&runtime, "read_level");
    assert_eq!(runtime.state.inline_caches.host_access(reloaded_site), None);

    adapter.insert_diagnostic_path_value(host_path, HostValue::Scalar(ScalarValue::I64(12)));
    let second = runtime
        .call_raw(
            "read_level",
            &[OwnedValue::HostRef(host_ref)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut access,
        )
        .expect("reloaded read_level should run");
    assert_eq!(second, OwnedValue::Scalar(ScalarValue::I64(13)));
    assert!(
        runtime
            .state
            .inline_caches
            .host_access(reloaded_site)
            .is_some(),
        "reloaded host read should repopulate its inline cache"
    );
}

fn host_read_site(runtime: &Runtime, function_name: &str) -> CacheSiteId {
    runtime
        .image
        .program_image()
        .function_by_name(function_name)
        .unwrap_or_else(|| panic!("{function_name} should exist"))
        .cache_sites
        .sites()
        .iter()
        .find(|site| site.kind == CacheSiteKind::HostPathRead)
        .unwrap_or_else(|| panic!("{function_name} should have a host read site"))
        .id
}
