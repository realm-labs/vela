use super::*;
use crate::owned_value::OwnedValue;

fn compile_reflection_permission_source(
    source: SourceId,
    text: &str,
) -> vela_bytecode::compiler::error::CompileResult<vela_bytecode::compiler::CompiledProgram> {
    compile_standard_program_source_with_native_functions(
        source,
        text,
        &[
            "reflect::call",
            "reflect::function",
            "reflect::get",
            "reflect::has_permission",
            "reflect::implements",
            "reflect::kind",
            "reflect::method",
            "reflect::name",
            "reflect::permissions",
            "reflect::set",
            "reflect::traits",
            "reflect::type_of",
        ],
    )
}

fn exec_reflection_permission_program(
    vm: &Vm,
    program: &UnlinkedProgram,
    args: &[OwnedValue],
    host: &mut HostExecution<'_>,
) -> VmResult<OwnedValue> {
    let mut budget = ExecutionBudget::unbounded();
    run_linked_test_program_with_host_budget(vm, program, "main", args, host, &mut budget)
}

#[test]
fn compiled_source_uses_reflection_natives_for_script_values() {
    let program = compile_reflection_permission_source(
        SourceId::new(1),
        include_str!("../../tests/fixtures/reflection/script_value_natives.vela"),
    )
    .expect("compile reflection source");

    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));

    let result = {
        let mut host = HostExecution { state_values: None };
        exec_reflection_permission_program(
            &vm,
            &program,
            &[OwnedValue::record(
                "Player",
                [
                    ("level", OwnedValue::i64(9)),
                    ("secret", OwnedValue::i64(99)),
                    ("title", OwnedValue::from("Keeper")),
                ],
            )],
            &mut host,
        )
    };

    assert_eq!(
        result,
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(10)))
    );
}

#[test]
fn reflection_permissions_deny_writes_before_value_access() {
    let program = compile_reflection_permission_source(
        SourceId::new(1),
        r#"
fn main(player) {
    reflect::set(player, "level", 10);
    return 1;
}
"#,
    )
    .expect("compile denied reflection write source");

    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(reflection_registry()),
        reflect::permissions::ReflectPermissionSet::read_only(),
    );
    let mut host = HostExecution { state_values: None };

    assert!(matches!(
        exec_reflection_permission_program(
            &vm,
            &program,
            &[OwnedValue::record("Player", [("level", OwnedValue::i64(9)), ("secret", OwnedValue::i64(99)), ("title", OwnedValue::from("Keeper"))])],
            &mut host
        ),
        Err(error) if error.kind() == VmErrorKind::Reflect(ReflectErrorKind::PermissionDenied {
            permission: reflect::permissions::ReflectPermission::WriteValueFields
        })
    ));
}

#[test]
fn reflection_permissions_deny_calls_before_value_access() {
    let program = compile_reflection_permission_source(
        SourceId::new(1),
        r#"
fn main(player) {
    reflect::call(player, "grant_exp", 10);
    return 1;
}
"#,
    )
    .expect("compile denied reflection call source");

    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(reflection_registry()),
        reflect::permissions::ReflectPermissionSet::read_only(),
    );
    let mut host = HostExecution { state_values: None };

    assert!(matches!(
        exec_reflection_permission_program(
            &vm,
            &program,
            &[OwnedValue::record("Player", [("level", OwnedValue::i64(9)), ("secret", OwnedValue::i64(99)), ("title", OwnedValue::from("Keeper"))])],
            &mut host
        ),
        Err(error) if error.kind() == VmErrorKind::Reflect(ReflectErrorKind::PermissionDenied {
            permission: reflect::permissions::ReflectPermission::CallMethods
        })
    ));
}

