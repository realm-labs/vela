use super::*;
use crate::owned_value::OwnedValue;

fn compile_reflection_metadata_source(
    source: SourceId,
    text: &str,
) -> vela_bytecode::compiler::error::CompileResult<vela_bytecode::compiler::CompiledProgram> {
    compile_standard_program_source_with_native_functions(
        source,
        text,
        &[
            "reflect::access",
            "reflect::attr",
            "reflect::attrs",
            "reflect::docs",
            "reflect::effects",
            "reflect::field",
            "reflect::fields",
            "reflect::function",
            "reflect::get",
            "reflect::has_attr",
            "reflect::has_field",
            "reflect::has_method",
            "reflect::has_trait",
            "reflect::has_type",
            "reflect::has_variant",
            "reflect::id",
            "reflect::kind",
            "reflect::method",
            "reflect::methods",
            "reflect::name",
            "reflect::origin",
            "reflect::owner",
            "reflect::params",
            "reflect::required_permissions",
            "reflect::returns",
            "reflect::source_span",
            "reflect::trait_info",
            "reflect::traits",
            "reflect::type_info",
            "reflect::types",
            "reflect::variant",
            "reflect::variant_info",
            "reflect::variant_is",
            "reflect::variants",
        ],
    )
}

fn exec_reflection_metadata_program(
    vm: &Vm,
    program: &UnlinkedProgram,
    args: &[OwnedValue],
    host: &mut HostExecution<'_>,
) -> VmResult<OwnedValue> {
    let mut budget = ExecutionBudget::unbounded();
    run_linked_test_program_with_host_budget(vm, program, "main", args, host, &mut budget)
}

#[test]
fn compiled_source_reflection_fields_returns_metadata() {
    let program = compile_reflection_metadata_source(
        SourceId::new(1),
        include_str!("../../tests/fixtures/reflection/field_metadata.vela"),
    )
    .expect("compile reflection fields source");

    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    let mut host = HostExecution { state_values: None };

    let result = exec_reflection_metadata_program(
        &vm,
        &program,
        &[OwnedValue::record(
            "Player",
            [("level", OwnedValue::i64(9))],
        )],
        &mut host,
    );

    assert_eq!(result, Ok(OwnedValue::Bool(true)));
}

#[test]
fn compiled_source_reflects_name_kind_and_field_metadata() {
    let program = compile_reflection_metadata_source(
        SourceId::new(1),
        include_str!("../../tests/fixtures/reflection/name_kind_fields.vela"),
    )
    .expect("compile field reflection source");

    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    vm.register_standard_natives();
    let mut host = HostExecution { state_values: None };

    assert_eq!(
        exec_reflection_metadata_program(
            &vm,
            &program,
            &[OwnedValue::record(
                "Player",
                [("level", OwnedValue::i64(9))]
            )],
            &mut host
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn compiled_source_reflects_required_permissions_metadata() {
    let program = compile_reflection_metadata_source(
        SourceId::new(1),
        include_str!("../../tests/fixtures/reflection/required_permissions.vela"),
    )
    .expect("compile required permission reflection source");
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_policy(
        Arc::new(policy_module_reflection_registry()),
        reflect::permissions::ReflectPolicy::read_only().with_function_permission("game::admin"),
    );

    let mut host = HostExecution { state_values: None };

    assert_eq!(
        exec_reflection_metadata_program(&vm, &program, &[], &mut host),
        Ok(OwnedValue::Bool(true))
    );
}

#[test]
fn compiled_source_reflects_signature_metadata_helpers() {
    let program = compile_reflection_metadata_source(
        SourceId::new(1),
        include_str!("../../tests/fixtures/reflection/signature.vela"),
    )
    .expect("compile reflected signature metadata source");
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(script_module_reflection_registry()));

    let mut host = HostExecution { state_values: None };

    assert_eq!(
        exec_reflection_metadata_program(&vm, &program, &[], &mut host),
        Ok(OwnedValue::Bool(true))
    );
}

#[test]
fn compiled_source_rejects_removed_async_metadata_fields() {
    for target in [
        r#"reflect::function("game::probe")"#,
        r#"reflect::method(reflect::type_info("ProbeHost"), "probe")"#,
    ] {
        let program = compile_reflection_metadata_source(
            SourceId::new(1),
            &format!("fn main() {{ let target = {target}; return target.is_async; }}"),
        )
        .expect("compile ordinary metadata field access");
        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            SourceId::new(2),
            vela_package::PackageId::anonymous(),
            ModulePath::from_qualified("game"),
            "pub fn probe() { return 1; }",
        ));
        let mut registry = TypeRegistry::new();
        registry.register_script_modules(&graph);
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(0xA535), "ProbeHost"))
                .method(MethodDesc::new(MethodId::new(0xA535), "probe")),
        );
        let mut vm = Vm::new();
        vm.register_reflection_natives(Arc::new(registry));

        let mut host = HostExecution { state_values: None };
        let error = exec_reflection_metadata_program(&vm, &program, &[], &mut host)
            .expect_err("removed async metadata must not be projected");
        assert!(error.to_string().contains("is_async"), "{error}");
    }
}

