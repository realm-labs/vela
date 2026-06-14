use super::linked_standard_method_cache_support::RecordingMethodCaches;
use super::*;
use crate::owned_value::OwnedValue;

fn run_script_method_program(
    vm: &Vm,
    program: &UnlinkedProgram,
    entry: &str,
    args: &[OwnedValue],
) -> VmResult<OwnedValue> {
    let mut budget = ExecutionBudget::unbounded();
    run_linked_test_program_with_budget(vm, program, entry, args, &mut budget)
}

fn run_script_method_program_with_host(
    vm: &Vm,
    program: &UnlinkedProgram,
    entry: &str,
    args: &[OwnedValue],
    host: &mut HostExecution<'_>,
) -> VmResult<OwnedValue> {
    let mut budget = ExecutionBudget::unbounded();
    run_linked_test_program_with_host_budget(vm, program, entry, args, host, &mut budget)
}

fn host_ref_script_method_registry(
    host_type: HostTypeId,
    natives: &[&str],
) -> vela_registry::DefinitionRegistry {
    let mut registry = host_definition_registry(&[("Player", host_type)], &[], &[]);
    for native in natives {
        let mut segments = native.split("::").collect::<Vec<_>>();
        let function = segments.pop().unwrap_or(native);
        registry
            .register_function(vela_registry::FunctionDef::new(
                vela_def::DefPath::function("host", segments, function),
                vela_registry::FunctionSignature::default(),
            ))
            .expect("test native function should register");
    }
    registry
}

#[test]
fn runs_compiled_script_value_methods() {
    let program = compile_standard_program_source(
            SourceId::new(1),
            r#"
fn main() {
    let values = [1, 2, 3];
    let rewards = {"gold": 4, "xp": 6};
    let empty = [];
    values.push(4);
    let popped = values.pop();
    let missing_pop = empty.pop();
    rewards.set("quest", 8);
    let missing_get = rewards.get("missing_before");
    let removed = rewards.remove("gold");
    let missing_remove = rewards.remove("missing_after");
    let keys = rewards.keys().collect_array();
    let amounts = rewards.values().collect_array();
    let entries = rewards.entries().collect_array();
    if empty.is_empty() && values.len() == 3 && option::unwrap_or(popped, 0) == 4 && rewards.len() == 2 && ("gold").len() == 4
        && ("gold").contains("ol") && ("quest").starts_with("que") && ("quest").ends_with("st")
        && option::is_none(missing_pop)
        && option::unwrap_or(removed, 0) == 4
        && option::is_none(missing_get) && option::is_none(missing_remove)
        && rewards.has("quest") && option::unwrap_or(rewards.get("xp"), 0) == 6 && rewards.get_or("missing", 10) == 10
        && keys[0] == "quest" && keys[1] == "xp"
        && amounts[0] == 8 && amounts[1] == 6
        && entries[0].key == "quest" && entries[1].value == 6 {
        return values.len();
    }
    return 0;
}
"#,
        )
        .expect("compile script value methods");

    let mut vm = Vm::new();
    vm.register_standard_natives();

    assert_eq!(
        run_script_method_program(&vm, &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(3)))
    );
}

#[test]
fn runs_script_method_self_record_compound_assignment() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Counter { counter: i64 }
impl Counter {
    fn inc(self) {
        self.counter += 1;
    }
}
fn main() {
    let counter = Counter { counter: 1 };
    counter.inc();
    return counter.counter;
}
"#,
    )
    .expect("compile self record compound assignment");

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn runs_compiled_script_impl_method_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> i64; }
struct Player { level: i64 }

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount;
    }
}

fn main() {
    let player = Player { level: 7 };
    return player.bonus(5);
}
"#,
    )
    .expect("compile script impl method dispatch");

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );
}

#[test]
fn runs_compiled_inherent_script_impl_method_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Player { level: i64 }

impl Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount;
    }
}

fn main() {
    let player = Player { level: 7 };
    return player.bonus(5);
}
"#,
    )
    .expect("compile inherent script impl method dispatch");

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );
}

#[test]
fn runs_compiled_script_method_named_and_default_args() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource {
    fn bonus(self, amount, multiplier = 2, offset = 1) -> i64;
}
struct Player { level: i64 }

impl BonusSource for Player {
    fn bonus(self, amount, multiplier = 2, offset = 1) -> i64 {
        return self.level + amount * multiplier + offset;
    }
}

