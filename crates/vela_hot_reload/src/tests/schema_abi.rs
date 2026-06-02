use super::*;

#[test]
fn schema_abi_changes_are_rejected() {
    let old_abi = HotReloadAbi::empty().schema(SchemaAbi::new("Reward", SchemaHash::new(0x1111)));
    let new_abi = HotReloadAbi::empty().schema(SchemaAbi::new("Reward", SchemaHash::new(0x2222)));
    let initial = compile_initial_with_abi(SourceId::new(1), "fn main() { return 1; }", old_abi)
        .expect("initial");

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(2),
        "fn main() { return 2; }",
        new_abi,
    )
    .expect_err("schema change should fail");

    assert_eq!(
        error.kind,
        HotReloadErrorKind::ChangedSchema {
            type_name: "Reward".to_owned(),
            old_hash: 0x1111,
            new_hash: 0x2222,
            source_span: None,
        }
    );
}

#[test]
fn removed_schema_abi_is_rejected() {
    let old_abi = HotReloadAbi::empty().schema(SchemaAbi::new("Reward", SchemaHash::new(0x1111)));
    let initial = compile_initial_with_abi(SourceId::new(1), "fn main() { return 1; }", old_abi)
        .expect("initial");

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(2),
        "fn main() { return 2; }",
        HotReloadAbi::empty(),
    )
    .expect_err("removed schema should fail");

    assert_eq!(
        error.kind,
        HotReloadErrorKind::RemovedSchema {
            type_name: "Reward".to_owned(),
            old_hash: 0x1111,
            source_span: None,
        }
    );
}

#[test]
fn registry_schema_abi_accepts_defaulted_field_additions() {
    let mut old_registry = TypeRegistry::new();
    old_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Reward"))
            .kind(TypeKind::ScriptStruct)
            .schema_hash(SchemaHash::new(0x1111))
            .field(FieldDesc::new(FieldId::new(1), "item_id").type_hint("string"))
            .field(FieldDesc::new(FieldId::new(2), "count").type_hint("int")),
    );

    let mut new_registry = TypeRegistry::new();
    new_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Reward"))
            .kind(TypeKind::ScriptStruct)
            .schema_hash(SchemaHash::new(0x2222))
            .field(
                FieldDesc::new(FieldId::new(3), "rarity")
                    .type_hint("string")
                    .defaulted(true),
            )
            .field(FieldDesc::new(FieldId::new(2), "count").type_hint("int"))
            .field(FieldDesc::new(FieldId::new(1), "item_id").type_hint("string")),
    );

    HotReloadAbi::from_registry(&old_registry)
        .ensure_compatible_update(&HotReloadAbi::from_registry(&new_registry))
        .expect("defaulted field additions and reordering should be compatible");
}

#[test]
fn registry_schema_abi_accepts_stable_id_field_and_variant_renames() {
    let mut old_registry = TypeRegistry::new();
    old_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Reward"))
            .kind(TypeKind::ScriptStruct)
            .schema_hash(SchemaHash::new(0x1111))
            .field(FieldDesc::new(FieldId::new(1), "item_id").type_hint("string"))
            .field(FieldDesc::new(FieldId::new(2), "count").type_hint("int")),
    );
    old_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(2), "QuestProgress"))
            .kind(TypeKind::ScriptEnum)
            .schema_hash(SchemaHash::new(0xaaaa))
            .variant(
                VariantDesc::new(VariantId::new(1), "Active")
                    .field(FieldDesc::new(FieldId::new(1), "quest_id").type_hint("string")),
            ),
    );

    let mut new_registry = TypeRegistry::new();
    new_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Reward"))
            .kind(TypeKind::ScriptStruct)
            .schema_hash(SchemaHash::new(0x2222))
            .field(FieldDesc::new(FieldId::new(1), "item").type_hint("string"))
            .field(FieldDesc::new(FieldId::new(2), "quantity").type_hint("int")),
    );
    new_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(2), "QuestProgress"))
            .kind(TypeKind::ScriptEnum)
            .schema_hash(SchemaHash::new(0xbbbb))
            .variant(
                VariantDesc::new(VariantId::new(1), "Started")
                    .field(FieldDesc::new(FieldId::new(1), "quest").type_hint("string")),
            )
            .variant(VariantDesc::new(VariantId::new(2), "Finished")),
    );

    HotReloadAbi::from_registry(&old_registry)
        .ensure_compatible_update(&HotReloadAbi::from_registry(&new_registry))
        .expect("field and variant renames with stable IDs should be compatible");
}

#[test]
fn registry_schema_abi_rejects_existing_field_or_variant_id_changes() {
    let mut old_registry = TypeRegistry::new();
    old_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Reward"))
            .kind(TypeKind::ScriptStruct)
            .schema_hash(SchemaHash::new(0x1111))
            .field(FieldDesc::new(FieldId::new(1), "item_id").type_hint("string")),
    );

    let mut changed_field_registry = TypeRegistry::new();
    changed_field_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Reward"))
            .kind(TypeKind::ScriptStruct)
            .schema_hash(SchemaHash::new(0x2222))
            .field(FieldDesc::new(FieldId::new(2), "item_id").type_hint("string")),
    );

    let error = HotReloadAbi::from_registry(&old_registry)
        .ensure_compatible_update(&HotReloadAbi::from_registry(&changed_field_registry))
        .expect_err("changed field ID for an existing field name should fail");
    assert_eq!(error.code(), "reload.schema.abi_changed");

    let mut old_enum_registry = TypeRegistry::new();
    old_enum_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(2), "QuestProgress"))
            .kind(TypeKind::ScriptEnum)
            .schema_hash(SchemaHash::new(0xaaaa))
            .variant(VariantDesc::new(VariantId::new(1), "Active")),
    );

    let mut changed_variant_registry = TypeRegistry::new();
    changed_variant_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(2), "QuestProgress"))
            .kind(TypeKind::ScriptEnum)
            .schema_hash(SchemaHash::new(0xbbbb))
            .variant(VariantDesc::new(VariantId::new(2), "Active")),
    );

    let error = HotReloadAbi::from_registry(&old_enum_registry)
        .ensure_compatible_update(&HotReloadAbi::from_registry(&changed_variant_registry))
        .expect_err("changed variant ID for an existing variant name should fail");
    assert_eq!(error.code(), "reload.schema.abi_changed");
}

