use super::*;
use crate::owned_value::OwnedValue;

fn compile_reflection_metadata_source(
    source: SourceId,
    text: &str,
) -> vela_bytecode::compiler::error::CompileResult<UnlinkedProgram> {
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
    let host_ref = player_ref(3);
    let program = compile_reflection_metadata_source(
        SourceId::new(1),
        r#"
fn main(player) {
    let fields = reflect::fields(player);
    let player_type = reflect::type_info("Player");
    return reflect::get(player_type, "field_count") == 2
        && reflect::get(fields[0], "owner") == "Player"
        && reflect::get(fields[0], "name") == "id"
        && reflect::get(fields[1], "owner") == "Player"
        && reflect::get(fields[1], "name") == "level"
        && reflect::kind(fields[1]) == "field";
}
"#,
    )
    .expect("compile reflection fields source");
    let mut adapter = host_adapter(
        host_ref,
        HostValue::Scalar(vela_common::ScalarValue::I64(9)),
    );
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    let result = exec_reflection_metadata_program(
        &vm,
        &program,
        &[OwnedValue::HostRef(host_ref)],
        &mut host,
    );

    assert_eq!(result, Ok(OwnedValue::Bool(true)));
}

#[test]
fn compiled_source_reflects_name_kind_and_field_metadata() {
    let host_ref = player_ref(3);
    let program = compile_reflection_metadata_source(
        SourceId::new(1),
        r#"
fn main(player) {
    let player_type = reflect::type_info("Player");
    let field = reflect::field(player, "level");
    let type_field = reflect::field(player_type, "level");
    let access = reflect::access(field);
    let all_fields = reflect::fields();
    if reflect::name(player) == "Player"
        && reflect::id(player) == 100
        && reflect::kind(player) == "host"
        && reflect::docs(player) == "A player host object."
        && reflect::attr(player, "domain") == "gameplay"
        && reflect::attr(player, "domain") == "gameplay"
        && reflect::has_attr(player, "domain")
        && reflect::attr(player, "missing") == null
        && !reflect::has_attr(player, "missing")
        && reflect::has_field(player, "level")
        && reflect::has_field(player_type, "level")
        && !reflect::has_field(player, "mana")
        && reflect::get(player_type, "field_count") == 2
        && reflect::get(all_fields[1], "owner") == "Player"
        && reflect::get(all_fields[1], "name") == "level"
        && reflect::get(field, "owner") == "Player"
        && reflect::get(field, "name") == "level"
        && reflect::get(type_field, "name") == "level"
        && reflect::get(type_field, "id") == reflect::get(field, "id")
        && reflect::name(field) == "level"
        && reflect::owner(field) == "Player"
        && reflect::origin(field) == "host"
        && reflect::id(field) == 2
        && reflect::kind(field) == "field"
        && reflect::get(field, "type") == "i64"
        && reflect::get(field, "docs") == "Current player level."
        && reflect::docs(field) == "Current player level."
        && reflect::source_span(field) == null
        && reflect::get(access, "reflect_readable")
        && reflect::get(access, "reflect_writable")
        && reflect::attr(field, "unit") == "level"
        && reflect::attr(field, "unit") == "level"
        && reflect::has_attr(field, "unit")
        && reflect::attr(field, "missing") == null
        && !reflect::has_attr(field, "missing")
        && reflect::get(field, "writable") {
        return 1;
    }
    return 0;
}
"#,
    )
    .expect("compile field reflection source");
    let mut adapter = host_adapter(
        host_ref,
        HostValue::Scalar(vela_common::ScalarValue::I64(9)),
    );
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    vm.register_standard_natives();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        exec_reflection_metadata_program(
            &vm,
            &program,
            &[OwnedValue::HostRef(host_ref)],
            &mut host
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn compiled_source_reflects_required_permissions_metadata() {
    let program = compile_reflection_metadata_source(
        SourceId::new(1),
        r#"
fn main() {
    let function = reflect::function("game::reward::admin");
    let permissions = reflect::required_permissions(function);
    let access = reflect::access(function);
    let function_access = reflect::get(function, "access");
    let access_permissions = reflect::required_permissions(function_access);
    let direct_access = reflect::access(function_access);
    let public_function = reflect::function("game::reward::grant");
    return permissions[0] == "game::admin"
        && reflect::get(access, "required_permissions")[0] == "game::admin"
        && access_permissions[0] == "game::admin"
        && reflect::get(direct_access, "required_permissions")[0] == "game::admin"
        && reflect::required_permissions(public_function).len() == 0;
}
"#,
    )
    .expect("compile required permission reflection source");
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_policy(
        Arc::new(policy_module_reflection_registry()),
        reflect::permissions::ReflectPolicy::read_only().with_function_permission("game::admin"),
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        exec_reflection_metadata_program(&vm, &program, &[], &mut host),
        Ok(OwnedValue::Bool(true))
    );
}

#[test]
fn compiled_source_reflects_effect_metadata_helper() {
    let host_ref = player_ref(3);
    let program = compile_reflection_metadata_source(
        SourceId::new(1),
        r#"
fn main(player) {
    let method = reflect::method(player, "grant_exp");
    let effects = reflect::effects(method);
    let direct = reflect::effects(reflect::get(method, "effects"));
    return reflect::get(effects, "writes_host")
        && reflect::get(effects, "reads_host")
        && !reflect::get(effects, "emits_events")
        && reflect::get(direct, "writes_host")
        && reflect::get(direct, "reads_host")
        && reflect::kind(effects) == "effect_set";
}
"#,
    )
    .expect("compile reflected effect metadata source");
    let mut adapter = host_adapter(
        host_ref,
        HostValue::Scalar(vela_common::ScalarValue::I64(9)),
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
        exec_reflection_metadata_program(
            &vm,
            &program,
            &[OwnedValue::HostRef(host_ref)],
            &mut host
        ),
        Ok(OwnedValue::Bool(true))
    );
}

#[test]
fn compiled_source_reflects_signature_metadata_helpers() {
    let program = compile_reflection_metadata_source(
        SourceId::new(1),
        r#"
fn main() {
    let function = reflect::function("game::reward::grant");
    let params = reflect::params(function);
    let direct = reflect::params(reflect::get(function, "params"));
    return reflect::get(params[0], "name") == "player"
        && reflect::get(params[0], "type") == "Player"
        && reflect::get(params[1], "name") == "amount"
        && reflect::get(params[1], "defaulted")
        && reflect::get(direct[1], "name") == "amount"
        && reflect::returns(function) == "bool";
}
"#,
    )
    .expect("compile reflected signature metadata source");
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(script_module_reflection_registry()));
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        exec_reflection_metadata_program(&vm, &program, &[], &mut host),
        Ok(OwnedValue::Bool(true))
    );
}