fn main() {
    let player = Player { level: 7 };
    return player.bonus(offset = 4, amount = 5)
        + Player { level: 3 }.bonus(amount = 2);
}
"#,
    )
    .expect("compile script method named/default args");

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(29)))
    );
}

#[test]
fn runs_compiled_typed_parameter_method_id_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> i64; }
struct Player { level: i64 }

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount;
    }
}

fn main(player: Player) {
    return player.bonus(5);
}
"#,
    )
    .expect("compile typed parameter method id dispatch");
    let player = OwnedValue::Record {
        type_name: "Player".to_owned(),
        fields: ScriptFields::from_pairs(
            "Player",
            [(
                "level".to_owned(),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(7)),
            )],
        ),
    };

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "main", &[player]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );
}

#[test]
fn runs_compiled_immediate_script_method_id_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> i64; }
struct Player { level: i64 }

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount;
    }
}

fn main() {
    return Player { level: 7 }.bonus(5);
}
"#,
    )
    .expect("compile immediate script method id dispatch");

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );
}

#[test]
fn runs_compiled_trait_default_method_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource {
    fn bonus(self, amount) -> i64 { return self.level + amount; }
    fn label(self) -> String { return self.name; }
}
struct Player { level: i64, name: String }

impl BonusSource for Player {}

fn main() {
    let player = Player { level: 7, name: "hero" };
    if player.label() == "hero" {
        return player.bonus(5) + 4;
    }
    return 0;
}
"#,
    )
    .expect("compile trait default method dispatch");

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(16)))
    );
}

#[test]
fn runs_compiled_self_method_id_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource {
    fn label(self) -> String;
    fn summary(self) -> String { return self.label(); }
}
struct Player { name: String }

impl BonusSource for Player {
    fn label(self) -> String {
        return self.name;
    }
}

fn main() {
    return Player { name: "hero" }.summary();
}
"#,
    )
    .expect("compile self method id dispatch");

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "main", &[]),
        Ok(OwnedValue::String("hero".to_owned()))
    );
}

#[test]
fn runs_compiled_captured_receiver_method_id_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> i64; }
struct Player { level: i64 }

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount;
    }
}

fn main() {
    let player = Player { level: 7 };
    let bonus = |ignored| player.bonus(5);
    return bonus(null);
}
"#,
    )
    .expect("compile captured receiver method id dispatch");

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );
}

#[test]
fn runs_compiled_binding_pattern_receiver_method_id_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> i64; }
struct Player { level: i64 }

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount;
    }
}

fn main() {
    let player = Player { level: 7 };
    return match player {
        bound => bound.bonus(5),
    };
}
"#,
    )
    .expect("compile binding pattern receiver method id dispatch");

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );
}

#[test]
fn runs_compiled_host_ref_script_impl_method_dispatch() {
    let host_ref = player_ref(3);
    let program = compile_host_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> i64; }

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return reflect::get(self, "level") + amount;
    }
}

fn main(player: Player) {
    return player.bonus(5);
}
"#,
        host_ref_script_method_registry(host_ref.type_id, &["reflect::get"]),
    )
    .expect("compile host ref script impl method dispatch");
    let mut adapter = host_adapter(
        host_ref,
        HostValue::Scalar(vela_common::ScalarValue::I64(7)),
    );
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        run_script_method_program_with_host(
            &vm,
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );
}

#[test]
fn host_ref_script_impl_dispatch_uses_registered_type_registry() {
    let host_ref = player_ref(3);
    let program = compile_host_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> i64; }

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return amount + 7;
    }
}

