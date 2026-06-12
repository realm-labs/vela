use super::linked_standard_method_cache_support::{
    RecordingMethodCaches, run_linked_method_cache_program,
    run_linked_method_cache_program_with_standard_natives,
};
use super::standard_id_dispatch::std_method_id;
use super::*;
use std::cell::RefCell;
use vela_bytecode::{CacheSiteId, LinkedProgram, Linker};

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

#[test]
fn linked_callback_value_method_caches_array_any_target() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return [1, 2, 3].any(|value| value == 2);
}
"#,
    )
    .expect("standard callback method source should compile");
    let linked = link_test_program(&program);
    let site = linked_method_cache_site(&linked, "main", "any");
    let caches = RecordingMethodCaches::new(max_cache_site_len(&linked));

    assert_eq!(
        run_linked_method_cache_program(&linked, &caches),
        Ok(Value::Bool(true))
    );
    let entry = caches
        .entry(site)
        .expect("standard callback method cache should populate");
    let MethodInlineCacheTarget::CallbackValue {
        method_id,
        callback_method,
    } = entry.target
    else {
        panic!("standard callback method should store callback target");
    };
    assert_eq!(method_id, std_method_id("Array", "any"));
    assert_eq!(callback_method.receiver, StandardMethodReceiver::Array);
    assert_eq!(callback_method.target, CallbackMethodInlineCacheTarget::Any);
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_program(&linked, &caches),
        Ok(Value::Bool(true))
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_callback_value_method_caches_option_targets() {
    assert_callback_value_method_cache(
        r#"
fn main() {
    return option::some(4).map(|value| value + 1).unwrap_or(0);
}
"#,
        "map",
        "Option",
        "map",
        StandardMethodReceiver::Option,
        CallbackMethodInlineCacheTarget::Map,
        Value::i64(5),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return option::some(4).filter(|value| value > 3).unwrap_or(0);
}
"#,
        "filter",
        "Option",
        "filter",
        StandardMethodReceiver::Option,
        CallbackMethodInlineCacheTarget::Filter,
        Value::i64(4),
    );
}

#[test]
fn linked_callback_value_method_caches_result_targets() {
    assert_callback_value_method_cache(
        r#"
fn main() {
    return option::unwrap_or(result::err(4).map_err(|error| error + 1).to_error_option(), 0);
}
"#,
        "map_err",
        "Result",
        "map_err",
        StandardMethodReceiver::Result,
        CallbackMethodInlineCacheTarget::MapErr,
        Value::i64(5),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return result::unwrap_or(result::ok(4).and_then(|value| result::ok(value + 1)), 0);
}
"#,
        "and_then",
        "Result",
        "and_then",
        StandardMethodReceiver::Result,
        CallbackMethodInlineCacheTarget::AndThen,
        Value::i64(5),
    );
    assert_callback_value_method_cache(
        r#"
fn main() {
    return result::unwrap_or(result::err(4).or_else(|error| result::ok(error + 1)), 0);
}
"#,
        "or_else",
        "Result",
        "or_else",
        StandardMethodReceiver::Result,
        CallbackMethodInlineCacheTarget::OrElse,
        Value::i64(5),
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

fn linked_method_cache_site(
    program: &vela_bytecode::LinkedProgram,
    function: &str,
    method: &str,
) -> CacheSiteId {
    let (_, code) = program
        .functions()
        .find(|(_, code)| program.debug_name(code.debug_name) == function)
        .expect("linked function should exist");
    code.instructions
        .iter()
        .find_map(|instruction| {
            let vela_bytecode::linked::InstructionKind::CallMethod {
                debug_name,
                cache_site,
                ..
            } = instruction.kind
            else {
                return None;
            };
            if program.debug_name(debug_name) == method {
                cache_site
            } else {
                None
            }
        })
        .expect("linked method call should have cache site")
}

fn max_cache_site_len(program: &vela_bytecode::LinkedProgram) -> usize {
    program
        .functions()
        .map(|(_, code)| code.cache_sites.len())
        .max()
        .unwrap_or(0)
}

fn assert_callback_value_method_cache(
    source: &str,
    site_method: &str,
    method_owner: &str,
    method_name: &str,
    receiver: StandardMethodReceiver,
    target: CallbackMethodInlineCacheTarget,
    expected: Value,
) {
    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("standard callback method source should compile");
    let linked = link_standard_native_test_program(&program);
    let site = linked_method_cache_site(&linked, "main", site_method);
    let caches = RecordingMethodCaches::new(max_cache_site_len(&linked));

    assert_eq!(
        run_linked_method_cache_program_with_standard_natives(&linked, &caches),
        Ok(expected)
    );
    assert_callback_cache_entry(
        &caches,
        site,
        std_method_id(method_owner, method_name),
        receiver,
        target,
    );
    let warmed_set_count = caches.set_count();

    assert_eq!(
        run_linked_method_cache_program_with_standard_natives(&linked, &caches),
        Ok(expected)
    );
    assert_eq!(caches.set_count(), warmed_set_count);
}

fn assert_callback_cache_entry(
    caches: &RecordingMethodCaches,
    site: CacheSiteId,
    expected_method_id: MethodId,
    expected_receiver: StandardMethodReceiver,
    expected_target: CallbackMethodInlineCacheTarget,
) {
    let entry = caches
        .entry(site)
        .expect("standard callback method cache should populate");
    let MethodInlineCacheTarget::CallbackValue {
        method_id,
        callback_method,
    } = entry.target
    else {
        panic!("standard callback method should store callback target");
    };
    assert_eq!(method_id, expected_method_id);
    assert_eq!(callback_method.receiver, expected_receiver);
    assert_eq!(callback_method.target, expected_target);
}

fn link_standard_native_test_program(program: &UnlinkedProgram) -> LinkedProgram {
    let vm = Vm::new().with_standard_natives();
    let mut linker = Linker::new();
    for id in vm.native_implementation_ids() {
        linker.add_native_implementation(id);
    }
    linker
        .link_program(program)
        .expect("standard native test program should link")
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
