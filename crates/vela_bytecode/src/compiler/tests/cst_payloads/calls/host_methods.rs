use super::*;

#[test]
fn method_call_with_non_field_cst_callee_does_not_use_legacy_method_name() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let callable = |value| value;
    let cst_call = ({
        let selected = callable;
        selected
    })(1);
    let legacy_call = "ready".len();
}
"#,
        |compiler, payload| {
            let statements = call_statement_payloads(&payload.body);
            let cst_call = statements[1]
                .let_initializer_expression_payload()
                .expect("CST call payload");
            let legacy_call = statements[2]
                .let_initializer_expression_payload()
                .expect("legacy method call fallback");
            let mismatched_payload = expression_payload_with_fallback(
                SourceId::new(1),
                cst_call
                    .syntax_expression()
                    .expect("CST expression")
                    .clone(),
                legacy_call.fallback(),
            );

            let error = compiler
                .compile_expr_with_payload(mismatched_payload.fallback(), Some(&mismatched_payload))
                .expect_err("mismatched non-field CST callee must not compile");

            assert!(
                matches!(
                    error.kind,
                    CompileErrorKind::UnknownLocal(ref name) if name == "callable"
                ),
                "expected CST closure callee to read `callable`, got {error:?}"
            );
        },
    );
}

#[test]
fn host_path_push_with_non_field_cst_callee_does_not_use_legacy_method_name() {
    let inventory = FieldId::new(3);
    let rewards = FieldId::new(4);
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
                DefPath::field("host", std::iter::empty::<&str>(), "Inventory", "rewards"),
                inventory_type,
            )
            .host_runtime_id(rewards.get())
            .writable(true),
        )
        .expect("Inventory rewards field should register");

    let source = SourceId::new(1);
    let semantic = parse_semantic_source(
        source,
        r#"
fn main(player: Player) {
    let callable = |value| value;
    let cst_call = ({
        let selected = callable;
        selected
    })("silver");
    let legacy_call = player.inventory.rewards.push("gold");
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
    let statements = call_statement_payloads(&payload.body);
    let cst_call = statements[1]
        .let_initializer_expression_payload()
        .expect("CST call payload");
    let legacy_call = statements[2]
        .let_initializer_expression_payload()
        .expect("legacy host push call fallback");
    let mismatched_payload = expression_payload_with_fallback(
        source,
        cst_call
            .syntax_expression()
            .expect("CST expression")
            .clone(),
        legacy_call.fallback(),
    );
    let mut compiler = Compiler::new_with_param_defaults(
        payload.name.clone(),
        payload.body.clone(),
        payload.param_defaults.clone(),
        signature,
        bindings,
        facts,
    )
    .expect("compiler should initialize");

    let error = compiler
        .compile_expr_with_payload(mismatched_payload.fallback(), Some(&mismatched_payload))
        .expect_err("mismatched host push fallback must not compile");

    assert!(
        matches!(
            error.kind,
            CompileErrorKind::UnknownLocal(ref name) if name == "callable"
        ),
        "expected CST closure callee to read `callable`, got {error:?}"
    );
}

#[test]
fn host_method_call_requires_cst_callee_payload() {
    let method = HostMethodId::new(5);
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty("host", std::iter::empty::<&str>(), "Player"))
                .host_runtime_id(77),
        )
        .expect("Player host type should register");
    registry
        .register_method(
            vela_registry::MethodDef::new(
                DefPath::method("host", std::iter::empty::<&str>(), "Player", "grant_exp"),
                player,
                vela_registry::FunctionSignature::new(
                    [vela_registry::ParamDef::new("amount", Some("i64"))],
                    None::<vela_registry::TypeHintDef>,
                ),
            )
            .host_runtime_id(method.get()),
        )
        .expect("Player grant_exp method should register");

    let source = SourceId::new(1);
    let semantic = parse_semantic_source(
        source,
        r#"
fn main(player: Player) {
    let result = player.grant_exp(20);
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
    let call_payload = call_statement_payloads(&payload.body)[0]
        .let_initializer_expression_payload()
        .expect("host method call payload");
    let mut compiler = Compiler::new_with_param_defaults(
        payload.name.clone(),
        payload.body.clone(),
        payload.param_defaults.clone(),
        signature,
        bindings,
        facts,
    )
    .expect("compiler should initialize");

    let error = compiler
        .compile_expr_with_payload(call_payload.fallback(), None)
        .expect_err("missing CST call callee payload must not compile legacy host method call");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST call expression payload")
    ));
}

