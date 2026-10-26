use super::*;
use crate::owned_value::OwnedValue;

fn compile_reflection_member_source(
    source: SourceId,
    text: &str,
) -> vela_bytecode::compiler::error::CompileResult<vela_bytecode::compiler::CompiledProgram> {
    compile_standard_program_source_with_native_functions(
        source,
        text,
        &[
            "reflect::access",
            "reflect::field",
            "reflect::fields",
            "reflect::get",
            "reflect::has_field",
            "reflect::has_method",
            "reflect::id",
            "reflect::implements",
            "reflect::kind",
            "reflect::method",
            "reflect::methods",
            "reflect::name",
            "reflect::owner",
            "reflect::trait_info",
            "reflect::type_info",
            "reflect::variant_info",
            "reflect::variant_is",
            "reflect::variants",
        ],
    )
}

fn exec_reflection_member_program(
    vm: &Vm,
    program: &UnlinkedProgram,
    args: &[OwnedValue],
    host: &mut HostExecution<'_>,
) -> VmResult<OwnedValue> {
    let mut budget = ExecutionBudget::unbounded();
    run_linked_test_program_with_host_budget(vm, program, "main", args, host, &mut budget)
}

#[test]
fn compiled_source_reflect_type_reports_unknown_type_candidates() {
    let program = compile_reflection_member_source(
        SourceId::new(1),
        r#"
fn main() {
    return reflect::type_info("Plyer");
}
"#,
    )
    .expect("compile unknown type metadata source");

    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    let mut host = HostExecution { state_values: None };

    assert!(matches!(
        exec_reflection_member_program(&vm, &program, &[], &mut host),
        Err(error) if error.kind() == VmErrorKind::Reflect(ReflectErrorKind::UnknownTypeName {
            type_name: "Plyer".to_owned(),
            candidates: vec!["Player".to_owned()],
            related: vec![ReflectCandidate::new("Player", None)],
        })
    ));
}

#[test]
fn compiled_source_reflect_trait_reports_unknown_trait_candidates() {
    let program = compile_reflection_member_source(
        SourceId::new(1),
        r#"
fn main() {
    return reflect::trait_info("Damagable");
}
"#,
    )
    .expect("compile unknown trait metadata source");

    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    let mut host = HostExecution { state_values: None };

    assert!(matches!(
        exec_reflection_member_program(&vm, &program, &[], &mut host),
        Err(error) if error.kind() == VmErrorKind::Reflect(ReflectErrorKind::UnknownTrait {
            trait_name: "Damagable".to_owned(),
            candidates: vec!["Damageable".to_owned()],
            related: vec![ReflectCandidate::new("Damageable", None)],
        })
    ));
}

#[test]
fn compiled_source_reflect_variants_respect_field_access() {
    let program = compile_reflection_member_source(
        SourceId::new(1),
        include_str!("../../tests/fixtures/reflection/variant_access.vela"),
    )
    .expect("compile policy variant reflection source");

    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(member_reflection_registry()));
    let mut host = HostExecution { state_values: None };

    assert_eq!(
        exec_reflection_member_program(&vm, &program, &[], &mut host),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(22)))
    );
}

#[test]
fn compiled_source_reflect_field_denies_hidden_variant_fields() {
    let program = compile_reflection_member_source(
        SourceId::new(1),
        r#"
fn main() {
    let quest = QuestProgress::Active { count: 1 };
    return reflect::field(quest, "secret");
}
"#,
    )
    .expect("compile hidden variant field reflection source");

    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(member_reflection_registry()));
    let mut host = HostExecution { state_values: None };

    assert!(matches!(
        exec_reflection_member_program(&vm, &program, &[], &mut host),
        Err(error) if error.kind() == VmErrorKind::Reflect(ReflectErrorKind::FieldNotReflectReadable {
            type_name: "QuestProgress::Active".to_owned(),
            field: "secret".to_owned(),
            source_span: None,
        })
    ));
}

