use super::*;

#[test]
fn reflect_get_record_field_reads_value() {
    let registry = TypeRegistry::new();
    let adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut record = BTreeMap::new();
    record.insert("field".to_owned(), ReflectValue::Host(HostValue::Int(42)));
    let mut ctx = ReflectContext {
        registry: &registry,
        adapter: &adapter,
        tx: &mut tx,
    };

    let value = get(&mut ctx, &ReflectValue::Record(record), "field").expect("record get");

    assert_eq!(value, ReflectValue::Host(HostValue::Int(42)));
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
    let adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut fields = BTreeMap::new();
    fields.insert("level".to_owned(), ReflectValue::Host(HostValue::Int(7)));
    let record = ReflectValue::ScriptRecord {
        type_name: "Player".to_owned(),
        fields,
    };
    let mut ctx = ReflectContext {
        registry: &registry,
        adapter: &adapter,
        tx: &mut tx,
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
    let adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut fields = BTreeMap::new();
    fields.insert("level".to_owned(), ReflectValue::Host(HostValue::Int(7)));
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
        adapter: &adapter,
        tx: &mut tx,
    };

    let updated = set(
        &mut ctx,
        &record,
        "level",
        ReflectValue::Host(HostValue::Int(10)),
    )
    .expect("script record set");

    let ReflectValue::ScriptRecord { fields, .. } = updated else {
        panic!("script record set should return an updated record");
    };
    assert_eq!(
        fields.get("level"),
        Some(&ReflectValue::Host(HostValue::Int(10)))
    );
    assert_eq!(
        get(&mut ctx, &record, "level").expect("original record remains readable"),
        ReflectValue::Host(HostValue::Int(7))
    );
    assert!(ctx.tx.patches().is_empty());
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
    let adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut fields = BTreeMap::new();
    fields.insert("level".to_owned(), ReflectValue::Host(HostValue::Int(7)));
    let record = ReflectValue::ScriptRecord {
        type_name: "Player".to_owned(),
        fields,
    };
    let mut ctx = ReflectContext {
        registry: &registry,
        adapter: &adapter,
        tx: &mut tx,
    };

    let error = set(
        &mut ctx,
        &record,
        "leve",
        ReflectValue::Host(HostValue::Int(10)),
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
    assert!(ctx.tx.patches().is_empty());
}

#[test]
fn reflect_get_denies_non_reflect_readable_host_fields() {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .host_type(HostTypeId::new(1))
            .field(
                FieldDesc::new(FieldId::new(2), "secret")
                    .access(FieldAccess::new().reflect_readable(false)),
            ),
    );
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(
        HostPath::new(player_ref()).field(FieldId::new(2)),
        HostValue::Int(9),
    );
    let mut tx = PatchTx::new();
    let mut ctx = ReflectContext {
        registry: &registry,
        adapter: &adapter,
        tx: &mut tx,
    };

    let error = get(&mut ctx, &ReflectValue::HostRef(player_ref()), "secret").expect_err("read");

    assert_eq!(
        error.kind,
        ReflectErrorKind::FieldNotReflectReadable {
            type_name: "Player".to_owned(),
            field: "secret".to_owned()
        }
    );
}

#[test]
fn reflect_set_denies_non_reflect_writable_host_fields() {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .host_type(HostTypeId::new(1))
            .field(
                FieldDesc::new(FieldId::new(2), "level")
                    .access(FieldAccess::new().writable(true).reflect_writable(false)),
            ),
    );
    let adapter = adapter_with_level(HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut ctx = ReflectContext {
        registry: &registry,
        adapter: &adapter,
        tx: &mut tx,
    };

    let error = set(
        &mut ctx,
        &ReflectValue::HostRef(player_ref()),
        "level",
        ReflectValue::Host(HostValue::Int(10)),
    )
    .expect_err("write");

    assert_eq!(
        error.kind,
        ReflectErrorKind::FieldNotReflectWritable {
            type_name: "Player".to_owned(),
            field: "level".to_owned()
        }
    );
    assert!(ctx.tx.patches().is_empty());
}