#[test]
fn reflection_permissions_allow_script_metadata_without_host_inspection() {
    let program = compile_reflection_permission_source(
        SourceId::new(1),
        r#"
struct Player { level: i64 }

fn main() {
    let player = Player { level: 7 };
    return reflect::name(player);
}
"#,
    )
    .expect("compile script metadata source");

    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(script_reflection_registry()),
        reflect::permissions::ReflectPermissionSet::new()
            .with(reflect::permissions::ReflectPermission::ReadTypeInfo),
    );
    let mut host = HostExecution { state_values: None };

    assert_eq!(
        exec_reflection_permission_program(&vm, &program, &[], &mut host),
        Ok(OwnedValue::String("Player".into()))
    );
}

#[test]
fn reflection_permissions_report_active_policy_metadata() {
    let program = compile_reflection_permission_source(
        SourceId::new(1),
        r#"
fn main() {
    if !reflect::has_permission("reflect::read_type_info") {
        return 0;
    }
    if reflect::has_permission("reflect::write_value_fields") {
        return 0;
    }
    return reflect::permissions();
}
"#,
    )
    .expect("compile reflection permission metadata source");

    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(TypeRegistry::new()),
        reflect::permissions::ReflectPermissionSet::read_only()
            .with(reflect::permissions::ReflectPermission::ReadTypeInfo),
    );
    let mut host = HostExecution { state_values: None };

    assert_eq!(
        exec_reflection_permission_program(&vm, &program, &[], &mut host),
        Ok(OwnedValue::Array(vec![
            OwnedValue::String("reflect::read_type_info".to_owned()),
            OwnedValue::String("reflect::read_value_fields".to_owned()),
        ]))
    );
}

#[test]
fn reflection_permissions_report_unknown_permission_candidates() {
    let program = compile_reflection_permission_source(
        SourceId::new(1),
        r#"
fn main() {
    return reflect::has_permission("reflect::inspect_host");
}
"#,
    )
    .expect("compile reflection unknown permission source");

    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(TypeRegistry::new()),
        reflect::permissions::ReflectPermissionSet::read_only(),
    );
    let mut host = HostExecution { state_values: None };

    let error = exec_reflection_permission_program(&vm, &program, &[], &mut host)
        .expect_err("unknown permission should diagnose");
    assert_eq!(
        error.kind(),
        VmErrorKind::Reflect(ReflectErrorKind::UnknownPermission {
            permission: "reflect::inspect_host".to_owned(),
            candidates: vec![
                "reflect::call_methods".to_owned(),
                "reflect::access_private".to_owned(),
                "reflect::call_event_methods".to_owned()
            ]
        })
    );
}

#[test]
fn reflection_permissions_deny_permission_metadata_without_type_read() {
    let program = compile_reflection_permission_source(
        SourceId::new(1),
        r#"
fn main() {
    return reflect::permissions();
}
"#,
    )
    .expect("compile denied reflection permission metadata source");

    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(TypeRegistry::new()),
        reflect::permissions::ReflectPermissionSet::new(),
    );
    let mut host = HostExecution { state_values: None };

    assert!(matches!(
        exec_reflection_permission_program(&vm, &program, &[], &mut host),
        Err(error) if error.kind() == VmErrorKind::Reflect(ReflectErrorKind::PermissionDenied {
            permission: reflect::permissions::ReflectPermission::ReadTypeInfo
        })
    ));
}

#[test]
fn reflection_permissions_deny_function_metadata_without_function_permission() {
    let program = compile_reflection_permission_source(
        SourceId::new(1),
        r#"
fn main() {
    reflect::function("game::admin");
    return 1;
}
"#,
    )
    .expect("compile function metadata permission source");
    let mut registry = TypeRegistry::new();
    registry.register_function(
        FunctionDesc::new(FunctionId::new(9), "game::admin")
            .access(FunctionAccess::new().require_permission("game::admin")),
    );

    let mut vm = Vm::new();
    vm.register_reflection_natives_with_policy(
        Arc::new(registry),
        reflect::permissions::ReflectPolicy::new(
            reflect::permissions::ReflectPermissionSet::new()
                .with(reflect::permissions::ReflectPermission::ReadTypeInfo),
        ),
    );
    let mut host = HostExecution { state_values: None };

    let error = exec_reflection_permission_program(&vm, &program, &[], &mut host)
        .expect_err("function metadata permission should be denied");
    assert_eq!(
        error.kind(),
        VmErrorKind::Reflect(ReflectErrorKind::FunctionPermissionDenied {
            function: "game::admin".to_owned(),
            permission: "game::admin".to_owned(),
            source_span: None,
        })
    );
}

