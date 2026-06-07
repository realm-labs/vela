use super::*;
use crate::owned_value::OwnedValue;

#[test]
fn compiled_source_reflection_fields_returns_metadata() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    let fields = reflect::fields(player);
    return fields.len() == 2
        && fields[0].owner == "Player"
        && fields[0].name == "id"
        && fields[1].owner == "Player"
        && fields[1].name == "level"
        && reflect::kind(fields[1]) == "field";
}
"#,
    )
    .expect("compile reflection fields source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    let result = vm.run_program_with_host(
        &program,
        "main",
        &[OwnedValue::HostRef(host_ref)],
        &mut host,
    );

    assert_eq!(result, Ok(OwnedValue::Bool(true)));
}

#[test]
fn compiled_source_reflects_name_kind_and_field_metadata() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
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
        && option::unwrap_or(reflect::attrs(player).get("domain"), "") == "gameplay"
        && reflect::attr(player, "domain") == "gameplay"
        && reflect::has_attr(player, "domain")
        && reflect::attr(player, "missing") == null
        && !reflect::has_attr(player, "missing")
        && reflect::has_field(player, "level")
        && reflect::has_field(player_type, "level")
        && !reflect::has_field(player, "mana")
        && reflect::fields(player_type).len() == 2
        && all_fields.len() == 2
        && all_fields[1].owner == "Player"
        && all_fields[1].name == "level"
        && field.owner == "Player"
        && field.name == "level"
        && type_field.name == "level"
        && type_field.id == field.id
        && reflect::name(field) == "level"
        && reflect::owner(field) == "Player"
        && reflect::origin(field) == "host"
        && reflect::id(field) == 2
        && reflect::kind(field) == "field"
        && field.type == "int"
        && field.docs == "Current player level."
        && reflect::docs(field) == "Current player level."
        && reflect::source_span(field) == null
        && access.reflect_readable
        && access.reflect_writable
        && option::unwrap_or(field.attrs.get("unit"), "") == "level"
        && option::unwrap_or(reflect::attrs(field).get("unit"), "") == "level"
        && reflect::attr(field, "unit") == "level"
        && reflect::has_attr(field, "unit")
        && reflect::attr(field, "missing") == null
        && !reflect::has_attr(field, "missing")
        && field.writable {
        return 1;
    }
    return 0;
}
"#,
    )
    .expect("compile field reflection source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
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
        vm.run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host
        ),
        Ok(OwnedValue::Int(1))
    );
}

#[test]
fn compiled_source_reflects_required_permissions_metadata() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let function = reflect::function("game::reward::admin");
    let permissions = reflect::required_permissions(function);
    let access = reflect::access(function);
    let access_permissions = reflect::required_permissions(function.access);
    let direct_access = reflect::access(function.access);
    let public_function = reflect::function("game::reward::grant");
    return permissions.len() == 1
        && permissions[0] == "game::admin"
        && access.required_permissions[0] == "game::admin"
        && access_permissions.len() == 1
        && access_permissions[0] == "game::admin"
        && direct_access.required_permissions[0] == "game::admin"
        && reflect::required_permissions(public_function).is_empty();
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
        vm.run_program_with_host(&program, "main", &[], &mut host),
        Ok(OwnedValue::Bool(true))
    );
}

#[test]
fn compiled_source_reflects_effect_metadata_helper() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    let method = reflect::method(player, "grant_exp");
    let effects = reflect::effects(method);
    let direct = reflect::effects(method.effects);
    return effects.writes_host
        && effects.reads_host
        && !effects.emits_events
        && direct.writes_host
        && direct.reads_host
        && reflect::kind(effects) == "effect_set";
}
"#,
    )
    .expect("compile reflected effect metadata source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        vm.run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host
        ),
        Ok(OwnedValue::Bool(true))
    );
}

#[test]
fn compiled_source_reflects_signature_metadata_helpers() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let function = reflect::function("game::reward::grant");
    let params = reflect::params(function);
    let direct = reflect::params(function.params);
    return params.len() == 2
        && params[0].name == "player"
        && params[0].type == "Player"
        && params[1].name == "amount"
        && params[1].defaulted
        && direct[1].name == "amount"
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
        vm.run_program_with_host(&program, "main", &[], &mut host),
        Ok(OwnedValue::Bool(true))
    );
}

#[test]
fn compiled_source_reflect_fields_respect_field_access() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    let fields = reflect::fields(player);
    let all_fields = reflect::fields();
    if reflect::has_field(player, "level")
        && !reflect::has_field(player, "secret")
        && fields[0].owner == "Player"
        && fields[0].name == "level"
        && reflect::field(player, "level").name == "level" {
        return fields.len() * 10 + all_fields.len();
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
        vm.run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host
        ),
        Ok(OwnedValue::Int(11))
    );
}