#[test]
fn reflect_get_and_set_with_policy_require_field_permission() {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .host_type(HostTypeId::new(1))
            .field(
                FieldDesc::new(FieldId::new(2), "level").access(
                    FieldAccess::new()
                        .writable(true)
                        .reflect_writable(true)
                        .require_permission("player.level.reflect"),
                ),
            ),
    );
    let adapter = adapter_with_level(HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut ctx = ReflectContext {
        registry: &registry,
        adapter: &adapter,
        tx: &mut tx,
    };

    let error = get_with_policy(
        &mut ctx,
        &ReflectValue::HostRef(player_ref()),
        "level",
        &ReflectPolicy::all(),
    )
    .expect_err("missing field read permission");
    assert_eq!(
        error.kind,
        ReflectErrorKind::FieldPermissionDenied {
            type_name: "Player".to_owned(),
            field: "level".to_owned(),
            permission: "player.level.reflect".to_owned(),
        }
    );

    let error = set_with_policy(
        &mut ctx,
        &ReflectValue::HostRef(player_ref()),
        "level",
        ReflectValue::Host(HostValue::Int(10)),
        &ReflectPolicy::all(),
    )
    .expect_err("missing field write permission");
    assert_eq!(
        error.kind,
        ReflectErrorKind::FieldPermissionDenied {
            type_name: "Player".to_owned(),
            field: "level".to_owned(),
            permission: "player.level.reflect".to_owned(),
        }
    );
    assert!(ctx.tx.patches().is_empty());

    let policy = ReflectPolicy::all().with_field_permission("player.level.reflect");
    assert_eq!(
        get_with_policy(
            &mut ctx,
            &ReflectValue::HostRef(player_ref()),
            "level",
            &policy,
        )
        .expect("field read permission"),
        ReflectValue::Host(HostValue::Int(9))
    );
    assert_eq!(
        set_with_policy(
            &mut ctx,
            &ReflectValue::HostRef(player_ref()),
            "level",
            ReflectValue::Host(HostValue::Int(10)),
            &policy,
        )
        .expect("field write permission"),
        ReflectValue::Host(HostValue::Null)
    );
    assert_eq!(ctx.tx.patches().len(), 1);
}

#[test]
fn reflect_get_with_policy_filters_unknown_host_field_candidates() {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(101), "Player"))
            .host_type(HostTypeId::new(1))
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
    let adapter = adapter_with_level(HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut ctx = ReflectContext {
        registry: &registry,
        adapter: &adapter,
        tx: &mut tx,
    };

    let error = get_with_policy(
        &mut ctx,
        &ReflectValue::HostRef(player_ref()),
        "level_secrett",
        &ReflectPolicy::read_only(),
    )
    .expect_err("unknown host field");

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
fn reflect_set_with_policy_filters_unknown_host_field_candidates() {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(102), "Player"))
            .host_type(HostTypeId::new(1))
            .field(
                FieldDesc::new(FieldId::new(1), "level")
                    .access(FieldAccess::new().writable(true).reflect_writable(true)),
            )
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
    let adapter = adapter_with_level(HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut ctx = ReflectContext {
        registry: &registry,
        adapter: &adapter,
        tx: &mut tx,
    };

    let error = set_with_policy(
        &mut ctx,
        &ReflectValue::HostRef(player_ref()),
        "level_secrett",
        ReflectValue::Host(HostValue::Int(10)),
        &ReflectPolicy::read_only(),
    )
    .expect_err("unknown host field");

    assert_eq!(
        error.kind,
        ReflectErrorKind::UnknownField {
            type_name: "Player".to_owned(),
            field: "level_secrett".to_owned(),
            candidates: vec!["level".to_owned()],
            related: vec![ReflectCandidate::new("level", None)],
        }
    );
    assert!(ctx.tx.patches().is_empty());
}

