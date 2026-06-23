use super::*;

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
        selected
    };
    player.level;
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
    let statements = payload.body.statement_payloads();
    let cst_block = statements[0]
        .let_initializer_expression_payload()
        .expect("CST block initializer");
    let legacy_path = statements[1]
        .expression_payload()
        .expect("legacy host path expression");
    let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
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
        selected
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
    let statements = payload.body.statement_payloads();
    let cst_block = statements[0]
        .let_initializer_expression_payload()
        .expect("CST block initializer");
    let legacy_index = statements[1]
        .expression_payload()
        .expect("legacy host index expression");
    let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
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