#[test]
fn compiled_source_reflect_variants_respect_field_permissions() {
    let program = compile_reflection_member_source(
        SourceId::new(1),
        include_str!("../../tests/fixtures/reflection/variant_permissions_denied.vela"),
    )
    .expect("compile denied variant field permission source");

    let mut vm = Vm::new();
    vm.register_reflection_natives_with_policy(
        Arc::new(policy_variant_field_reflection_registry()),
        reflect::permissions::ReflectPolicy::read_only(),
    );
    let mut host = HostExecution { state_values: None };

    assert_eq!(
        exec_reflection_member_program(&vm, &program, &[], &mut host),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn compiled_source_reflect_variants_expose_granted_field_permissions() {
    let program = compile_reflection_member_source(
        SourceId::new(1),
        include_str!("../../tests/fixtures/reflection/variant_permissions_granted.vela"),
    )
    .expect("compile granted variant field permission source");

    let mut vm = Vm::new();
    vm.register_reflection_natives_with_policy(
        Arc::new(policy_variant_field_reflection_registry()),
        reflect::permissions::ReflectPolicy::read_only()
            .with_field_permission("quest.admin.inspect"),
    );
    let mut host = HostExecution { state_values: None };

    assert_eq!(
        exec_reflection_member_program(&vm, &program, &[], &mut host),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn compiled_source_reflect_methods_respect_method_policy() {
    let program = compile_reflection_member_source(
        SourceId::new(1),
        include_str!("../../tests/fixtures/reflection/method_policy.vela"),
    )
    .expect("compile policy methods reflection source");

    let mut vm = Vm::new();
    let policy = reflect::permissions::ReflectPolicy::new(
        reflect::permissions::ReflectPermissionSet::new()
            .with(reflect::permissions::ReflectPermission::ReadTypeInfo)
            .with(reflect::permissions::ReflectPermission::ReadTypeInfo),
    );
    vm.register_reflection_natives_with_policy(
        Arc::new(policy_method_reflection_registry()),
        policy,
    );
    let mut host = HostExecution { state_values: None };

    assert_eq!(
        exec_reflection_member_program(
            &vm,
            &program,
            &[OwnedValue::record(
                "Player",
                [("level", OwnedValue::i64(9))]
            )],
            &mut host
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(11)))
    );
}

#[test]
fn compiled_source_reflect_method_reports_unknown_method_candidates() {
    let program = compile_reflection_member_source(
        SourceId::new(1),
        r#"
fn main(player) {
    return reflect::method(player, "grant_xp");
}
"#,
    )
    .expect("compile unknown method reflection source");

    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(member_reflection_registry()));
    let mut host = HostExecution { state_values: None };

    assert!(matches!(
        exec_reflection_member_program(&vm, &program, &[OwnedValue::record("Player", [("level", OwnedValue::i64(9))])], &mut host),
        Err(error) if error.kind() == VmErrorKind::Reflect(ReflectErrorKind::UnknownMethod {
            type_name: "Player".to_owned(),
            method: "grant_xp".to_owned(),
            candidates: vec!["grant_exp".to_owned()],
            related: vec![ReflectCandidate::new("grant_exp", None)],
        })
    ));
}

#[test]
fn compiled_source_reflect_variant_is_reports_unknown_variant_candidates() {
    let program = compile_reflection_member_source(
        SourceId::new(1),
        r#"
fn main() {
    let quest = QuestProgress::Active { count: 1 };
    return reflect::variant_is(quest, "Actve");
}
"#,
    )
    .expect("compile unknown variant reflection source");

    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(member_reflection_registry()));
    let mut host = HostExecution { state_values: None };

    assert!(matches!(
        exec_reflection_member_program(&vm, &program, &[], &mut host),
        Err(error) if error.kind() == VmErrorKind::Reflect(ReflectErrorKind::UnknownVariant {
            type_name: "QuestProgress".to_owned(),
            variant: "Actve".to_owned(),
            candidates: vec!["Active".to_owned(), "Finished".to_owned()],
            related: vec![
                ReflectCandidate::new("Active", None),
                ReflectCandidate::new("Finished", None),
            ],
        })
    ));
}

#[test]
fn compiled_source_reflect_variant_info_reports_unknown_variant_candidates() {
    let program = compile_reflection_member_source(
        SourceId::new(1),
        r#"
fn main() {
    let quest = QuestProgress::Active { count: 1 };
    return reflect::variant_info(quest, "Actve");
}
"#,
    )
    .expect("compile unknown variant info reflection source");

    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(member_reflection_registry()));
    let mut host = HostExecution { state_values: None };

    assert!(matches!(
        exec_reflection_member_program(&vm, &program, &[], &mut host),
        Err(error) if error.kind() == VmErrorKind::Reflect(ReflectErrorKind::UnknownVariant {
            type_name: "QuestProgress".to_owned(),
            variant: "Actve".to_owned(),
            candidates: vec!["Active".to_owned(), "Finished".to_owned()],
            related: vec![
                ReflectCandidate::new("Active", None),
                ReflectCandidate::new("Finished", None),
            ],
        })
    ));
}

#[test]
fn compiled_source_reflect_implements_reports_unknown_trait_candidates() {
    let program = compile_reflection_member_source(
        SourceId::new(1),
        r#"
fn main(player) {
    return reflect::implements(player, "Damagable");
}
"#,
    )
    .expect("compile unknown trait reflection source");

    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    let mut host = HostExecution { state_values: None };

    assert!(matches!(
        exec_reflection_member_program(&vm, &program, &[OwnedValue::record("Player", [("level", OwnedValue::i64(9))])], &mut host),
        Err(error) if error.kind() == VmErrorKind::Reflect(ReflectErrorKind::UnknownTrait {
            trait_name: "Damagable".to_owned(),
            candidates: vec!["Damageable".to_owned()],
            related: vec![ReflectCandidate::new("Damageable", None)],
        })
    ));
}

#[test]
fn compiled_source_reflect_implements_accepts_type_descriptor() {
    let program = compile_reflection_member_source(
        SourceId::new(1),
        r#"
fn main() {
    let player_type = reflect::type_info("Player");
    let damageable = reflect::trait_info("Damageable");
    if reflect::kind(player_type) == "script_struct" && reflect::implements(player_type, damageable) {
        return reflect::id(player_type);
    }
    return 0;
}
"#,
    )
    .expect("compile type descriptor implements source");

    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    let mut host = HostExecution { state_values: None };

    assert_eq!(
        exec_reflection_member_program(&vm, &program, &[], &mut host),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(100)))
    );
}
