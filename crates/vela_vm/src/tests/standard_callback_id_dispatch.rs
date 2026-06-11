use super::standard_id_dispatch::std_method_id;
use super::*;
use std::cell::RefCell;
use vela_bytecode::CacheSiteId;

#[test]
fn linked_callback_method_id_rejects_receiver_owner_mismatch() {
    let mut program = compile_standard_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let mapped = [1, 2, 3].map(|value| value + 1);
    return mapped[0];
}
"#,
    )
    .expect("standard callback method source should compile");
    replace_call_method_id(
        &mut program,
        std_method_id("Array", "map"),
        std_method_id("Set", "map"),
    );

    let mut budget = ExecutionBudget::unbounded();
    let error = run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget)
        .expect_err("linked callback dispatch must reject owner-mismatched method ids");

    assert_eq!(
        error.kind(),
        VmErrorKind::UnknownMethod {
            method: "map".to_owned()
        }
    );
}

#[test]
fn linked_callback_methods_forward_inline_caches_to_callback_body() {
    let host_ref = player_ref(3);
    let mut registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let player = registry
        .register_type(
            vela_registry::TypeDef::new(vela_def::DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "Player",
            ))
            .host_runtime_id(host_ref.type_id.get().into()),
        )
        .expect("test host type should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                vela_def::DefPath::field("host", std::iter::empty::<&str>(), "Player", "level"),
                player,
            )
            .host_runtime_id(level_field().get())
            .type_hint(Some("i64".to_owned())),
        )
        .expect("test host field should register");
    let program = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    let mapped = [1, 2, 3].map(|value| value + player.level);
    return mapped[0];
}
"#,
        registry.compile_view(),
    )
    .expect("standard callback method source should compile");
    let linked = link_test_program(&program);
    let caches = RecordingHostAccessCaches::new(
        linked
            .functions()
            .map(|(_, code)| code.cache_sites.len())
            .max()
            .unwrap_or(0),
    );
    let mut adapter = host_adapter(
        host_ref,
        HostValue::Scalar(vela_common::ScalarValue::I64(40)),
    );
    let mut access = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut access,
        script_globals: None,
    };
    let mut budget = ExecutionBudget::unbounded();

    let result = Vm::new()
        .run_linked_program_with_host_budget_and_caches(
            &linked,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
            &mut budget,
            Some(&caches),
        )
        .expect("linked callback should run");

    assert_eq!(
        result,
        OwnedValue::Scalar(vela_common::ScalarValue::I64(41))
    );
    assert!(
        caches.recorded_entry().is_some(),
        "callback host read should populate the shared inline cache provider"
    );
}

struct RecordingHostAccessCaches {
    len: usize,
    entry: RefCell<Option<HostInlineCacheEntry>>,
}

impl RecordingHostAccessCaches {
    fn new(len: usize) -> Self {
        Self {
            len,
            entry: RefCell::new(None),
        }
    }

    fn recorded_entry(&self) -> Option<HostInlineCacheEntry> {
        *self.entry.borrow()
    }
}

impl VmInlineCaches for RecordingHostAccessCaches {
    fn len(&self) -> usize {
        self.len
    }

    fn host_access(&self, _site: CacheSiteId) -> Option<HostInlineCacheEntry> {
        *self.entry.borrow()
    }

    fn set_host_access(&self, _site: CacheSiteId, entry: HostInlineCacheEntry) {
        *self.entry.borrow_mut() = Some(entry);
    }
}

fn replace_call_method_id(
    program: &mut UnlinkedProgram,
    expected_method: MethodId,
    replacement_method: MethodId,
) {
    let code = program
        .function_mut("main")
        .expect("test function should exist");
    for instruction in &mut code.instructions {
        if let UnlinkedInstructionKind::CallMethodId { method_id, .. } = &mut instruction.kind
            && *method_id == expected_method
        {
            *method_id = replacement_method;
            return;
        }
    }
    panic!("test method call should exist");
}
