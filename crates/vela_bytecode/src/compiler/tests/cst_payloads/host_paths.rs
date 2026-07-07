use super::*;
use crate::compiler::host_paths::{
    DynamicHostPathPart, HostPathPart as CompilerHostPathPart, HostPathRoot,
};

#[test]
fn host_path_with_non_path_cst_payload_does_not_use_legacy_path() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty("host", std::iter::empty::<&str>(), "Player"))
                .host_runtime_id(77),
        )
        .expect("Player host type should register");
    let level = FieldId::new(3);
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field("host", std::iter::empty::<&str>(), "Player", "level"),
                player,
            )
            .host_runtime_id(level.get())
            .writable(true)
            .type_hint(Some("i64".to_owned())),
        )
        .expect("Player level field should register");

    let source = SourceId::new(1);
    let semantic = parse_semantic_source(
        source,
        r#"
fn main(player: Player) {
    let cst_value = {
        let selected = player;
        selected;
        selected && true
    };
    make(player).level;
}

fn make(value) {
    return value;
}
"#,
    )
    .expect("semantic source should parse");
    let facts = cst_payload_compiler_facts_with_options(
        &semantic,
        CompilerOptions::default(),
        Some(registry.compile_view()),
    );
    let (payload, signature, bindings) = semantic.function("main").expect("main function");
    let statements = paired_statement_payloads_for_body(source, &payload.body);
    let cst_block = statements[0]
        .let_initializer_expression_payload()
        .expect("CST block initializer");
    let legacy_path = statements[1]
        .expression_payload()
        .expect("legacy host path expression");
    let mismatched_payload = expression_payload_with_fallback(
        source,
        cst_block
            .syntax_expression()
            .expect("CST block syntax")
            .clone(),
        legacy_path.fallback(),
    );
    let compiler = Compiler::new_with_param_defaults(
        payload.name.clone(),
        payload.body.clone(),
        payload.param_defaults.clone(),
        signature,
        bindings,
        facts,
    )
    .expect("compiler should initialize");

    assert!(
        compiler
            .resolve_host_path_with_payload(
                mismatched_payload.fallback(),
                Some(&mismatched_payload)
            )
            .is_none(),
        "non-path CST payload must not resolve the legacy host path"
    );
}

#[test]
fn syntax_host_index_path_lowers_untyped_field_key_from_cst() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty("host", std::iter::empty::<&str>(), "Player"))
                .host_runtime_id(77),
        )
        .expect("Player host type should register");
    let inventory = FieldId::new(3);
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field("host", std::iter::empty::<&str>(), "Player", "inventory"),
                player,
            )
            .host_runtime_id(inventory.get())
            .writable(true),
        )
        .expect("Player inventory field should register");

    let source = SourceId::new(1);
    let semantic = parse_semantic_source(
        source,
        r#"
fn main(player: Player, amount) {
    player.inventory["gold"] += amount;
}
"#,
    )
    .expect("semantic source should parse");
    let facts = cst_payload_compiler_facts_with_options(
        &semantic,
        CompilerOptions::default(),
        Some(registry.compile_view()),
    );
    let (payload, signature, bindings) = semantic.function("main").expect("main function");
    let statement = cst_statement_payloads(&payload.body)
        .into_iter()
        .next()
        .expect("assignment statement");
    let expression = statement
        .expression_payload()
        .and_then(|payload| payload.syntax_expression().cloned())
        .expect("CST assignment expression");
    let target = expression
        .as_assign()
        .and_then(|assign| assign.target())
        .expect("CST assignment target");
    let compiler = Compiler::new_with_param_defaults(
        payload.name.clone(),
        payload.body.clone(),
        payload.param_defaults.clone(),
        signature,
        bindings,
        facts,
    )
    .expect("compiler should initialize");

    let path = compiler
        .syntax_root_host_index_path(source, &target)
        .expect("CST host index path should resolve");
    let HostPathRoot::OwnedLocalPath { name, .. } = path.root else {
        panic!("expected local host root");
    };
    assert_eq!(name, "player");
    let [
        CompilerHostPathPart::Field(field),
        CompilerHostPathPart::SyntaxValue {
            expression,
            dynamic_kind,
            ..
        },
    ] = path.segments.as_slice()
    else {
        panic!("expected field plus syntax key segment");
    };
    assert_eq!(*field, inventory);
    assert!(matches!(dynamic_kind, DynamicHostPathPart::Key));
    assert_eq!(expression.syntax().text().to_string(), "\"gold\"");
}

