use super::*;

#[test]
fn registry_abi_rejections_carry_new_declaration_spans() {
    let schema_span = Span::new(SourceId::new(9), 10, 25);
    let mut old_registry = TypeRegistry::new();
    old_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Reward"))
            .schema_hash(SchemaHash::new(0x1111))
            .source_span(Span::new(SourceId::new(1), 1, 8)),
    );
    let mut new_registry = TypeRegistry::new();
    new_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Reward"))
            .schema_hash(SchemaHash::new(0x2222))
            .source_span(schema_span),
    );

    let error = HotReloadAbi::from_registry(&old_registry)
        .ensure_compatible_update(&HotReloadAbi::from_registry(&new_registry))
        .expect_err("schema hash change should fail");
    assert_eq!(error.source_span(), Some(schema_span));
    let report = HotReloadReport::rejected(ProgramVersionId(1), error);
    assert_eq!(report.errors[0].source_span, Some(schema_span));
    assert!(
        report
            .render_lines()
            .iter()
            .any(|line| line.kind == HotReloadReportLineKind::Diagnostic
                && line.span == Some(schema_span))
    );

    let function_span = Span::new(SourceId::new(10), 30, 50);
    let old_abi = HotReloadAbi::empty().function(FunctionAbi::new(
        "game.reward.grant",
        EffectAbi::host_read(),
        AccessAbi::public(),
    ));
    let new_abi = HotReloadAbi::empty().function(
        FunctionAbi::new(
            "game.reward.grant",
            EffectAbi::host_write(),
            AccessAbi::public(),
        )
        .source_span(function_span),
    );
    let error = old_abi
        .ensure_compatible_update(&new_abi)
        .expect_err("function effect change should fail");
    assert_eq!(error.source_span(), Some(function_span));

    let method_span = Span::new(SourceId::new(11), 60, 75);
    let old_abi = HotReloadAbi::empty().method(MethodAbi::new(
        "Player",
        "grant_exp",
        EffectAbi::host_write(),
        AccessAbi::public(),
    ));
    let new_abi = HotReloadAbi::empty().method(
        MethodAbi::new(
            "Player",
            "grant_exp",
            EffectAbi::host_read(),
            AccessAbi::public(),
        )
        .source_span(method_span),
    );
    let error = old_abi
        .ensure_compatible_update(&new_abi)
        .expect_err("method effect change should fail");
    assert_eq!(error.source_span(), Some(method_span));
}

#[test]
fn trait_abi_manifest_can_be_built_from_type_registry() {
    let mut old_registry = TypeRegistry::new();
    old_registry.register_trait(
        TraitDesc::new("Damageable").method(
            TraitMethodDesc::new(MethodId::new(1), "damage")
                .param(MethodParamDesc::new("amount").type_hint("int"))
                .return_type("int"),
        ),
    );

    let mut reordered_registry = TypeRegistry::new();
    reordered_registry.register_trait(
        TraitDesc::new("Damageable")
            .method(
                TraitMethodDesc::new(MethodId::new(2), "heal")
                    .param(MethodParamDesc::new("amount").type_hint("int"))
                    .return_type("int")
                    .defaulted(true),
            )
            .method(
                TraitMethodDesc::new(MethodId::new(1), "damage")
                    .param(MethodParamDesc::new("amount").type_hint("int"))
                    .return_type("int"),
            ),
    );

    HotReloadAbi::from_registry(&old_registry)
        .ensure_compatible_update(&HotReloadAbi::from_registry(&reordered_registry))
        .expect("reordered trait methods plus defaulted additions should be accepted");

    let mut changed_registry = TypeRegistry::new();
    changed_registry.register_trait(
        TraitDesc::new("Damageable").method(
            TraitMethodDesc::new(MethodId::new(1), "damage")
                .param(MethodParamDesc::new("amount").type_hint("float"))
                .return_type("int"),
        ),
    );
    let error = HotReloadAbi::from_registry(&old_registry)
        .ensure_compatible_update(&HotReloadAbi::from_registry(&changed_registry))
        .expect_err("changed registry trait method ABI should be rejected");
    assert_eq!(error.code(), "reload.trait.changed_abi");
}