#[test]
fn reflection_field_access_denies_hidden_script_field_reads() {
    let secret_field = FieldId::new(77);
    let program = compile_reflection_permission_source(
        SourceId::new(1),
        r#"
fn main(player) {
    return reflect::get(player, "secret");
}
"#,
    )
    .expect("compile hidden field reflection source");
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .kind(TypeKind::ScriptStruct)
            .field(
                FieldDesc::new(secret_field, "secret")
                    .access(FieldAccess::new().reflect_readable(false)),
            ),
    );

    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(registry));
    let mut host = HostExecution { state_values: None };

    let error = exec_reflection_permission_program(
        &vm,
        &program,
        &[OwnedValue::record(
            "Player",
            [
                ("level", OwnedValue::i64(9)),
                ("secret", OwnedValue::i64(99)),
                ("title", OwnedValue::from("Keeper")),
            ],
        )],
        &mut host,
    )
    .expect_err("hidden field read should be denied");
    assert_eq!(
        error.kind(),
        VmErrorKind::Reflect(ReflectErrorKind::FieldNotReflectReadable {
            type_name: "Player".to_owned(),
            field: "secret".to_owned(),
            source_span: None,
        })
    );
}

#[test]
fn reflection_field_permissions_deny_script_field_reads_before_value_access() {
    let title_field = FieldId::new(78);
    let program = compile_reflection_permission_source(
        SourceId::new(1),
        r#"
fn main(player) {
    return reflect::get(player, "title");
}
"#,
    )
    .expect("compile field permission reflection source");
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .kind(TypeKind::ScriptStruct)
            .field(
                FieldDesc::new(title_field, "title")
                    .access(FieldAccess::new().require_permission("player.title.inspect")),
            ),
    );

    let mut vm = Vm::new();
    vm.register_reflection_natives_with_policy(
        Arc::new(registry),
        reflect::permissions::ReflectPolicy::all(),
    );
    let mut host = HostExecution { state_values: None };

    let error = exec_reflection_permission_program(
        &vm,
        &program,
        &[OwnedValue::record(
            "Player",
            [
                ("level", OwnedValue::i64(9)),
                ("secret", OwnedValue::i64(99)),
                ("title", OwnedValue::from("Keeper")),
            ],
        )],
        &mut host,
    )
    .expect_err("field permission should be denied");
    assert_eq!(
        error.kind(),
        VmErrorKind::Reflect(ReflectErrorKind::FieldPermissionDenied {
            type_name: "Player".to_owned(),
            field: "title".to_owned(),
            permission: "player.title.inspect".to_owned(),
            source_span: None,
        })
    );
}