#[test]
fn nested_syntax_host_index_validates_receiver_capability_from_cst_path() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty("host", std::iter::empty::<&str>(), "Player"))
                .host_runtime_id(77),
        )
        .expect("Player host type should register");
    let inventory_type = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "Inventory",
            ))
            .host_runtime_id(78),
        )
        .expect("Inventory host type should register");
    registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty("host", std::iter::empty::<&str>(), "ItemMap"))
                .host_runtime_id(79),
        )
        .expect("ItemMap host type should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field("host", std::iter::empty::<&str>(), "Player", "inventory"),
                player,
            )
            .host_runtime_id(FieldId::new(3).get())
            .writable(true)
            .type_hint(Some("Inventory".to_owned())),
        )
        .expect("Player inventory field should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field("host", std::iter::empty::<&str>(), "Inventory", "items"),
                inventory_type,
            )
            .host_runtime_id(FieldId::new(4).get())
            .writable(true)
            .type_hint(Some("ItemMap".to_owned())),
        )
        .expect("Inventory items field should register");

    let error = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    return player.inventory.items["gold"];
}
"#,
        registry.compile_view(),
    )
    .expect_err("nested CST host index receiver without capability should fail");

    assert_eq!(
        semantic_diagnostic_codes(error),
        ["analysis::host_index_not_supported"]
    );
}

#[test]
fn missing_host_path_expression_payload_does_not_use_legacy_path() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty("host", std::iter::empty::<&str>(), "Player"))
                .host_runtime_id(77),
        )
        .expect("Player host type should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field("host", std::iter::empty::<&str>(), "Player", "level"),
                player,
            )
            .host_runtime_id(FieldId::new(3).get())
            .writable(true)
            .type_hint(Some("i64".to_owned())),
        )
        .expect("Player level field should register");

    let source = SourceId::new(1);
    let semantic = parse_semantic_source(
        source,
        r#"
fn take(value) {
    return value;
}

fn main(player: Player) {
    take(player::level);
}
"#,
    )
    .expect("semantic source should parse");
    let facts = cst_payload_compiler_facts_with_options(
        &semantic,
        CompilerOptions::default(),
        Some(registry.compile_view()),
    );
    let (payload, signature, bindings) = semantic.function("main").expect("main function");
    let call = paired_statement_payloads_for_body(source, &payload.body)[0]
        .expression_payload()
        .expect("call expression payload");
    let _legacy_path = call
        .call_argument_value_payloads()
        .expect("host path call argument payloads")
        .remove(0);
    let legacy_expr = call_argument_fallback(&call, 0);
    let missing_path = body_payloads::CompilerExpressionPayload::missing_syntax(source);
    let compiler = Compiler::new_with_param_defaults(
        payload.name.clone(),
        payload.body.clone(),
        payload.param_defaults.clone(),
        signature,
        bindings,
        facts,
    )
    .expect("compiler should initialize");

    assert!(
        compiler
            .resolve_host_path_with_payload(legacy_expr, Some(&missing_path))
            .is_none(),
        "missing source-backed CST path payload must not resolve the legacy host path"
    );
}

#[test]
fn indexed_host_path_with_non_index_cst_payload_does_not_use_legacy_index() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty("host", std::iter::empty::<&str>(), "Player"))
                .host_runtime_id(77),
        )
        .expect("Player host type should register");
    let inventory_type = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "Inventory",
            ))
            .host_runtime_id(78),
        )
        .expect("Inventory host type should register");
    let inventory = FieldId::new(3);
    let items = FieldId::new(4);
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field("host", std::iter::empty::<&str>(), "Player", "inventory"),
                player,
            )
            .host_runtime_id(inventory.get())
            .writable(true)
            .type_hint(Some("Inventory".to_owned())),
        )
        .expect("Player inventory field should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field("host", std::iter::empty::<&str>(), "Inventory", "items"),
                inventory_type,
            )
            .host_runtime_id(items.get())
            .writable(true)
            .type_hint(Some("ItemMap".to_owned())),
        )
        .expect("Inventory items field should register");

    let source = SourceId::new(1);
    let semantic = parse_semantic_source(
        source,
        r#"
fn main(player: Player) {
    let cst_value = {
        let selected = player;
        selected;
        selected && true
    };
    player.inventory.items["gold"];
}
"#,
    )
    .expect("semantic source should parse");
    let facts = cst_payload_compiler_facts_with_options(
        &semantic,
        CompilerOptions::default().with_host_index_capability(
            "ItemMap",
            crate::compiler::options::HostIndexCapabilityInfo {
                readable: true,
                value_type: Some("Item".to_owned()),
                ..Default::default()
            },
        ),
        Some(registry.compile_view()),
    );
    let (payload, signature, bindings) = semantic.function("main").expect("main function");
    let statements = paired_statement_payloads_for_body(source, &payload.body);
    let cst_block = statements[0]
        .let_initializer_expression_payload()
        .expect("CST block initializer");
    let legacy_index = statements[1]
        .expression_payload()
        .expect("legacy host index expression");
    let mismatched_payload = expression_payload_with_fallback(
        source,
        cst_block
            .syntax_expression()
            .expect("CST block syntax")
            .clone(),
        legacy_index.fallback(),
    );
    let compiler = Compiler::new_with_param_defaults(
        payload.name.clone(),
        payload.body.clone(),
        payload.param_defaults.clone(),
        signature,
        bindings,
        facts,
    )
    .expect("compiler should initialize");

    assert!(
        compiler
            .resolve_host_path_with_payload(
                mismatched_payload.fallback(),
                Some(&mismatched_payload)
            )
            .is_none(),
        "non-index CST payload must not resolve the legacy host index"
    );
}