#[test]
fn abi_manifest_can_be_built_from_type_registry() {
    let player = TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
        .schema_hash(SchemaHash::new(0xfeed))
        .method(
            MethodDesc::new(HostMethodId::new(9), "grant_exp")
                .param(MethodParamDesc::new("amount").type_hint("int"))
                .return_type("int")
                .effects(MethodEffectSet::host_write())
                .access(
                    MethodAccess::new()
                        .reflect_callable(true)
                        .require_permission("player.write"),
                ),
        );
    let mut registry = TypeRegistry::new();
    registry.register(player);
    registry.register_function(
        FunctionDesc::new(FunctionId::new(11), "game.reward.grant")
            .param(FunctionParamDesc::new("player").type_hint("Player"))
            .param(FunctionParamDesc::new("amount").type_hint("int"))
            .return_type("int")
            .effects(FunctionEffectSet::event_emit())
            .access(
                FunctionAccess::new()
                    .reflect_visible(true)
                    .require_permission("reward.grant"),
            )
            .attr("event", "monster.kill"),
    );

    let abi = HotReloadAbi::from_registry(&registry);
    let initial =
        compile_initial_with_abi(SourceId::new(1), "fn main() { return 1; }", abi.clone())
            .expect("initial");

    compile_update_with_abi(&initial, SourceId::new(2), "fn main() { return 2; }", abi)
        .expect("unchanged registry ABI should be accepted");

    let mut changed_registry = TypeRegistry::new();
    changed_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
            .schema_hash(SchemaHash::new(0xfeed))
            .method(
                MethodDesc::new(HostMethodId::new(9), "grant_exp")
                    .param(MethodParamDesc::new("amount").type_hint("int"))
                    .return_type("int")
                    .effects(MethodEffectSet::host_write())
                    .access(
                        MethodAccess::new()
                            .reflect_callable(true)
                            .require_permission("player.write"),
                    ),
            ),
    );
    changed_registry.register_function(
        FunctionDesc::new(FunctionId::new(11), "game.reward.grant")
            .param(FunctionParamDesc::new("player").type_hint("Player"))
            .param(FunctionParamDesc::new("amount").type_hint("int"))
            .return_type("int")
            .effects(FunctionEffectSet::event_emit())
            .access(
                FunctionAccess::new()
                    .reflect_visible(true)
                    .require_permission("reward.grant"),
            )
            .attr("event", "quest.complete"),
    );

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(3),
        "fn main() { return 3; }",
        HotReloadAbi::from_registry(&changed_registry),
    )
    .expect_err("changed registry event binding should be rejected");
    assert_eq!(error.code(), "reload.function.event_changed");

    let mut changed_param_registry = TypeRegistry::new();
    changed_param_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
            .schema_hash(SchemaHash::new(0xfeed))
            .method(
                MethodDesc::new(HostMethodId::new(9), "grant_exp")
                    .param(MethodParamDesc::new("amount").type_hint("int"))
                    .return_type("int")
                    .effects(MethodEffectSet::host_write())
                    .access(
                        MethodAccess::new()
                            .reflect_callable(true)
                            .require_permission("player.write"),
                    ),
            ),
    );
    changed_param_registry.register_function(
        FunctionDesc::new(FunctionId::new(11), "game.reward.grant")
            .param(FunctionParamDesc::new("player").type_hint("Player"))
            .param(FunctionParamDesc::new("amount").type_hint("float"))
            .return_type("int")
            .effects(FunctionEffectSet::event_emit())
            .access(
                FunctionAccess::new()
                    .reflect_visible(true)
                    .require_permission("reward.grant"),
            )
            .attr("event", "monster.kill"),
    );

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(4),
        "fn main() { return 4; }",
        HotReloadAbi::from_registry(&changed_param_registry),
    )
    .expect_err("changed registry parameter ABI should be rejected");
    assert_eq!(error.code(), "reload.function.parameter_abi_changed");

    let mut changed_method_param_registry = TypeRegistry::new();
    changed_method_param_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
            .schema_hash(SchemaHash::new(0xfeed))
            .method(
                MethodDesc::new(HostMethodId::new(9), "grant_exp")
                    .param(MethodParamDesc::new("amount").type_hint("float"))
                    .return_type("int")
                    .effects(MethodEffectSet::host_write())
                    .access(
                        MethodAccess::new()
                            .reflect_callable(true)
                            .require_permission("player.write"),
                    ),
            ),
    );
    changed_method_param_registry.register_function(
        FunctionDesc::new(FunctionId::new(11), "game.reward.grant")
            .param(FunctionParamDesc::new("player").type_hint("Player"))
            .param(FunctionParamDesc::new("amount").type_hint("int"))
            .return_type("int")
            .effects(FunctionEffectSet::event_emit())
            .access(
                FunctionAccess::new()
                    .reflect_visible(true)
                    .require_permission("reward.grant"),
            )
            .attr("event", "monster.kill"),
    );

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(5),
        "fn main() { return 5; }",
        HotReloadAbi::from_registry(&changed_method_param_registry),
    )
    .expect_err("changed registry method parameter ABI should be rejected");
    assert_eq!(error.code(), "reload.method.parameter_abi_changed");

    let mut changed_function_return_registry = TypeRegistry::new();
    changed_function_return_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
            .schema_hash(SchemaHash::new(0xfeed))
            .method(
                MethodDesc::new(HostMethodId::new(9), "grant_exp")
                    .param(MethodParamDesc::new("amount").type_hint("int"))
                    .return_type("int")
                    .effects(MethodEffectSet::host_write())
                    .access(
                        MethodAccess::new()
                            .reflect_callable(true)
                            .require_permission("player.write"),
                    ),
            ),
    );
    changed_function_return_registry.register_function(
        FunctionDesc::new(FunctionId::new(11), "game.reward.grant")
            .param(FunctionParamDesc::new("player").type_hint("Player"))
            .param(FunctionParamDesc::new("amount").type_hint("int"))
            .return_type("float")
            .effects(FunctionEffectSet::event_emit())
            .access(
                FunctionAccess::new()
                    .reflect_visible(true)
                    .require_permission("reward.grant"),
            )
            .attr("event", "monster.kill"),
    );

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(6),
        "fn main() { return 6; }",
        HotReloadAbi::from_registry(&changed_function_return_registry),
    )
    .expect_err("changed registry function return ABI should be rejected");
    assert_eq!(error.code(), "reload.function.return_abi_changed");

    let mut changed_method_return_registry = TypeRegistry::new();
    changed_method_return_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
            .schema_hash(SchemaHash::new(0xfeed))
            .method(
                MethodDesc::new(HostMethodId::new(9), "grant_exp")
                    .param(MethodParamDesc::new("amount").type_hint("int"))
                    .return_type("float")
                    .effects(MethodEffectSet::host_write())
                    .access(
                        MethodAccess::new()
                            .reflect_callable(true)
                            .require_permission("player.write"),
                    ),
            ),
    );
    changed_method_return_registry.register_function(
        FunctionDesc::new(FunctionId::new(11), "game.reward.grant")
            .param(FunctionParamDesc::new("player").type_hint("Player"))
            .param(FunctionParamDesc::new("amount").type_hint("int"))
            .return_type("int")
            .effects(FunctionEffectSet::event_emit())
            .access(
                FunctionAccess::new()
                    .reflect_visible(true)
                    .require_permission("reward.grant"),
            )
            .attr("event", "monster.kill"),
    );

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(7),
        "fn main() { return 7; }",
        HotReloadAbi::from_registry(&changed_method_return_registry),
    )
    .expect_err("changed registry method return ABI should be rejected");
    assert_eq!(error.code(), "reload.method.return_abi_changed");
}