#[test]
fn compiled_source_reflect_fields_respect_field_access() {
    let host_ref = player_ref(3);
    let program = compile_reflection_metadata_source(
        SourceId::new(1),
        r#"
fn main(player) {
    let fields = reflect::fields(player);
    let all_fields = reflect::fields();
    if reflect::has_field(player, "level")
        && !reflect::has_field(player, "secret")
        && reflect::owner(fields[0]) == "Player"
        && reflect::name(fields[0]) == "level"
        && reflect::name(reflect::field(player, "level")) == "level"
        && reflect::name(all_fields[0]) == "level" {
        return 11;
    }
    return 0;
}
"#,
    )
    .expect("compile policy fields reflection source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    let policy = reflect::permissions::ReflectPolicy::new(
        reflect::permissions::ReflectPermissionSet::new()
            .with(reflect::permissions::ReflectPermission::ReadTypeInfo)
            .with(reflect::permissions::ReflectPermission::InspectHostPath),
    );
    vm.register_reflection_natives_with_policy(
        Arc::new(policy_field_reflection_registry()),
        policy,
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        exec_reflection_metadata_program(
            &vm,
            &program,
            &[OwnedValue::HostRef(host_ref)],
            &mut host
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(11)))
    );
}

#[test]
fn compiled_source_reflects_methods_traits_and_variants() {
    let program = compile_reflection_metadata_source(
        SourceId::new(1),
        r#"
fn main(player) {
    let player_type = reflect::type_info("Player");
    let quest_type = reflect::type_info("QuestProgress");
    let methods = reflect::methods(player);
    let method = reflect::method(player, "grant_exp");
    let type_methods = reflect::methods(player_type);
    let type_method = reflect::method(player_type, "grant_exp");
    let all_methods = reflect::methods();
    let traits = reflect::traits(player);
    let type_traits = reflect::traits(player_type);
    let quest = QuestProgress::Active { count: 1 };
    let variants = reflect::variants(quest);
    let active = reflect::variant_info(quest, "Active");
    let active_fields = reflect::fields(quest);
    let active_count = reflect::field(quest, "count");
    let all_fields = reflect::fields();
    let type_variants = reflect::variants(quest_type);
    let type_active = reflect::variant_info(quest_type, "Active");
    let all_variants = reflect::variants();
    if reflect::has_method(player, "grant_exp")
        && reflect::has_method(player_type, "grant_exp")
        && reflect::get(player_type, "method_count") == 1
        && reflect::get(all_methods[0], "owner") == "Player"
        && reflect::get(all_methods[0], "name") == "grant_exp"
        && reflect::get(methods[0], "owner") == "Player"
        && reflect::get(method, "name") == "grant_exp"
        && reflect::get(type_method, "id") == reflect::get(method, "id")
        && reflect::get(method, "owner") == "Player"
        && reflect::owner(method) == "Player"
        && reflect::origin(method) == "host"
        && reflect::attr(method, "effect") == "write"
        && reflect::get(methods[0], "return") == "bool"
        && reflect::get(reflect::get(methods[0], "params")[0], "name") == "amount"
        && reflect::get(reflect::get(methods[0], "params")[0], "type") == "i64"
        && reflect::get(reflect::get(method, "params")[0], "name") == "amount"
        && reflect::get(player_type, "trait_count") == 1
        && reflect::get(quest_type, "variant_count") == 2
        && reflect::get(variants[0], "owner") == "QuestProgress"
        && reflect::owner(variants[0]) == "QuestProgress"
        && reflect::get(reflect::get(variants[0], "fields")[0], "owner") == "QuestProgress::Active"
        && reflect::get(active, "name") == "Active"
        && reflect::get(type_active, "id") == reflect::get(active, "id")
        && reflect::get(active, "owner") == "QuestProgress"
        && reflect::owner(active) == "QuestProgress"
        && reflect::origin(active) == "host"
        && reflect::get(reflect::get(active, "fields")[0], "name") == "count"
        && reflect::get(reflect::get(active, "fields")[0], "owner") == "QuestProgress::Active"
        && reflect::has_field(quest, "count")
        && !reflect::has_field(quest, "missing")
        && reflect::get(active_fields[0], "name") == "count"
        && reflect::get(active_fields[0], "owner") == "QuestProgress::Active"
        && reflect::get(active_count, "name") == "count"
        && reflect::get(active_count, "owner") == "QuestProgress::Active"
        && reflect::get(active_count, "id") == reflect::get(reflect::get(active, "fields")[0], "id")
        && reflect::get(all_fields[2], "owner") == "QuestProgress::Active"
        && reflect::get(all_fields[2], "name") == "count"
        && reflect::get(reflect::get(type_variants[0], "fields")[0], "owner") == "QuestProgress::Active"
        && reflect::get(reflect::get(type_active, "fields")[0], "owner") == "QuestProgress::Active"
        && reflect::has_variant(quest_type, "Active")
        && reflect::get(all_variants[0], "owner") == "QuestProgress"
        && reflect::get(all_variants[0], "name") == "Active"
        && reflect::get(reflect::get(all_variants[0], "fields")[0], "owner") == "QuestProgress::Active"
        && reflect::variant(quest) == "Active"
        && reflect::has_variant(quest, "Active")
        && !reflect::has_variant(quest, "Paused")
        && reflect::variant_is(quest, "Active") {
        return 1;
    }
    return 0;
}
"#,
    )
    .expect("compile member reflection source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(member_reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        exec_reflection_metadata_program(
            &vm,
            &program,
            &[OwnedValue::HostRef(player_ref(3))],
            &mut host
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn compiled_source_reflects_registered_trait_metadata() {
    let program = compile_reflection_metadata_source(
        SourceId::new(1),
        r#"
fn main() {
    let traits = reflect::traits();
    let trait_info = reflect::trait_info("Damageable");
    if reflect::has_trait("Damageable")
        && !reflect::has_trait("Damagable")
        && reflect::get(traits[0], "name") == "Damageable"
        && reflect::get(trait_info, "name") == "Damageable"
        && reflect::get(reflect::get(trait_info, "methods")[0], "name") == "damage"
        && reflect::get(reflect::get(trait_info, "methods")[0], "owner") == "Damageable"
        && reflect::origin(trait_info) == "host"
        && reflect::owner(reflect::get(trait_info, "methods")[0]) == "Damageable"
        && reflect::kind(reflect::get(trait_info, "methods")[0]) == "trait_method" {
        return 1;
    }
    return 0;
}
"#,
    )
    .expect("compile trait metadata reflection source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        exec_reflection_metadata_program(&vm, &program, &[], &mut host),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn compiled_source_reflects_registered_type_metadata() {
    let program = compile_reflection_metadata_source(
        SourceId::new(1),
        r#"
fn main() {
    let types = reflect::types();
    let player = reflect::type_info("Player");
    if reflect::has_type("Player")
        && !reflect::has_type("Plyer")
        && reflect::get(types[0], "name") == "Player"
        && reflect::get(types[0], "id") == reflect::get(player, "id")
        && reflect::name(types[0]) == "Player"
        && reflect::kind(types[0]) == "host"
        && reflect::get(player, "kind") == "host"
        && reflect::kind(player) == "host"
        && reflect::get(player, "origin") == "host"
        && reflect::origin(player) == "host"
        && reflect::get(player, "field_count") == 2
        && reflect::get(player, "method_count") == 1
        && reflect::get(player, "trait_count") == 1 {
        return reflect::get(player, "name");
    }
    return "missing";
}
"#,
    )
    .expect("compile type metadata reflection source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

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
    let span = reflect::get(player, "source_span");
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
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(registry));
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        exec_reflection_metadata_program(&vm, &program, &[], &mut host),
        Ok(OwnedValue::String("Player".to_owned()))
    );
}
