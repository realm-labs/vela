use super::*;

#[test]
fn compiled_source_uses_reflection_natives_for_host_state() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    let player_type = reflect.type_of(player);
    if reflect.name(player_type) == "Player" && reflect.kind(player_type) == "host" {
        if reflect.implements(player, "Damageable") {
            reflect.set(player, "level", 10);
            return reflect.get(player, "level");
        }
    }
    return 0;
}
"#,
    )
    .expect("compile reflection source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
    };

    assert_eq!(result, Ok(Value::Int(10)));
    assert_eq!(
        adapter.read_path(&level_path(host_ref)),
        Ok(HostValue::Int(9))
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(10)));
    tx.apply(&mut adapter).expect("apply reflection patch");
    assert_eq!(
        adapter.read_path(&level_path(host_ref)),
        Ok(HostValue::Int(10))
    );
}

#[test]
fn reflection_permissions_deny_writes_before_patches() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    reflect.set(player, "level", 10);
    return 1;
}
"#,
    )
    .expect("compile denied reflection write source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(reflection_registry()),
        reflect::permissions::ReflectPermissionSet::read_only(),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
        Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::PermissionDenied {
            permission: reflect::permissions::ReflectPermission::WriteValueFields
        })
    ));
    assert!(tx.patches().is_empty());
}

#[test]
fn reflection_permissions_deny_calls_before_patches() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    reflect.call(player, "grant_exp", 10);
    return 1;
}
"#,
    )
    .expect("compile denied reflection call source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    adapter.insert_method_return(HostMethodId::new(5), HostValue::Null);
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(reflection_registry()),
        reflect::permissions::ReflectPermissionSet::read_only(),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
        Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::PermissionDenied {
            permission: reflect::permissions::ReflectPermission::CallMethods
        })
    ));
    assert!(tx.patches().is_empty());
}

#[test]
fn reflection_permissions_deny_host_write_effect_calls_before_patches() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    reflect.call(player, "grant_exp", 10);
    return 1;
}
"#,
    )
    .expect("compile denied reflection effect source");
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .host_type(HostTypeId::new(1))
            .method(
                MethodDesc::new(HostMethodId::new(5), "grant_exp")
                    .effects(MethodEffectSet::host_write())
                    .access(MethodAccess::new().reflect_callable(true)),
            ),
    );
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_policy(
        Arc::new(registry),
        reflect::permissions::ReflectPolicy::new(
            reflect::permissions::ReflectPermissionSet::new()
                .with(reflect::permissions::ReflectPermission::CallMethods)
                .with(reflect::permissions::ReflectPermission::CallHostReadMethods)
                .with(reflect::permissions::ReflectPermission::InspectHostPath),
        ),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
        Err(error) if error.kind == VmErrorKind::Reflect(
            ReflectErrorKind::MethodEffectPermissionDenied {
                method: "grant_exp".to_owned(),
                permission: reflect::permissions::ReflectPermission::CallHostWriteMethods
            }
        )
    ));
    assert!(tx.patches().is_empty());
}

#[test]
fn reflection_permissions_deny_host_ref_metadata_without_inspection() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    return reflect.type_of(player);
}
"#,
    )
    .expect("compile denied host-ref metadata source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(reflection_registry()),
        reflect::permissions::ReflectPermissionSet::new()
            .with(reflect::permissions::ReflectPermission::ReadTypeInfo),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
        Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::PermissionDenied {
            permission: reflect::permissions::ReflectPermission::InspectHostPath
        })
    ));
    assert!(tx.patches().is_empty());
}

#[test]
fn reflection_permissions_deny_host_ref_trait_metadata_without_inspection() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    return reflect.traits(player);
}
"#,
    )
    .expect("compile denied host-ref trait metadata source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(reflection_registry()),
        reflect::permissions::ReflectPermissionSet::new()
            .with(reflect::permissions::ReflectPermission::ReadTypeInfo),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
        Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::PermissionDenied {
            permission: reflect::permissions::ReflectPermission::InspectHostPath
        })
    ));
    assert!(tx.patches().is_empty());
}

#[test]
fn reflection_permissions_deny_host_ref_implements_without_inspection() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    return reflect.implements(player, "Damageable");
}
"#,
    )
    .expect("compile denied host-ref implements source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(reflection_registry()),
        reflect::permissions::ReflectPermissionSet::new()
            .with(reflect::permissions::ReflectPermission::ReadTypeInfo),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
        Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::PermissionDenied {
            permission: reflect::permissions::ReflectPermission::InspectHostPath
        })
    ));
    assert!(tx.patches().is_empty());
}

