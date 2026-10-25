use super::*;

#[test]
fn reflect_get_record_field_reads_value() {
    let registry = TypeRegistry::new();
    let mut record = BTreeMap::new();
    record.insert(
        "field".to_owned(),
        ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(42))),
    );
    let mut ctx = ReflectContext {
        registry: &registry,
    };

    let value = get(&mut ctx, &ReflectValue::Record(record), "field").expect("record get");

    assert_eq!(
        value,
        ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(42)))
    );
}

#[test]
fn reflect_get_map_string_key_reads_key_preserving_map() {
    let registry = TypeRegistry::new();
    let map = ReflectValue::Map(vec![
        crate::value::ReflectMapEntry::new(
            ReflectValue::Host(HostValue::String("field".to_owned())),
            ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(42))),
        ),
        crate::value::ReflectMapEntry::new(
            ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(1))),
            ReflectValue::Host(HostValue::String("one".to_owned())),
        ),
    ]);
    let mut ctx = ReflectContext {
        registry: &registry,
    };

    let value = get(&mut ctx, &map, "field").expect("map string-key get");

    assert_eq!(
        value,
        ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(42)))
    );
}

#[test]
fn reflect_get_script_record_unknown_field_uses_schema_candidates() {
    let field_span = Span::new(SourceId::new(7), 20, 25);
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(200), "Player"))
            .kind(TypeKind::ScriptStruct)
            .field(FieldDesc::new(FieldId::new(2), "level").source_span(field_span)),
    );
    let mut fields = BTreeMap::new();
    fields.insert(
        "level".to_owned(),
        ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(7))),
    );
    let record = ReflectValue::ScriptRecord {
        type_name: "Player".to_owned(),
        fields,
    };
    let mut ctx = ReflectContext {
        registry: &registry,
    };

    let error = get(&mut ctx, &record, "leve").expect_err("unknown field");

    assert_eq!(
        error.kind,
        ReflectErrorKind::UnknownField {
            type_name: "Player".to_owned(),
            field: "leve".to_owned(),
            candidates: vec!["level".to_owned()],
            related: vec![ReflectCandidate::new("level", Some(field_span))],
        }
    );
}

#[test]
fn reflect_set_script_record_returns_updated_copy() {
    let registry = TypeRegistry::new();
    let mut fields = BTreeMap::new();
    fields.insert(
        "level".to_owned(),
        ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(7))),
    );
    fields.insert(
        "name".to_owned(),
        ReflectValue::Host(HostValue::String("hero".to_owned())),
    );
    let record = ReflectValue::ScriptRecord {
        type_name: "Player".to_owned(),
        fields,
    };
    let mut ctx = ReflectContext {
        registry: &registry,
    };

    let updated = set(
        &mut ctx,
        &record,
        "level",
        ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(10))),
    )
    .expect("script record set");

    let ReflectValue::ScriptRecord { fields, .. } = updated else {
        panic!("script record set should return an updated record");
    };
    assert_eq!(
        fields.get("level"),
        Some(&ReflectValue::Host(HostValue::Scalar(
            vela_common::ScalarValue::I64(10)
        )))
    );
    assert_eq!(
        get(&mut ctx, &record, "level").expect("original record remains readable"),
        ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(7)))
    );
}

#[test]
fn reflect_set_metadata_record_is_not_a_schema_mutation_path() {
    let registry = TypeRegistry::new();
    let record = ReflectValue::ScriptRecord {
        type_name: "ReflectType".to_owned(),
        fields: BTreeMap::from([(
            "name".to_owned(),
            ReflectValue::Host(HostValue::String("Player".to_owned())),
        )]),
    };
    let mut ctx = ReflectContext {
        registry: &registry,
    };

    let error = set(
        &mut ctx,
        &record,
        "name",
        ReflectValue::Host(HostValue::String("Monster".to_owned())),
    )
    .expect_err("reflection metadata records are not writable");

    assert_eq!(error.kind, ReflectErrorKind::InvalidTarget);
}

#[test]
fn reflect_set_script_record_rejects_unknown_fields() {
    let field_span = Span::new(SourceId::new(7), 20, 25);
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(200), "Player"))
            .kind(TypeKind::ScriptStruct)
            .field(FieldDesc::new(FieldId::new(2), "level").source_span(field_span)),
    );
    let mut fields = BTreeMap::new();
    fields.insert(
        "level".to_owned(),
        ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(7))),
    );
    let record = ReflectValue::ScriptRecord {
        type_name: "Player".to_owned(),
        fields,
    };
    let mut ctx = ReflectContext {
        registry: &registry,
    };

    let error = set(
        &mut ctx,
        &record,
        "leve",
        ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(10))),
    )
    .expect_err("unknown script record field should fail");

    assert_eq!(
        error.kind,
        ReflectErrorKind::UnknownField {
            type_name: "Player".to_owned(),
            field: "leve".to_owned(),
            candidates: vec!["level".to_owned()],
            related: vec![ReflectCandidate::new("level", Some(field_span))],
        }
    );
}