fn main(player: Player) {
    return player.bonus(5);
}
"#,
        host_ref_script_method_registry(host_ref.type_id, &[]),
    )
    .expect("compile host ref script impl method dispatch");
    let mut adapter = host_adapter(
        host_ref,
        HostValue::Scalar(vela_common::ScalarValue::I64(7)),
    );
    let mut tx = HostAccess::new();
    let vm = Vm::new().with_type_registry(Arc::new(reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        run_script_method_program_with_host(
            &vm,
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );
}

#[test]
fn dynamic_host_method_dispatch_uses_registered_method_id_and_host_access() {
    let host_ref = player_ref(3);
    let method = HostMethodId::new(5);
    let program = compile_host_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    return player.grant_exp(20);
}
"#,
        host_definition_registry(
            &[("Player", host_ref.type_id)],
            &[],
            &[TestHostMethod::new(
                "Player",
                "grant_exp",
                method,
                &["amount"],
            )],
        ),
    )
    .expect("dynamic host method source should compile");
    let mut adapter = host_adapter(host_ref, HostValue::Null);
    adapter.insert_method_return(method, HostValue::Scalar(vela_common::ScalarValue::I64(1)));
    let mut tx = HostAccess::new();
    let vm = Vm::new().with_type_registry(Arc::new(reflection_registry()));

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        run_script_method_program_with_host(
            &vm,
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(OwnedValue::i64(1)));
    assert_eq!(
        adapter.method_calls(),
        &[(
            HostPath::new(host_ref),
            method,
            vec![HostValue::Scalar(vela_common::ScalarValue::I64(20))]
        )]
    );
}

#[test]
fn dynamic_host_method_cache_refreshes_when_schema_epoch_changes() {
    let host_ref = player_ref(3);
    let method = HostMethodId::new(5);
    let program = compile_host_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    return player.grant_exp(20);
}
"#,
        host_definition_registry(
            &[("Player", host_ref.type_id)],
            &[],
            &[TestHostMethod::new(
                "Player",
                "grant_exp",
                method,
                &["amount"],
            )],
        ),
    )
    .expect("dynamic host cache source should compile");
    let linked = link_test_program(&program);
    let site = linked_dynamic_method_site(&linked, "main");
    let caches = RecordingMethodCaches::new(linked_cache_len(&linked));
    let vm = Vm::new().with_type_registry(Arc::new(reflection_registry()));
    let mut budget = ExecutionBudget::unbounded();
    let mut adapter = host_adapter(host_ref, HostValue::Null);
    adapter.insert_method_return(method, HostValue::i64(1));
    adapter.set_schema_epoch(vela_host::resolved::HostSchemaEpoch::new(1));
    let mut tx = HostAccess::new();

    {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        assert_eq!(
            run_linked_test_entry_with_host_and_caches(
                &vm,
                &linked,
                "main",
                &[OwnedValue::HostRef(host_ref)],
                &mut host,
                &mut budget,
                &caches,
            ),
            Ok(OwnedValue::i64(1))
        );
    }
    assert!(matches!(
        caches.dynamic_entry(site).map(|entry| entry.receiver_guard),
        Some(DynamicReceiverGuard::HostType {
            type_id,
            schema_epoch,
        }) if type_id == host_ref.type_id
            && schema_epoch == vela_host::resolved::HostSchemaEpoch::new(1)
    ));

    adapter.insert_method_return(method, HostValue::i64(2));
    adapter.set_schema_epoch(vela_host::resolved::HostSchemaEpoch::new(2));
    {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        assert_eq!(
            run_linked_test_entry_with_host_and_caches(
                &vm,
                &linked,
                "main",
                &[OwnedValue::HostRef(host_ref)],
                &mut host,
                &mut budget,
                &caches,
            ),
            Ok(OwnedValue::i64(2))
        );
    }

    assert_eq!(caches.dynamic_set_count_for(site), 2);
    assert!(matches!(
        caches.dynamic_entry(site).map(|entry| entry.receiver_guard),
        Some(DynamicReceiverGuard::HostType {
            type_id,
            schema_epoch,
        }) if type_id == host_ref.type_id
            && schema_epoch == vela_host::resolved::HostSchemaEpoch::new(2)
    ));
}