#[test]
fn reflection_permissions_allow_script_metadata_without_host_inspection() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Player { level: int }

fn main() {
    let player = Player { level: 7 };
    return reflect.name(player);
}
"#,
    )
    .expect("compile script metadata source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(script_reflection_registry()),
        reflect::permissions::ReflectPermissionSet::new()
            .with(reflect::permissions::ReflectPermission::ReadTypeInfo),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        vm.run_program_with_host(&program, "main", &[], &mut host),
        Ok(Value::String("Player".into()))
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn reflection_permissions_report_active_policy_metadata() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    if !reflect.has_permission("reflect.inspect_host_path") {
        return 0;
    }
    if reflect.has_permission("reflect.write_value_fields") {
        return 0;
    }
    return reflect.permissions();
}
"#,
    )
    .expect("compile reflection permission metadata source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(TypeRegistry::new()),
        reflect::permissions::ReflectPermissionSet::read_only()
            .with(reflect::permissions::ReflectPermission::InspectHostPath),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        vm.run_program_with_host(&program, "main", &[], &mut host),
        Ok(Value::Array(vec![
            Value::String("reflect.read_type_info".to_owned()),
            Value::String("reflect.read_value_fields".to_owned()),
            Value::String("reflect.inspect_host_path".to_owned()),
        ]))
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn reflection_permissions_report_unknown_permission_candidates() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return reflect.has_permission("reflect.inspect_host");
}
"#,
    )
    .expect("compile reflection unknown permission source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(TypeRegistry::new()),
        reflect::permissions::ReflectPermissionSet::read_only(),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    let error = vm
        .run_program_with_host(&program, "main", &[], &mut host)
        .expect_err("unknown permission should diagnose");
    assert_eq!(
        error.kind,
        VmErrorKind::Reflect(ReflectErrorKind::UnknownPermission {
            permission: "reflect.inspect_host".to_owned(),
            candidates: vec![
                "reflect.inspect_host_path".to_owned(),
                "reflect.call_methods".to_owned(),
                "reflect.access_private".to_owned()
            ]
        })
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn reflection_permissions_deny_permission_metadata_without_type_read() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return reflect.permissions();
}
"#,
    )
    .expect("compile denied reflection permission metadata source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(TypeRegistry::new()),
        reflect::permissions::ReflectPermissionSet::new(),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        vm.run_program_with_host(&program, "main", &[], &mut host),
        Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::PermissionDenied {
            permission: reflect::permissions::ReflectPermission::ReadTypeInfo
        })
    ));
    assert!(tx.patches().is_empty());
}

