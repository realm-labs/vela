use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use super::*;

#[test]
fn removed_closure_state_does_not_pin_its_own_generation() {
    let engine = Engine::builder()
        .register_type_desc(direct_player_type())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(24),
            r#"
extern state host: Player;
state retired: Closure = || 5;
fn current() { return 1; }
"#,
        )
        .expect("initial generation");
    let vm_state = initial
        .linked_program()
        .states()
        .iter()
        .find(|state| state.qualified_name == "main::retired")
        .expect("VM state descriptor")
        .id;
    let extern_state = initial
        .linked_program()
        .states()
        .iter()
        .find(|state| state.qualified_name == "main::host")
        .expect("extern state descriptor")
        .id;
    let update = engine
        .compile_hot_reload_update_with_id(
            &initial,
            SourceId::new(25),
            "fn current() { return 2; }",
        )
        .expect("private state removal is compatible");
    let drops = Arc::new(AtomicUsize::new(0));
    let mut builder = Runtime::builder_from_hot_reload_version(engine, initial);
    builder
        .bind_extern_state(
            "main::host",
            DropTrackedHost {
                drops: Arc::clone(&drops),
            },
        )
        .expect("extern state binding");
    let mut runtime = builder.build().expect("runtime initializes");

    let report = runtime.apply_hot_update(update).expect("reload applies");
    assert!(report.accepted);
    assert_eq!(runtime.check_reload(), Ok(None));

    assert_eq!(runtime.retained_generation_count(), 1);
    assert!(!runtime.retains_vm_state_id(vm_state));
    assert!(!runtime.retains_extern_state_id(extern_state));
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn reload_safe_point_reclaims_removed_state_after_final_old_owner_drops() {
    let engine = Engine::builder()
        .register_type_desc(direct_player_type())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(26),
            r#"
extern state host: Player;
state retired: i64 = 5;
fn make() { return || retired; }
fn invoke(callback) { return callback(); }
"#,
        )
        .expect("initial generation");
    let vm_state = initial
        .linked_program()
        .states()
        .iter()
        .find(|state| state.qualified_name == "main::retired")
        .expect("VM state descriptor")
        .id;
    let extern_state = initial
        .linked_program()
        .states()
        .iter()
        .find(|state| state.qualified_name == "main::host")
        .expect("extern state descriptor")
        .id;
    let update = engine
        .compile_hot_reload_update_with_id(
            &initial,
            SourceId::new(27),
            "fn make() { return || 0; } fn invoke(callback) { return callback(); }",
        )
        .expect("private state removal is compatible");
    let drops = Arc::new(AtomicUsize::new(0));
    let mut builder = Runtime::builder_from_hot_reload_version(engine, initial);
    builder
        .bind_extern_state(
            "main::host",
            DropTrackedHost {
                drops: Arc::clone(&drops),
            },
        )
        .expect("extern state binding");
    let mut runtime = builder.build().expect("runtime initializes");
    let old_closure = runtime
        .call("make", CallArgs::new(), CallOptions::unbounded())
        .expect("old closure");

    let report = runtime.apply_hot_update(update).expect("reload applies");

    assert!(report.accepted);
    assert_eq!(runtime.retained_generation_count(), 2);
    assert!(runtime.retains_vm_state_id(vm_state));
    assert!(runtime.retains_extern_state_id(extern_state));
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(runtime.check_reload(), Ok(None));
    assert_eq!(runtime.retained_generation_count(), 2);
    assert!(runtime.retains_vm_state_id(vm_state));
    assert!(runtime.retains_extern_state_id(extern_state));

    drop(old_closure);
    assert_eq!(runtime.check_reload(), Ok(None));

    assert_eq!(runtime.retained_generation_count(), 1);
    assert!(!runtime.retains_vm_state_id(vm_state));
    assert!(!runtime.retains_extern_state_id(extern_state));
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

struct DropTrackedHost {
    drops: Arc<AtomicUsize>,
}

impl Drop for DropTrackedHost {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

impl ScriptHostObject for DropTrackedHost {
    fn host_type_id(&self) -> HostTypeId {
        HostTypeId::new(1)
    }

    fn read_resolved_host(
        &self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue> {
        Err(HostError {
            kind: HostErrorKind::MissingPath {
                path: target.to_diagnostic_path().to_host_path(),
            },
            source_span: None,
        })
    }
}