#[test]
fn registry_schema_abi_rejects_required_field_additions() {
    let span = Span::new(SourceId::new(19), 20, 80);
    let mut old_registry = TypeRegistry::new();
    old_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Reward"))
            .kind(TypeKind::ScriptStruct)
            .schema_hash(SchemaHash::new(0x1111))
            .field(FieldDesc::new(FieldId::new(1), "item_id").type_hint("string")),
    );

    let mut new_registry = TypeRegistry::new();
    new_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Reward"))
            .kind(TypeKind::ScriptStruct)
            .schema_hash(SchemaHash::new(0x2222))
            .field(FieldDesc::new(FieldId::new(1), "item_id").type_hint("string"))
            .field(FieldDesc::new(FieldId::new(2), "count").type_hint("int"))
            .source_span(span),
    );

    let error = HotReloadAbi::from_registry(&old_registry)
        .ensure_compatible_update(&HotReloadAbi::from_registry(&new_registry))
        .expect_err("required field addition should fail");
    assert_eq!(error.code(), "reload.schema.abi_changed");
    assert_eq!(error.source_span(), Some(span));
    let report = HotReloadReport::rejected(ProgramVersionId(19), error);
    assert_eq!(report.errors[0].target.as_deref(), Some("Reward"));
    assert!(matches!(
        report.errors[0].detail,
        Some(HotReloadDiagnosticDetail::SchemaMemberAbi { .. })
    ));
    assert!(report.render_lines().iter().any(|line| {
        line.text
            .contains("schema ABI: old=(kind=script_struct hash=4369 fields=[item_id#1:string]")
    }));
}

#[test]
fn registry_schema_abi_rejects_existing_member_changes() {
    let mut old_registry = TypeRegistry::new();
    old_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(2), "QuestProgress"))
            .kind(TypeKind::ScriptEnum)
            .schema_hash(SchemaHash::new(0xaaaa))
            .variant(
                VariantDesc::new(VariantId::new(1), "Active")
                    .field(FieldDesc::new(FieldId::new(1), "count").type_hint("int")),
            ),
    );

    let mut new_registry = TypeRegistry::new();
    new_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(2), "QuestProgress"))
            .kind(TypeKind::ScriptEnum)
            .schema_hash(SchemaHash::new(0xbbbb))
            .variant(
                VariantDesc::new(VariantId::new(1), "Active")
                    .field(FieldDesc::new(FieldId::new(1), "count").type_hint("float")),
            ),
    );

    let error = HotReloadAbi::from_registry(&old_registry)
        .ensure_compatible_update(&HotReloadAbi::from_registry(&new_registry))
        .expect_err("existing variant field type change should fail");
    assert_eq!(error.code(), "reload.schema.abi_changed");
    let report = HotReloadReport::rejected(ProgramVersionId(20), error);
    assert!(
        report
            .render_lines()
            .iter()
            .any(|line| line.text.contains("variants=[Active#1(count#1:int)]"))
    );
}

#[test]
fn registry_schema_abi_tracks_trait_impls_even_with_stable_hashes() {
    let mut old_registry = TypeRegistry::new();
    old_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(3), "Player"))
            .kind(TypeKind::ScriptStruct)
            .schema_hash(SchemaHash::new(0x3333))
            .field(FieldDesc::new(FieldId::new(1), "level").type_hint("int"))
            .trait_impl(
                TraitDesc::new("Damageable").method(
                    TraitMethodDesc::new(MethodId::new(1), "damage")
                        .param(MethodParamDesc::new("amount").type_hint("int")),
                ),
            ),
    );

    let mut removed_impl_registry = TypeRegistry::new();
    removed_impl_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(3), "Player"))
            .kind(TypeKind::ScriptStruct)
            .schema_hash(SchemaHash::new(0x3333))
            .field(FieldDesc::new(FieldId::new(1), "level").type_hint("int")),
    );
    let error = HotReloadAbi::from_registry(&old_registry)
        .ensure_compatible_update(&HotReloadAbi::from_registry(&removed_impl_registry))
        .expect_err("removed trait implementation should fail");
    assert_eq!(error.code(), "reload.schema.abi_changed");
    let report = HotReloadReport::rejected(ProgramVersionId(21), error);
    assert!(
        report
            .render_lines()
            .iter()
            .any(|line| line.text.contains("traits=[Damageable#"))
    );

    let mut added_impl_registry = TypeRegistry::new();
    added_impl_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(3), "Player"))
            .kind(TypeKind::ScriptStruct)
            .schema_hash(SchemaHash::new(0x3333))
            .field(FieldDesc::new(FieldId::new(1), "level").type_hint("int"))
            .trait_impl(
                TraitDesc::new("Damageable").method(
                    TraitMethodDesc::new(MethodId::new(1), "damage")
                        .param(MethodParamDesc::new("amount").type_hint("int")),
                ),
            )
            .trait_impl(TraitDesc::new("Trackable")),
    );
    HotReloadAbi::from_registry(&old_registry)
        .ensure_compatible_update(&HotReloadAbi::from_registry(&added_impl_registry))
        .expect("added trait implementations should be compatible");
}