#[test]
fn compiled_source_reflect_fields_respect_field_access() {
    let program = compile_reflection_metadata_source(
        SourceId::new(1),
        include_str!("../../tests/fixtures/reflection/field_access.vela"),
    )
    .expect("compile policy fields reflection source");

    let mut vm = Vm::new();
    let policy = reflect::permissions::ReflectPolicy::new(
        reflect::permissions::ReflectPermissionSet::new()
            .with(reflect::permissions::ReflectPermission::ReadTypeInfo)
            .with(reflect::permissions::ReflectPermission::ReadTypeInfo),
    );
    vm.register_reflection_natives_with_policy(
        Arc::new(policy_field_reflection_registry()),
        policy,
    );
    let mut host = HostExecution { state_values: None };

    assert_eq!(
        exec_reflection_metadata_program(
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
fn compiled_source_reflects_methods_traits_and_variants() {
    let program = compile_reflection_metadata_source(
        SourceId::new(1),
        include_str!("../../tests/fixtures/reflection/methods_traits_variants.vela"),
    )
    .expect("compile member reflection source");

    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(member_reflection_registry()));
    let mut host = HostExecution { state_values: None };

    assert_eq!(
        exec_reflection_metadata_program(
            &vm,
            &program,
            &[OwnedValue::record(
                "Player",
                [("level", OwnedValue::i64(9))]
            )],
            &mut host
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn compiled_source_reflects_registered_trait_metadata() {
    let program = compile_reflection_metadata_source(
        SourceId::new(1),
        include_str!("../../tests/fixtures/reflection/registered_trait.vela"),
    )
    .expect("compile trait metadata reflection source");

    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    let mut host = HostExecution { state_values: None };

    assert_eq!(
        exec_reflection_metadata_program(&vm, &program, &[], &mut host),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn compiled_source_reflects_registered_type_metadata() {
    let program = compile_reflection_metadata_source(
        SourceId::new(1),
        include_str!("../../tests/fixtures/reflection/registered_type.vela"),
    )
    .expect("compile type metadata reflection source");

    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    let mut host = HostExecution { state_values: None };

    assert_eq!(
        exec_reflection_metadata_program(&vm, &program, &[], &mut host),
        Ok(OwnedValue::String("Player".to_owned()))
    );
}

#[test]
fn compiled_source_reflects_type_source_span_metadata() {
    let program = compile_reflection_metadata_source(
        SourceId::new(1),
        r#"
fn main() {
    let player = reflect::type_info("Player");
    let span = reflect::get(player, "source_span").unwrap_or(());
    if reflect::get(span, "source") == 7
        && reflect::get(span, "start") == 20
        && reflect::get(span, "end") == 42 {
        return reflect::get(player, "name");
    }
    return "missing";
}
"#,
    )
    .expect("compile type source span metadata source");
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .kind(TypeKind::ScriptStruct)
            .source_span(Span::new(SourceId::new(7), 20, 42)),
    );

    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(registry));
    let mut host = HostExecution { state_values: None };

    assert_eq!(
        exec_reflection_metadata_program(&vm, &program, &[], &mut host),
        Ok(OwnedValue::String("Player".to_owned()))
    );
}