#[test]
fn host_collection_method_targets_use_cst_receiver_roots() {
    fn host_type(
        registry: &mut vela_registry::DefinitionRegistry,
        name: &str,
        id: u32,
    ) -> vela_def::TypeId {
        registry
            .register_type(
                vela_registry::TypeDef::new(DefPath::ty("host", std::iter::empty::<&str>(), name))
                    .host_runtime_id(id.into()),
            )
            .expect("host type should register")
    }

    fn host_field(
        registry: &mut vela_registry::DefinitionRegistry,
        owner: vela_def::TypeId,
        owner_name: &str,
        name: &str,
        id: FieldId,
        type_hint: Option<&str>,
    ) {
        let mut field = vela_registry::FieldDef::new(
            DefPath::field("host", std::iter::empty::<&str>(), owner_name, name),
            owner,
        )
        .host_runtime_id(id.get())
        .writable(true);
        if let Some(type_hint) = type_hint {
            field = field.type_hint(Some(type_hint.to_owned()));
        }
        registry
            .register_field(field)
            .expect("host field should register");
    }

    let cst_inventory = FieldId::new(3);
    let cst_items = FieldId::new(4);
    let legacy_inventory = FieldId::new(5);
    let legacy_items = FieldId::new(6);
    let mut registry = vela_registry::DefinitionRegistry::new();
    let cst_player = host_type(&mut registry, "CstPlayer", 77);
    let cst_inventory_type = host_type(&mut registry, "CstInventory", 78);
    let legacy_player = host_type(&mut registry, "LegacyPlayer", 79);
    let legacy_inventory_type = host_type(&mut registry, "LegacyInventory", 80);
    host_field(
        &mut registry,
        cst_player,
        "CstPlayer",
        "inventory",
        cst_inventory,
        Some("CstInventory"),
    );
    host_field(
        &mut registry,
        cst_inventory_type,
        "CstInventory",
        "items",
        cst_items,
        Some("CstItems"),
    );
    host_field(
        &mut registry,
        legacy_player,
        "LegacyPlayer",
        "inventory",
        legacy_inventory,
        Some("LegacyInventory"),
    );
    host_field(
        &mut registry,
        legacy_inventory_type,
        "LegacyInventory",
        "items",
        legacy_items,
        Some("LegacyItems"),
    );

    let source = SourceId::new(1);
    let semantic = parse_semantic_source(
        source,
        r#"
fn main(cst: CstPlayer, legacy: LegacyPlayer) {
    let cst_call = cst.inventory.items.remove();
    let legacy_call = legacy.inventory.items.remove();
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
    let statements = call_statement_payloads(&payload.body);
    let cst_call = statements[0]
        .let_initializer_expression_payload()
        .expect("CST remove call payload");
    let legacy_call = statements[1]
        .let_initializer_expression_payload()
        .expect("legacy remove call fallback");
    let ExprKind::Call { callee, .. } = &legacy_call.fallback().kind else {
        panic!("expected legacy remove call fallback");
    };
    let mismatched_payload = expression_payload_with_fallback(
        source,
        cst_call
            .syntax_expression()
            .expect("CST expression")
            .clone(),
        legacy_call.fallback(),
    );
    let callee_payload = mismatched_payload
        .call_callee_payload()
        .expect("mismatched callee payload");
    let compiler = Compiler::new_with_param_defaults(
        payload.name.clone(),
        payload.body.clone(),
        payload.param_defaults.clone(),
        signature,
        bindings,
        facts,
    )
    .expect("compiler should initialize");

    assert_eq!(
        compiler.host_collection_method_target_root_name_for_test(
            callee,
            Some(&callee_payload),
            "remove",
        ),
        Some("cst".to_owned()),
        "collection host path root must come from the CST receiver payload"
    );
}