#[test]
fn reflect_get_and_set_with_policy_require_script_field_permission() {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(200), "Player"))
            .kind(TypeKind::ScriptStruct)
            .field(
                FieldDesc::new(FieldId::new(2), "level").access(
                    FieldAccess::new()
                        .writable(true)
                        .reflect_writable(true)
                        .require_permission("player.level.reflect"),
                ),
            ),
    );
    let mut fields = BTreeMap::new();
    fields.insert(
        "level".to_owned(),
        ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(7))),
    );
    let record = ReflectValue::ScriptRecord {
        type_name: "Player".to_owned(),
        fields,
    };
    let mut ctx = ReflectContext {
        registry: &registry,
    };

    let error = get_with_policy(&mut ctx, &record, "level", &ReflectPolicy::all())
        .expect_err("missing script field read permission");
    assert_eq!(
        error.kind,
        ReflectErrorKind::FieldPermissionDenied {
            type_name: "Player".to_owned(),
            field: "level".to_owned(),
            permission: "player.level.reflect".to_owned(),
            source_span: None,
        }
    );

    let error = set_with_policy(
        &mut ctx,
        &record,
        "level",
        ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(10))),
        &ReflectPolicy::all(),
    )
    .expect_err("missing script field write permission");
    assert_eq!(
        error.kind,
        ReflectErrorKind::FieldPermissionDenied {
            type_name: "Player".to_owned(),
            field: "level".to_owned(),
            permission: "player.level.reflect".to_owned(),
            source_span: None,
        }
    );

    let policy = ReflectPolicy::all().with_field_permission("player.level.reflect");
    assert_eq!(
        get_with_policy(&mut ctx, &record, "level", &policy).expect("script field read"),
        ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(7)))
    );
    assert_eq!(
        set_with_policy(
            &mut ctx,
            &record,
            "level",
            ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(10))),
            &policy,
        )
        .expect("script field write"),
        ReflectValue::ScriptRecord {
            type_name: "Player".to_owned(),
            fields: BTreeMap::from([(
                "level".to_owned(),
                ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(10))),
            )]),
        }
    );
}

#[test]
fn reflect_set_with_policy_denies_non_reflect_writable_script_fields() {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(203), "Player"))
            .kind(TypeKind::ScriptStruct)
            .field(
                FieldDesc::new(FieldId::new(2), "level")
                    .access(FieldAccess::new().writable(true).reflect_writable(false)),
            ),
    );
    let record = ReflectValue::ScriptRecord {
        type_name: "Player".to_owned(),
        fields: BTreeMap::from([(
            "level".to_owned(),
            ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(7))),
        )]),
    };
    let mut ctx = ReflectContext {
        registry: &registry,
    };

    let error = set_with_policy(
        &mut ctx,
        &record,
        "level",
        ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(10))),
        &ReflectPolicy::all(),
    )
    .expect_err("script field should not be reflect writable");

    assert_eq!(
        error.kind,
        ReflectErrorKind::FieldNotReflectWritable {
            type_name: "Player".to_owned(),
            field: "level".to_owned(),
            source_span: None,
        }
    );
}

#[test]
fn reflect_get_with_policy_filters_unknown_script_field_candidates() {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(201), "Player"))
            .kind(TypeKind::ScriptStruct)
            .field(FieldDesc::new(FieldId::new(1), "level"))
            .field(
                FieldDesc::new(FieldId::new(2), "level_secret")
                    .access(FieldAccess::new().reflect_readable(false)),
            )
            .field(
                FieldDesc::new(FieldId::new(3), "level_admin")
                    .access(FieldAccess::new().require_permission("player.level.admin")),
            ),
    );
    let record = ReflectValue::ScriptRecord {
        type_name: "Player".to_owned(),
        fields: BTreeMap::from([(
            "level".to_owned(),
            ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(7))),
        )]),
    };
    let mut ctx = ReflectContext {
        registry: &registry,
    };

    let error = get_with_policy(
        &mut ctx,
        &record,
        "level_secrett",
        &ReflectPolicy::read_only(),
    )
    .expect_err("unknown script field");

    assert_eq!(
        error.kind,
        ReflectErrorKind::UnknownField {
            type_name: "Player".to_owned(),
            field: "level_secrett".to_owned(),
            candidates: vec!["level".to_owned()],
            related: vec![ReflectCandidate::new("level", None)],
        }
    );
}