#[test]
fn reflect_get_and_set_with_policy_require_script_field_permission() {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(200), "Player"))
            .kind(TypeKind::ScriptStruct)
            .field(
                FieldDesc::new(FieldId::new(2), "level")
                    .access(FieldAccess::new().require_permission("player.level.reflect")),
            ),
    );
    let adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut fields = BTreeMap::new();
    fields.insert("level".to_owned(), ReflectValue::Host(HostValue::Int(7)));
    let record = ReflectValue::ScriptRecord {
        type_name: "Player".to_owned(),
        fields,
    };
    let mut ctx = ReflectContext {
        registry: &registry,
        adapter: &adapter,
        tx: &mut tx,
    };

    let error = get_with_policy(&mut ctx, &record, "level", &ReflectPolicy::all())
        .expect_err("missing script field read permission");
    assert_eq!(
        error.kind,
        ReflectErrorKind::FieldPermissionDenied {
            type_name: "Player".to_owned(),
            field: "level".to_owned(),
            permission: "player.level.reflect".to_owned(),
        }
    );

    let error = set_with_policy(
        &mut ctx,
        &record,
        "level",
        ReflectValue::Host(HostValue::Int(10)),
        &ReflectPolicy::all(),
    )
    .expect_err("missing script field write permission");
    assert_eq!(
        error.kind,
        ReflectErrorKind::FieldPermissionDenied {
            type_name: "Player".to_owned(),
            field: "level".to_owned(),
            permission: "player.level.reflect".to_owned(),
        }
    );
    assert!(ctx.tx.patches().is_empty());

    let policy = ReflectPolicy::all().with_field_permission("player.level.reflect");
    assert_eq!(
        get_with_policy(&mut ctx, &record, "level", &policy).expect("script field read"),
        ReflectValue::Host(HostValue::Int(7))
    );
    assert_eq!(
        set_with_policy(
            &mut ctx,
            &record,
            "level",
            ReflectValue::Host(HostValue::Int(10)),
            &policy,
        )
        .expect("script field write"),
        ReflectValue::ScriptRecord {
            type_name: "Player".to_owned(),
            fields: BTreeMap::from([("level".to_owned(), ReflectValue::Host(HostValue::Int(10)),)]),
        }
    );
    assert!(ctx.tx.patches().is_empty());
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
    let adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let record = ReflectValue::ScriptRecord {
        type_name: "Player".to_owned(),
        fields: BTreeMap::from([("level".to_owned(), ReflectValue::Host(HostValue::Int(7)))]),
    };
    let mut ctx = ReflectContext {
        registry: &registry,
        adapter: &adapter,
        tx: &mut tx,
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
    let adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let value = ReflectValue::ScriptEnum {
        enum_name: "QuestProgress".to_owned(),
        variant: "Active".to_owned(),
        fields: BTreeMap::from([("count".to_owned(), ReflectValue::Host(HostValue::Int(7)))]),
    };
    let mut ctx = ReflectContext {
        registry: &registry,
        adapter: &adapter,
        tx: &mut tx,
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
            type_name: "QuestProgress.Active".to_owned(),
            field: "count_secrett".to_owned(),
            candidates: vec!["count".to_owned()],
            related: vec![ReflectCandidate::new("count", None)],
        }
    );
}

#[test]
fn unknown_fields_include_candidate_hints() {
    let registry = registry();
    let adapter = adapter_with_level(HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut ctx = ReflectContext {
        registry: &registry,
        adapter: &adapter,
        tx: &mut tx,
    };

    let error =
        get(&mut ctx, &ReflectValue::HostRef(player_ref()), "levle").expect_err("unknown field");

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
fn type_registry_exposes_host_type_fields() {
    let registry = registry();
    let desc = type_of(&registry, &ReflectValue::HostRef(player_ref())).expect("type desc");
    let fields = fields(&registry, &desc.key).expect("fields");

    assert_eq!(desc.key.name, "Player");
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[1].name, "level");
}

#[test]
fn reflect_get_propagates_host_generation_errors() {
    let registry = registry();
    let mut adapter = MockStateAdapter::new();
    let fresh_ref = player_ref();
    adapter.insert_value(
        HostPath::new(fresh_ref).field(FieldId::new(2)),
        HostValue::Int(9),
    );
    let stale_ref = HostRef::new(fresh_ref.type_id, fresh_ref.object_id, 2);
    let mut tx = PatchTx::new();
    let mut ctx = ReflectContext {
        registry: &registry,
        adapter: &adapter,
        tx: &mut tx,
    };

    let error = get(&mut ctx, &ReflectValue::HostRef(stale_ref), "level").expect_err("stale get");

    assert!(matches!(error.kind, ReflectErrorKind::Host(_)));
    assert_eq!(
        vela_host::tx::PatchTx::require_fresh_ref(
            stale_ref,
            &HostObjectSnapshot {
                type_id: fresh_ref.type_id,
                object_id: fresh_ref.object_id,
                generation: 3,
            }
        )
        .expect_err("stale ref")
        .kind,
        vela_host::error::HostErrorKind::StaleGeneration {
            expected: 2,
            actual: 3
        }
    );
}