#[test]
fn compiled_source_reflects_methods_traits_and_variants() {
    let program = compile_program_source(
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
        && methods.len() == 1
        && type_methods.len() == 1
        && all_methods.len() == 1
        && all_methods[0].owner == "Player"
        && all_methods[0].name == "grant_exp"
        && methods[0].owner == "Player"
        && method.name == "grant_exp"
        && type_method.id == method.id
        && method.owner == "Player"
        && reflect::owner(method) == "Player"
        && reflect::origin(method) == "host"
        && method.attrs["effect"] == "write"
        && methods[0].returns == "bool"
        && methods[0].params[0].name == "amount"
        && methods[0].params[0].type == "int"
        && method.params[0].name == "amount"
        && traits.len() == 1
        && type_traits.len() == 1
        && variants.len() == 2
        && type_variants.len() == 2
        && variants[0].owner == "QuestProgress"
        && reflect::owner(variants[0]) == "QuestProgress"
        && variants[0].fields[0].owner == "QuestProgress::Active"
        && active.name == "Active"
        && type_active.id == active.id
        && active.owner == "QuestProgress"
        && reflect::owner(active) == "QuestProgress"
        && reflect::origin(active) == "host"
        && active.fields[0].name == "count"
        && active.fields[0].owner == "QuestProgress::Active"
        && reflect::has_field(quest, "count")
        && !reflect::has_field(quest, "missing")
        && active_fields.len() == 1
        && active_fields[0].name == "count"
        && active_fields[0].owner == "QuestProgress::Active"
        && active_count.name == "count"
        && active_count.owner == "QuestProgress::Active"
        && active_count.id == active.fields[0].id
        && all_fields.len() == 3
        && all_fields[2].owner == "QuestProgress::Active"
        && all_fields[2].name == "count"
        && type_variants[0].fields[0].owner == "QuestProgress::Active"
        && type_active.fields[0].owner == "QuestProgress::Active"
        && reflect::has_variant(quest_type, "Active")
        && all_variants.len() == 2
        && all_variants[0].owner == "QuestProgress"
        && all_variants[0].name == "Active"
        && all_variants[0].fields[0].owner == "QuestProgress::Active"
        && reflect::variant(quest) == "Active"
        && reflect::has_variant(quest, "Active")
        && !reflect::has_variant(quest, "Paused")
        && reflect::variant_is(quest, "Active") {
        return variants[0].fields.len();
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
        vm.run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(player_ref(3))],
            &mut host
        ),
        Ok(OwnedValue::Int(1))
    );
}

#[test]
fn compiled_source_reflects_registered_trait_metadata() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let traits = reflect::traits();
    let trait_info = reflect::trait_info("Damageable");
    if traits.len() == 1
        && reflect::has_trait("Damageable")
        && !reflect::has_trait("Damagable")
        && traits[0].name == "Damageable"
        && trait_info.name == "Damageable"
        && trait_info.methods[0].name == "damage"
        && trait_info.methods[0].owner == "Damageable"
        && reflect::origin(trait_info) == "host"
        && reflect::owner(trait_info.methods[0]) == "Damageable"
        && reflect::kind(trait_info.methods[0]) == "trait_method" {
        return trait_info.methods.len();
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
        vm.run_program_with_host(&program, "main", &[], &mut host),
        Ok(OwnedValue::Int(1))
    );
}

#[test]
fn compiled_source_reflects_registered_type_metadata() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let types = reflect::types();
    let player = reflect::type_info("Player");
    if types.len() == 1
        && reflect::has_type("Player")
        && !reflect::has_type("Plyer")
        && types[0].name == "Player"
        && types[0].id == player.id
        && reflect::name(types[0]) == "Player"
        && reflect::kind(types[0]) == "host"
        && player.kind == "host"
        && reflect::kind(player) == "host"
        && player.origin == "host"
        && reflect::origin(player) == "host"
        && player.field_count == 2
        && player.method_count == 1
        && player.trait_count == 1 {
        return player.name;
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
        vm.run_program_with_host(&program, "main", &[], &mut host),
        Ok(OwnedValue::String("Player".to_owned()))
    );
}

#[test]
fn compiled_source_reflects_type_source_span_metadata() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let player = reflect::type_info("Player");
    if player.source_span.source == 7
        && player.source_span.start == 20
        && player.source_span.end == 42 {
        return player.name;
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
        vm.run_program_with_host(&program, "main", &[], &mut host),
        Ok(OwnedValue::String("Player".to_owned()))
    );
}