#[test]
fn reflect_set_with_policy_filters_unknown_script_field_candidates() {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(205), "Player"))
            .kind(TypeKind::ScriptStruct)
            .field(FieldDesc::new(FieldId::new(1), "level").writable(true))
            .field(
                FieldDesc::new(FieldId::new(2), "level_secret")
                    .access(FieldAccess::new().writable(true).reflect_writable(false)),
            )
            .field(
                FieldDesc::new(FieldId::new(3), "level_admin").access(
                    FieldAccess::new()
                        .writable(true)
                        .reflect_writable(true)
                        .require_permission("player.level.admin"),
                ),
            ),
    );
    let record = ReflectValue::ScriptRecord {
        type_name: "Player".to_owned(),
        fields: BTreeMap::from([(
            "level".to_owned(),
            ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(7))),
        )]),
    };
    let mut ctx = ReflectContext {
        registry: &registry,
    };

    let error = set_with_policy(
        &mut ctx,
        &record,
        "level_secrett",
        ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(10))),
        &ReflectPolicy::read_only(),
    )
    .expect_err("unknown script field");

    assert_eq!(
        error.kind,
        ReflectErrorKind::UnknownField {
            type_name: "Player".to_owned(),
            field: "level_secrett".to_owned(),
            candidates: vec!["level".to_owned()],
            related: vec![ReflectCandidate::new("level", None)],
        }
    );
}

#[test]
fn reflect_get_with_policy_filters_unknown_script_enum_field_candidates() {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(202), "QuestProgress"))
            .kind(TypeKind::ScriptEnum)
            .variant(
                VariantDesc::new(VariantId::new(1), "Active")
                    .field(FieldDesc::new(FieldId::new(1), "count"))
                    .field(
                        FieldDesc::new(FieldId::new(2), "count_secret")
                            .access(FieldAccess::new().reflect_readable(false)),
                    )
                    .field(
                        FieldDesc::new(FieldId::new(3), "count_admin")
                            .access(FieldAccess::new().require_permission("quest.count.admin")),
                    ),
            ),
    );
    let value = ReflectValue::ScriptEnum {
        enum_name: "QuestProgress".to_owned(),
        variant: "Active".to_owned(),
        fields: BTreeMap::from([(
            "count".to_owned(),
            ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(7))),
        )]),
    };
    let mut ctx = ReflectContext {
        registry: &registry,
    };

    let error = get_with_policy(
        &mut ctx,
        &value,
        "count_secrett",
        &ReflectPolicy::read_only(),
    )
    .expect_err("unknown script enum field");

    assert_eq!(
        error.kind,
        ReflectErrorKind::UnknownField {
            type_name: "QuestProgress::Active".to_owned(),
            field: "count_secrett".to_owned(),
            candidates: vec!["count".to_owned()],
            related: vec![ReflectCandidate::new("count", None)],
        }
    );
}

#[test]
fn reflect_set_with_policy_denies_non_reflect_writable_script_enum_fields() {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(204), "QuestProgress"))
            .kind(TypeKind::ScriptEnum)
            .variant(
                VariantDesc::new(VariantId::new(1), "Active").field(
                    FieldDesc::new(FieldId::new(1), "count")
                        .access(FieldAccess::new().writable(true).reflect_writable(false)),
                ),
            ),
    );
    let value = ReflectValue::ScriptEnum {
        enum_name: "QuestProgress".to_owned(),
        variant: "Active".to_owned(),
        fields: BTreeMap::from([(
            "count".to_owned(),
            ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(7))),
        )]),
    };
    let mut ctx = ReflectContext {
        registry: &registry,
    };

    let error = set_with_policy(
        &mut ctx,
        &value,
        "count",
        ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(10))),
        &ReflectPolicy::all(),
    )
    .expect_err("script enum field should not be reflect writable");

    assert_eq!(
        error.kind,
        ReflectErrorKind::FieldNotReflectWritable {
            type_name: "QuestProgress::Active".to_owned(),
            field: "count".to_owned(),
            source_span: None,
        }
    );
}

#[test]
fn unknown_fields_include_candidate_hints() {
    let registry = registry();
    let mut ctx = ReflectContext {
        registry: &registry,
    };

    let error = get(&mut ctx, &player_value(), "levle").expect_err("unknown field");

    assert_eq!(
        error.kind,
        ReflectErrorKind::UnknownField {
            type_name: "Player".to_owned(),
            field: "levle".to_owned(),
            candidates: vec!["level".to_owned(), "id".to_owned()],
            related: vec![
                ReflectCandidate::new("level", None),
                ReflectCandidate::new("id", None),
            ],
        }
    );
}

#[test]
fn type_registry_exposes_value_type_fields() {
    let registry = registry();
    let desc = type_of(&registry, &player_value()).expect("type desc");
    let fields = fields(&registry, &desc.key).expect("fields");

    assert_eq!(desc.key.name, "Player");
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[1].name, "level");
}