#[test]
fn host_field_base_with_misaligned_cst_payload_does_not_use_legacy_receiver() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let cst_player = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "CstPlayer",
            ))
            .host_runtime_id(77),
        )
        .expect("CstPlayer host type should register");
    let legacy_player = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "LegacyPlayer",
            ))
            .host_runtime_id(78),
        )
        .expect("LegacyPlayer host type should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field("host", std::iter::empty::<&str>(), "CstPlayer", "level"),
                cst_player,
            )
            .host_runtime_id(FieldId::new(3).get())
            .writable(true),
        )
        .expect("CstPlayer level field should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field("host", std::iter::empty::<&str>(), "LegacyPlayer", "level"),
                legacy_player,
            )
            .host_runtime_id(FieldId::new(4).get())
            .writable(true),
        )
        .expect("LegacyPlayer level field should register");

    let source = SourceId::new(1);
    let semantic = parse_semantic_source(
        source,
        r#"
fn main(cst: CstPlayer, legacy: LegacyPlayer) {
    make(cst).level;
    make(legacy).level;
}

fn make(value) {
    return value;
}
"#,
    )
    .expect("semantic source should parse");
    let facts = cst_payload_compiler_facts_with_options(
        &semantic,
        CompilerOptions::default(),
        Some(registry.compile_view()),
    );
    let (payload, signature, bindings) = semantic.function("main").expect("main function");
    let statements = paired_statement_payloads_for_body(source, &payload.body);
    let cst_field = statements[0]
        .expression_payload()
        .expect("CST host field expression");
    let legacy_field = statements[1]
        .expression_payload()
        .expect("legacy host field expression");
    let mismatched_payload = expression_payload_with_fallback(
        source,
        cst_field
            .syntax_expression()
            .expect("CST field syntax")
            .clone(),
        legacy_field.fallback(),
    );
    let compiler = Compiler::new_with_param_defaults(
        payload.name.clone(),
        payload.body.clone(),
        payload.param_defaults.clone(),
        signature,
        bindings,
        facts,
    )
    .expect("compiler should initialize");

    assert!(
        compiler
            .resolve_host_path_with_payload(
                mismatched_payload.fallback(),
                Some(&mismatched_payload)
            )
            .is_none(),
        "misaligned CST field receiver must not resolve the legacy host receiver"
    );
}

#[test]
fn missing_host_field_payload_does_not_use_legacy_field_name() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let legacy_player = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "LegacyPlayer",
            ))
            .host_runtime_id(78),
        )
        .expect("LegacyPlayer host type should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field("host", std::iter::empty::<&str>(), "LegacyPlayer", "level"),
                legacy_player,
            )
            .host_runtime_id(FieldId::new(4).get())
            .writable(true),
        )
        .expect("LegacyPlayer level field should register");

    let source = SourceId::new(1);
    let semantic = parse_semantic_source(
        source,
        r#"
fn main(legacy: LegacyPlayer) {
    make(legacy).level;
}

fn make(value) {
    return value;
}
"#,
    )
    .expect("semantic source should parse");
    let facts = cst_payload_compiler_facts_with_options(
        &semantic,
        CompilerOptions::default(),
        Some(registry.compile_view()),
    );
    let (payload, signature, bindings) = semantic.function("main").expect("main function");
    let legacy_field = paired_statement_payloads_for_body(source, &payload.body)[0]
        .expression_payload()
        .expect("legacy host field expression");
    let compiler = Compiler::new_with_param_defaults(
        payload.name.clone(),
        payload.body.clone(),
        payload.param_defaults.clone(),
        signature,
        bindings,
        facts,
    )
    .expect("compiler should initialize");

    assert!(
        compiler
            .resolve_host_path_with_payload(legacy_field.fallback(), None)
            .is_none(),
        "payload-aware host field paths must not resolve from legacy field names"
    );
}