#[test]
fn dynamic_host_method_missing_and_host_access_failures_keep_source_spans() {
    let host_ref = player_ref(3);
    let method = HostMethodId::new(5);
    let registry = host_definition_registry(
        &[("Player", host_ref.type_id)],
        &[],
        &[TestHostMethod::new(
            "Player",
            "grant_exp",
            method,
            &["amount"],
        )],
    );
    let missing_program = compile_host_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    return player.missing(20);
}
"#,
        registry.clone(),
    )
    .expect("missing dynamic host method source should compile");
    let vm = Vm::new().with_type_registry(Arc::new(reflection_registry()));
    let mut adapter = host_adapter(host_ref, HostValue::Null);
    let mut tx = HostAccess::new();
    let error = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        run_script_method_program_with_host(
            &vm,
            &missing_program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    }
    .expect_err("missing dynamic host method should fail");
    assert!(matches!(
        error.kind(),
        VmErrorKind::UnknownMethod { method } if method == "missing"
    ));
    assert!(error.source_span.is_some());

    let call_program = compile_host_program_source(
        SourceId::new(2),
        r#"
fn main(player) {
    return player.grant_exp(20);
}
"#,
        registry,
    )
    .expect("dynamic host method source should compile");
    let mut denied_adapter = host_adapter(host_ref, HostValue::Null);
    denied_adapter.deny_diagnostic_path_call(HostPath::new(host_ref));
    let mut tx = HostAccess::new();
    let error = {
        let mut host = HostExecution {
            adapter: &mut denied_adapter,
            access: &mut tx,
            script_globals: None,
        };
        run_script_method_program_with_host(
            &vm,
            &call_program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    }
    .expect_err("denied dynamic host method should fail through HostAccess");
    assert!(matches!(error.kind(), VmErrorKind::Host(_)));
    assert!(error.source_span.is_some());

    let stale_ref = player_ref(4);
    let mut stale_adapter = host_adapter(host_ref, HostValue::Null);
    let mut tx = HostAccess::new();
    let error = {
        let mut host = HostExecution {
            adapter: &mut stale_adapter,
            access: &mut tx,
            script_globals: None,
        };
        run_script_method_program_with_host(
            &vm,
            &call_program,
            "main",
            &[OwnedValue::HostRef(stale_ref)],
            &mut host,
        )
    }
    .expect_err("stale dynamic host method receiver should fail through HostAccess");
    assert!(matches!(error.kind(), VmErrorKind::Host(_)));
    assert!(error.source_span.is_some());
}

#[test]
fn runs_compiled_record_variant_field_method_id_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> i64; }
struct Player { level: i64 }

enum Event {
    Grant { player: Player },
    None,
}

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount;
    }
}

fn main() {
    let event = Event::Grant { player: Player { level: 7 } };
    return match event {
        Event::Grant { player } => player.bonus(5),
        _ => 0,
    };
}
"#,
    )
    .expect("compile record variant field method id dispatch");

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );
}

#[test]
fn runs_compiled_tuple_variant_field_method_id_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> i64; }
struct Player { level: i64 }

enum Event {
    Grant(player: Player),
    None,
}

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount;
    }
}

fn main() {
    let event = Event::Grant(Player { level: 7 });
    return match event {
        Event::Grant(player) => player.bonus(5),
        _ => 0,
    };
}
"#,
    )
    .expect("compile tuple variant field method id dispatch");

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );
}

#[test]
fn explicit_impl_method_overrides_trait_default_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource {
    fn bonus(self, amount) -> i64 { return self.level + amount; }
}
struct Player { level: i64 }

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return amount * 2;
    }
}

fn main() {
    let player = Player { level: 7 };
    return player.bonus(5);
}
"#,
    )
    .expect("compile explicit impl method override");

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(10)))
    );
}

#[test]
fn runs_compiled_module_qualified_script_impl_method_dispatch() {
    let program = compile_module_sources(&[ModuleSource::new(
        SourceId::new(1),
        ModulePath::from_qualified("game::combat"),
        r#"
trait BonusSource { fn bonus(self, amount) -> i64; }
struct Player { level: i64 }

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount;
    }
}

pub fn main() {
    let player = Player { level: 10 };
    return player.bonus(4);
}
"#,
    )])
    .expect("compile module-qualified script impl method dispatch");

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "game::combat::main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(14)))
    );
}

#[test]
fn runs_compiled_module_typed_parameter_method_id_dispatch() {
    let program = compile_module_sources(&[
        ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_qualified("game::model"),
            r#"
pub trait BonusSource { fn bonus(self, amount) -> i64; }
pub struct Player { level: i64 }

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount;
    }
}
"#,
        ),
        ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_qualified("game::combat"),
            r#"
use game::model::Player

pub fn main(player: Player) {
    return player.bonus(5);
}
"#,
        ),
    ])
    .expect("compile module typed parameter method id dispatch");
    let player = OwnedValue::Record {
        type_name: "game::model::Player".to_owned(),
        fields: ScriptFields::from_pairs(
            "game::model::Player",
            [(
                "level".to_owned(),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(7)),
            )],
        ),
    };

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "game::combat::main", &[player]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );
}