#[test]
fn reflection_unknown_script_field_candidates_respect_read_policy() {
    let program = compile_reflection_permission_source(
        SourceId::new(1),
        r#"
fn main(player) {
    return reflect::get(player, "leve");
}
"#,
    )
    .expect("compile unknown field candidate policy source");
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .kind(TypeKind::ScriptStruct)
            .field(FieldDesc::new(FieldId::new(1), "level"))
            .field(
                FieldDesc::new(FieldId::new(2), "lever")
                    .access(FieldAccess::new().reflect_readable(false)),
            )
            .field(
                FieldDesc::new(FieldId::new(3), "leves")
                    .access(FieldAccess::new().require_permission("player.admin.inspect")),
            ),
    );

    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(registry),
        reflect::permissions::ReflectPermissionSet::read_only()
            .with(reflect::permissions::ReflectPermission::ReadTypeInfo),
    );
    let mut host = HostExecution { state_values: None };

    let error = exec_reflection_permission_program(
        &vm,
        &program,
        &[OwnedValue::record(
            "Player",
            [
                ("level", OwnedValue::i64(9)),
                ("secret", OwnedValue::i64(99)),
                ("title", OwnedValue::from("Keeper")),
            ],
        )],
        &mut host,
    )
    .expect_err("unknown field should diagnose allowed candidates only");
    assert_eq!(
        error.kind(),
        VmErrorKind::Reflect(ReflectErrorKind::UnknownField {
            type_name: "Player".to_owned(),
            field: "leve".to_owned(),
            candidates: vec!["level".to_owned()],
            related: vec![ReflectCandidate::new("level", None)],
        })
    );
}

#[test]
fn reflection_unknown_script_method_candidates_respect_call_policy() {
    let program = compile_reflection_permission_source(
        SourceId::new(1),
        r#"
fn main(player) {
    return reflect::method(player, "visibl");
}
"#,
    )
    .expect("compile unknown method candidate policy source");
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .kind(TypeKind::ScriptStruct)
            .method(MethodDesc::new(MethodId::new(1), "visible"))
            .method(
                MethodDesc::new(MethodId::new(2), "visibly_hidden")
                    .access(MethodAccess::new().reflect_callable(false)),
            )
            .method(
                MethodDesc::new(MethodId::new(3), "visibly_private")
                    .access(MethodAccess::new().public(false).reflect_callable(true)),
            )
            .method(
                MethodDesc::new(MethodId::new(4), "visibly_admin")
                    .access(MethodAccess::new().require_permission("player.admin.call")),
            ),
    );

    let mut vm = Vm::new();
    vm.register_reflection_natives_with_policy(
        Arc::new(registry),
        reflect::permissions::ReflectPolicy::new(
            reflect::permissions::ReflectPermissionSet::new()
                .with(reflect::permissions::ReflectPermission::CallMethods)
                .with(reflect::permissions::ReflectPermission::ReadTypeInfo),
        ),
    );
    let mut host = HostExecution { state_values: None };

    let error = exec_reflection_permission_program(
        &vm,
        &program,
        &[OwnedValue::record(
            "Player",
            [
                ("level", OwnedValue::i64(9)),
                ("secret", OwnedValue::i64(99)),
                ("title", OwnedValue::from("Keeper")),
            ],
        )],
        &mut host,
    )
    .expect_err("unknown method should diagnose allowed candidates only");
    assert_eq!(
        error.kind(),
        VmErrorKind::Reflect(ReflectErrorKind::UnknownMethod {
            type_name: "Player".to_owned(),
            method: "visibl".to_owned(),
            candidates: vec!["visible".to_owned()],
            related: vec![ReflectCandidate::new("visible", None)],
        })
    );
}

#[test]
fn reflection_lookup_budget_stops_after_limit() {
    let program = compile_reflection_permission_source(
        SourceId::new(1),
        r#"
fn main(player) {
    reflect::name(player);
    reflect::kind(player);
    return 1;
}
"#,
    )
    .expect("compile budgeted reflection source");

    let mut vm = Vm::new();
    vm.register_reflection_natives_with_policy(
        Arc::new(reflection_registry()),
        reflect::permissions::ReflectPolicy::all().with_lookup_limit(1),
    );
    let mut host = HostExecution { state_values: None };

    assert!(matches!(
        exec_reflection_permission_program(
            &vm,
            &program,
            &[OwnedValue::record("Player", [("level", OwnedValue::i64(9)), ("secret", OwnedValue::i64(99)), ("title", OwnedValue::from("Keeper"))])],
            &mut host
        ),
        Err(error) if error.kind() == VmErrorKind::Reflect(ReflectErrorKind::LookupBudgetExceeded {
            limit: 1
        })
    ));
}