#[test]
fn reflection_permissions_deny_function_metadata_without_function_permission() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    reflect.function("game.admin");
    return 1;
}
"#,
    )
    .expect("compile function metadata permission source");
    let mut registry = TypeRegistry::new();
    registry.register_function(
        FunctionDesc::new(FunctionId::new(9), "game.admin")
            .access(FunctionAccess::new().require_permission("game.admin")),
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_policy(
        Arc::new(registry),
        reflect::permissions::ReflectPolicy::new(
            reflect::permissions::ReflectPermissionSet::new()
                .with(reflect::permissions::ReflectPermission::ReadTypeInfo),
        ),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    let error = vm
        .run_program_with_host(&program, "main", &[], &mut host)
        .expect_err("function metadata permission should be denied");
    assert_eq!(
        error.kind,
        VmErrorKind::Reflect(ReflectErrorKind::FunctionPermissionDenied {
            function: "game.admin".to_owned(),
            permission: "game.admin".to_owned(),
        })
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn reflection_field_access_denies_hidden_host_field_reads() {
    let host_ref = player_ref(3);
    let secret_field = FieldId::new(77);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    return reflect.get(player, "secret");
}
"#,
    )
    .expect("compile hidden field reflection source");
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .host_type(HostTypeId::new(1))
            .field(
                FieldDesc::new(secret_field, "secret")
                    .access(FieldAccess::new().reflect_readable(false)),
            ),
    );
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(
        HostPath::new(host_ref).field(secret_field),
        HostValue::Int(99),
    );
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(registry));
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    let error = vm
        .run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
        .expect_err("hidden field read should be denied");
    assert_eq!(
        error.kind,
        VmErrorKind::Reflect(ReflectErrorKind::FieldNotReflectReadable {
            type_name: "Player".to_owned(),
            field: "secret".to_owned(),
        })
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn reflection_field_permissions_deny_host_field_reads_before_patch() {
    let host_ref = player_ref(3);
    let title_field = FieldId::new(78);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    return reflect.get(player, "title");
}
"#,
    )
    .expect("compile field permission reflection source");
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .host_type(HostTypeId::new(1))
            .field(
                FieldDesc::new(title_field, "title")
                    .access(FieldAccess::new().require_permission("player.title.inspect")),
            ),
    );
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(
        HostPath::new(host_ref).field(title_field),
        HostValue::String("Knight".to_owned()),
    );
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_policy(
        Arc::new(registry),
        reflect::permissions::ReflectPolicy::all(),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    let error = vm
        .run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
        .expect_err("field permission should be denied");
    assert_eq!(
        error.kind,
        VmErrorKind::Reflect(ReflectErrorKind::FieldPermissionDenied {
            type_name: "Player".to_owned(),
            field: "title".to_owned(),
            permission: "player.title.inspect".to_owned(),
        })
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn reflection_unknown_host_field_candidates_respect_read_policy() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    return reflect.get(player, "leve");
}
"#,
    )
    .expect("compile unknown field candidate policy source");
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .host_type(HostTypeId::new(1))
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
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(
        HostPath::new(host_ref).field(FieldId::new(1)),
        HostValue::Int(9),
    );
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(registry),
        reflect::permissions::ReflectPermissionSet::read_only()
            .with(reflect::permissions::ReflectPermission::InspectHostPath),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    let error = vm
        .run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
        .expect_err("unknown field should diagnose allowed candidates only");
    assert_eq!(
        error.kind,
        VmErrorKind::Reflect(ReflectErrorKind::UnknownField {
            type_name: "Player".to_owned(),
            field: "leve".to_owned(),
            candidates: vec!["level".to_owned()],
            related: vec![ReflectCandidate::new("level", None)],
        })
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn reflection_unknown_host_method_candidates_respect_call_policy() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    return reflect.call(player, "visibl");
}
"#,
    )
    .expect("compile unknown method candidate policy source");
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .host_type(HostTypeId::new(1))
            .method(MethodDesc::new(HostMethodId::new(1), "visible"))
            .method(
                MethodDesc::new(HostMethodId::new(2), "visibly_hidden")
                    .access(MethodAccess::new().reflect_callable(false)),
            )
            .method(
                MethodDesc::new(HostMethodId::new(3), "visibly_private")
                    .access(MethodAccess::new().public(false).reflect_callable(true)),
            )
            .method(
                MethodDesc::new(HostMethodId::new(4), "visibly_admin")
                    .access(MethodAccess::new().require_permission("player.admin.call")),
            ),
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_policy(
        Arc::new(registry),
        reflect::permissions::ReflectPolicy::new(
            reflect::permissions::ReflectPermissionSet::new()
                .with(reflect::permissions::ReflectPermission::CallMethods)
                .with(reflect::permissions::ReflectPermission::InspectHostPath),
        ),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    let error = vm
        .run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
        .expect_err("unknown method should diagnose allowed candidates only");
    assert_eq!(
        error.kind,
        VmErrorKind::Reflect(ReflectErrorKind::UnknownMethod {
            type_name: "Player".to_owned(),
            method: "visibl".to_owned(),
            candidates: vec!["visible".to_owned()],
            related: vec![ReflectCandidate::new("visible", None)],
        })
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn reflection_lookup_budget_stops_after_limit() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    reflect.name(player);
    reflect.kind(player);
    return 1;
}
"#,
    )
    .expect("compile budgeted reflection source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_policy(
        Arc::new(reflection_registry()),
        reflect::permissions::ReflectPolicy::all().with_lookup_limit(1),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
        Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::LookupBudgetExceeded {
            limit: 1
        })
    ));
    assert!(tx.patches().is_empty());
}

#[test]
fn heap_execution_uses_reflection_natives_for_host_state() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    let player_type = reflect.type_of(player);
    if reflect.name(player_type) == "Player" && reflect.kind(player_type) == "host" {
        if reflect.implements(player, "Damageable") {
            reflect.set(player, "level", 10);
            return reflect.get(player, "level");
        }
    }
    return 0;
}
"#,
    )
    .expect("compile reflection source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        vm.run_program_with_host_heap_and_budget(
            &program,
            "main",
            &[Value::HostRef(host_ref)],
            &mut host,
            &mut heap_execution,
            &mut budget,
        )
    };

    assert_eq!(result, Ok(Value::Int(10)));
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(10)));
}
