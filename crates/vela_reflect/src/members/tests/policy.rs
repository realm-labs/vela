use super::*;

#[test]
fn methods_with_policy_hide_inaccessible_methods() {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(500), "Player"))
            .host_type(HostTypeId::new(5))
            .method(MethodDesc::new(HostMethodId::new(1), "visible"))
            .method(
                MethodDesc::new(HostMethodId::new(2), "hidden")
                    .access(crate::access::MethodAccess::new().reflect_callable(false)),
            )
            .method(
                MethodDesc::new(HostMethodId::new(3), "private").access(
                    crate::access::MethodAccess::new()
                        .public(false)
                        .reflect_callable(true),
                ),
            )
            .method(
                MethodDesc::new(HostMethodId::new(4), "admin")
                    .access(crate::access::MethodAccess::new().require_permission("player.admin")),
            ),
    );
    let target = ReflectValue::HostRef(HostRef::new(HostTypeId::new(5), HostObjectId::new(1), 1));

    let ReflectValue::Array(raw_methods) = methods(&registry, &target).expect("raw methods") else {
        panic!("methods should be an array");
    };
    assert_eq!(raw_methods.len(), 4);
    assert!(has_method(&registry, &target, "private").expect("raw has private"));

    let ReflectValue::Array(policy_methods) =
        methods_with_policy(&registry, &target, &ReflectPolicy::read_only())
            .expect("policy methods")
    else {
        panic!("methods should be an array");
    };
    let ReflectValue::Array(policy_all_methods) =
        all_methods_with_policy(&registry, &ReflectPolicy::read_only())
    else {
        panic!("all methods should be an array");
    };
    assert_eq!(policy_methods.len(), 1);
    assert_eq!(policy_all_methods.len(), 1);
    let ReflectValue::ScriptRecord { fields, .. } = &policy_methods[0] else {
        panic!("method metadata should be a record");
    };
    assert_eq!(
        fields.get("name"),
        Some(&ReflectValue::Host(HostValue::String("visible".to_owned())))
    );
    let ReflectValue::ScriptRecord {
        fields: method_fields,
        ..
    } = method_with_policy(&registry, &target, "visible", &ReflectPolicy::read_only())
        .expect("visible method")
    else {
        panic!("method metadata should be a record");
    };
    assert_eq!(
        method_fields.get("name"),
        Some(&ReflectValue::Host(HostValue::String("visible".to_owned())))
    );
    let ReflectValue::ScriptRecord {
        fields: all_method_fields,
        ..
    } = &policy_all_methods[0]
    else {
        panic!("all method metadata should be a record");
    };
    assert_eq!(
        all_method_fields.get("owner"),
        Some(&ReflectValue::Host(HostValue::String("Player".to_owned())))
    );
    assert_eq!(
        all_method_fields.get("name"),
        Some(&ReflectValue::Host(HostValue::String("visible".to_owned())))
    );
    assert!(
        has_method_with_policy(&registry, &target, "visible", &ReflectPolicy::read_only())
            .expect("has visible")
    );
    assert!(
        !has_method_with_policy(&registry, &target, "private", &ReflectPolicy::read_only())
            .expect("has private")
    );
    assert!(
        !has_method_with_policy(&registry, &target, "admin", &ReflectPolicy::read_only())
            .expect("has admin")
    );
    assert!(method_with_policy(&registry, &target, "admin", &ReflectPolicy::read_only()).is_err());

    let admin_policy = ReflectPolicy::new(
        crate::permissions::ReflectPermissionSet::read_only()
            .with(crate::permissions::ReflectPermission::AccessPrivate),
    )
    .with_method_permission("player.admin");
    let ReflectValue::Array(admin_methods) =
        methods_with_policy(&registry, &target, &admin_policy).expect("admin methods")
    else {
        panic!("methods should be an array");
    };
    assert_eq!(admin_methods.len(), 3);
    assert!(
        has_method_with_policy(&registry, &target, "private", &admin_policy).expect("has private")
    );
    assert!(has_method_with_policy(&registry, &target, "admin", &admin_policy).expect("has admin"));
    let ReflectValue::ScriptRecord {
        fields: admin_fields,
        ..
    } = method_with_policy(&registry, &target, "admin", &admin_policy).expect("admin method")
    else {
        panic!("admin method metadata should be a record");
    };
    assert_eq!(
        admin_fields.get("name"),
        Some(&ReflectValue::Host(HostValue::String("admin".to_owned())))
    );
}

#[test]
fn method_policy_filters_unknown_candidates() {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(501), "Player"))
            .host_type(HostTypeId::new(5))
            .method(MethodDesc::new(HostMethodId::new(1), "grant_exp"))
            .method(
                MethodDesc::new(HostMethodId::new(2), "grant_exp_hidden")
                    .access(crate::access::MethodAccess::new().reflect_callable(false)),
            )
            .method(
                MethodDesc::new(HostMethodId::new(3), "grant_exp_private").access(
                    crate::access::MethodAccess::new()
                        .public(false)
                        .reflect_callable(true),
                ),
            )
            .method(
                MethodDesc::new(HostMethodId::new(4), "grant_exp_admin")
                    .access(crate::access::MethodAccess::new().require_permission("player.admin")),
            ),
    );
    let target = ReflectValue::HostRef(HostRef::new(HostTypeId::new(5), HostObjectId::new(1), 1));

    let error = method_with_policy(
        &registry,
        &target,
        "grant_exp_hiddden",
        &ReflectPolicy::read_only(),
    )
    .expect_err("unknown method");
    let ReflectErrorKind::UnknownMethod {
        candidates,
        related,
        ..
    } = error.kind
    else {
        panic!("expected unknown method");
    };

    assert_eq!(candidates, vec!["grant_exp".to_owned()]);
    assert_eq!(
        related,
        vec![crate::candidates::ReflectCandidate::new("grant_exp", None)]
    );
}

#[test]
fn fields_with_policy_hide_non_reflect_readable_fields() {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(600), "Player"))
            .host_type(HostTypeId::new(6))
            .field(FieldDesc::new(FieldId::new(1), "level"))
            .field(
                FieldDesc::new(FieldId::new(2), "secret")
                    .access(crate::access::FieldAccess::new().reflect_readable(false)),
            ),
    );
    let target = ReflectValue::HostRef(HostRef::new(HostTypeId::new(6), HostObjectId::new(1), 1));

    assert!(has_field(&registry, &target, "secret").expect("raw has field"));
    let ReflectValue::Array(fields) =
        fields_with_policy(&registry, &target, &ReflectPolicy::read_only())
            .expect("field metadata")
    else {
        panic!("fields should be an array");
    };
    assert_eq!(fields.len(), 1);
    let ReflectValue::ScriptRecord {
        fields: field_fields,
        ..
    } = &fields[0]
    else {
        panic!("field metadata should be a record");
    };
    assert_eq!(
        field_fields.get("owner"),
        Some(&ReflectValue::Host(HostValue::String("Player".to_owned())))
    );
    assert_eq!(
        field_fields.get("name"),
        Some(&ReflectValue::Host(HostValue::String("level".to_owned())))
    );
    let ReflectValue::Array(all_fields) =
        all_fields_with_policy(&registry, &ReflectPolicy::read_only())
    else {
        panic!("all fields should be an array");
    };
    assert_eq!(all_fields.len(), 1);
    let ReflectValue::ScriptRecord {
        fields: field_list_item,
        ..
    } = &all_fields[0]
    else {
        panic!("field list item should be a record");
    };
    assert_eq!(
        field_list_item.get("owner"),
        Some(&ReflectValue::Host(HostValue::String("Player".to_owned())))
    );
    assert_eq!(
        field_list_item.get("name"),
        Some(&ReflectValue::Host(HostValue::String("level".to_owned())))
    );
    assert!(
        has_field_with_policy(&registry, &target, "level", &ReflectPolicy::read_only())
            .expect("has level")
    );
    assert!(
        !has_field_with_policy(&registry, &target, "secret", &ReflectPolicy::read_only())
            .expect("has secret")
    );

    let error = field_with_policy(&registry, &target, "secret", &ReflectPolicy::read_only())
        .expect_err("hidden field metadata");
    assert_eq!(
        error.kind,
        ReflectErrorKind::FieldNotReflectReadable {
            type_name: "Player".to_owned(),
            field: "secret".to_owned(),
            source_span: None,
        }
    );

    let ReflectValue::ScriptRecord { fields, .. } =
        field(&registry, &target, "secret").expect("raw field metadata")
    else {
        panic!("field metadata should be a record");
    };
    assert_eq!(
        fields.get("name"),
        Some(&ReflectValue::Host(HostValue::String("secret".to_owned())))
    );
}

#[test]
fn field_policy_filters_unknown_candidates() {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(602), "Player"))
            .host_type(HostTypeId::new(6))
            .field(FieldDesc::new(FieldId::new(1), "level"))
            .field(
                FieldDesc::new(FieldId::new(2), "level_secret")
                    .access(crate::access::FieldAccess::new().reflect_readable(false)),
            )
            .field(FieldDesc::new(FieldId::new(3), "level_admin").access(
                crate::access::FieldAccess::new().require_permission("player.level.admin"),
            )),
    );
    let target = ReflectValue::HostRef(HostRef::new(HostTypeId::new(6), HostObjectId::new(1), 1));

    let error = field_with_policy(
        &registry,
        &target,
        "level_secrett",
        &ReflectPolicy::read_only(),
    )
    .expect_err("unknown field");
    let ReflectErrorKind::UnknownField {
        candidates,
        related,
        ..
    } = error.kind
    else {
        panic!("expected unknown field");
    };

    assert_eq!(candidates, vec!["level".to_owned()]);
    assert_eq!(
        related,
        vec![crate::candidates::ReflectCandidate::new("level", None)]
    );
}

#[test]
fn fields_with_policy_require_field_permissions() {
    let mut registry = TypeRegistry::new();
    let title_span = Span::new(SourceId::new(9), 30, 45);
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(601), "Player"))
            .host_type(HostTypeId::new(6))
            .field(FieldDesc::new(FieldId::new(1), "level"))
            .field(
                FieldDesc::new(FieldId::new(2), "title")
                    .source_span(title_span)
                    .access(
                        crate::access::FieldAccess::new()
                            .require_permission("player.title.inspect"),
                    ),
            ),
    );
    let target = ReflectValue::HostRef(HostRef::new(HostTypeId::new(6), HostObjectId::new(1), 1));

    let ReflectValue::Array(fields) =
        fields_with_policy(&registry, &target, &ReflectPolicy::read_only())
            .expect("field metadata")
    else {
        panic!("fields should be an array");
    };
    assert_eq!(fields.len(), 1);
    let ReflectValue::ScriptRecord {
        fields: field_fields,
        ..
    } = &fields[0]
    else {
        panic!("field metadata should be a record");
    };
    assert_eq!(
        field_fields.get("name"),
        Some(&ReflectValue::Host(HostValue::String("level".to_owned())))
    );
    assert!(
        !has_field_with_policy(&registry, &target, "title", &ReflectPolicy::read_only())
            .expect("has title")
    );

    let error = field_with_policy(&registry, &target, "title", &ReflectPolicy::read_only())
        .expect_err("field permission");
    assert_eq!(
        error.kind,
        ReflectErrorKind::FieldPermissionDenied {
            type_name: "Player".to_owned(),
            field: "title".to_owned(),
            permission: "player.title.inspect".to_owned(),
            source_span: Some(title_span),
        }
    );
    assert_eq!(
        error.kind.related_labels(),
        vec![(
            title_span,
            "field `Player.title` is declared here".to_owned()
        )]
    );

    let policy = ReflectPolicy::read_only().with_field_permission("player.title.inspect");
    let ReflectValue::ScriptRecord { fields, .. } =
        field_with_policy(&registry, &target, "title", &policy).expect("allowed field")
    else {
        panic!("field metadata should be a record");
    };
    assert_eq!(
        fields.get("name"),
        Some(&ReflectValue::Host(HostValue::String("title".to_owned())))
    );
    let Some(ReflectValue::ScriptRecord { fields: access, .. }) = fields.get("access") else {
        panic!("field access should be a record");
    };
    assert_eq!(
        access.get("required_permissions"),
        Some(&ReflectValue::Array(vec![ReflectValue::Host(
            HostValue::String("player.title.inspect".to_owned())
        )]))
    );
}

#[test]
fn variant_field_policy_filters_unknown_candidates() {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(702), "QuestProgress"))
            .kind(TypeKind::ScriptEnum)
            .variant(
                VariantDesc::new(VariantId::new(1), "Active")
                    .field(FieldDesc::new(FieldId::new(1), "count"))
                    .field(
                        FieldDesc::new(FieldId::new(2), "count_secret")
                            .access(crate::access::FieldAccess::new().reflect_readable(false)),
                    )
                    .field(FieldDesc::new(FieldId::new(3), "count_admin").access(
                        crate::access::FieldAccess::new().require_permission("quest.count.admin"),
                    )),
            ),
    );
    let target = ReflectValue::ScriptEnum {
        enum_name: "QuestProgress".to_owned(),
        variant: "Active".to_owned(),
        fields: BTreeMap::new(),
    };

    let error = field_with_policy(
        &registry,
        &target,
        "count_secrett",
        &ReflectPolicy::read_only(),
    )
    .expect_err("unknown variant field");
    let ReflectErrorKind::UnknownField {
        candidates,
        related,
        ..
    } = error.kind
    else {
        panic!("expected unknown field");
    };

    assert_eq!(candidates, vec!["count".to_owned()]);
    assert_eq!(
        related,
        vec![crate::candidates::ReflectCandidate::new("count", None)]
    );
}

#[test]
fn variants_with_policy_hide_non_reflect_readable_fields() {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(700), "QuestProgress"))
            .kind(TypeKind::ScriptEnum)
            .variant(
                VariantDesc::new(VariantId::new(1), "Active")
                    .field(FieldDesc::new(FieldId::new(1), "count"))
                    .field(
                        FieldDesc::new(FieldId::new(2), "secret")
                            .access(crate::access::FieldAccess::new().reflect_readable(false)),
                    ),
            ),
    );
    let target = ReflectValue::ScriptEnum {
        enum_name: "QuestProgress".to_owned(),
        variant: "Active".to_owned(),
        fields: BTreeMap::new(),
    };

    let ReflectValue::Array(raw_variants) = variants(&registry, &target).expect("raw variants")
    else {
        panic!("variants should be an array");
    };
    let ReflectValue::ScriptRecord { fields, .. } = &raw_variants[0] else {
        panic!("variant metadata should be a record");
    };
    assert!(matches!(
        fields.get("fields"),
        Some(ReflectValue::Array(raw_fields)) if raw_fields.len() == 2
    ));
    let Some(ReflectValue::Array(raw_fields)) = fields.get("fields") else {
        panic!("raw variant fields should be an array");
    };
    let ReflectValue::ScriptRecord {
        fields: raw_field_fields,
        ..
    } = &raw_fields[0]
    else {
        panic!("raw variant field metadata should be a record");
    };
    assert_eq!(
        raw_field_fields.get("owner"),
        Some(&ReflectValue::Host(HostValue::String(
            "QuestProgress::Active".to_owned()
        )))
    );

    let ReflectValue::Array(policy_variants) =
        variants_with_policy(&registry, &target, &ReflectPolicy::read_only())
            .expect("policy variants")
    else {
        panic!("variants should be an array");
    };
    let ReflectValue::ScriptRecord {
        fields: policy_variant,
        ..
    } = variant_info_with_policy(&registry, &target, "Active", &ReflectPolicy::read_only())
        .expect("policy variant info")
    else {
        panic!("variant info should be a record");
    };
    let ReflectValue::Array(policy_all_variants) =
        all_variants_with_policy(&registry, &ReflectPolicy::read_only())
    else {
        panic!("all variants should be an array");
    };
    let ReflectValue::Array(policy_all_fields) =
        all_fields_with_policy(&registry, &ReflectPolicy::read_only())
    else {
        panic!("all fields should be an array");
    };
    let ReflectValue::ScriptRecord { fields, .. } = &policy_variants[0] else {
        panic!("variant metadata should be a record");
    };
    let Some(ReflectValue::Array(policy_fields)) = fields.get("fields") else {
        panic!("variant fields should be an array");
    };
    assert_eq!(policy_fields.len(), 1);
    let ReflectValue::ScriptRecord { fields, .. } = &policy_fields[0] else {
        panic!("field metadata should be a record");
    };
    assert_eq!(
        fields.get("name"),
        Some(&ReflectValue::Host(HostValue::String("count".to_owned())))
    );
    assert_eq!(
        fields.get("owner"),
        Some(&ReflectValue::Host(HostValue::String(
            "QuestProgress::Active".to_owned()
        )))
    );
    let ReflectValue::ScriptRecord {
        fields: all_variant_fields,
        ..
    } = &policy_all_variants[0]
    else {
        panic!("variant metadata should be a record");
    };
    assert_eq!(
        all_variant_fields.get("owner"),
        Some(&ReflectValue::Host(HostValue::String(
            "QuestProgress".to_owned()
        )))
    );
    let Some(ReflectValue::Array(all_policy_fields)) = all_variant_fields.get("fields") else {
        panic!("variant fields should be an array");
    };
    assert_eq!(all_policy_fields.len(), 1);
    let ReflectValue::ScriptRecord {
        fields: all_policy_field_fields,
        ..
    } = &all_policy_fields[0]
    else {
        panic!("all variant field metadata should be a record");
    };
    assert_eq!(
        all_policy_field_fields.get("owner"),
        Some(&ReflectValue::Host(HostValue::String(
            "QuestProgress::Active".to_owned()
        )))
    );
    assert_eq!(policy_all_fields.len(), 1);
    let ReflectValue::ScriptRecord {
        fields: all_field_fields,
        ..
    } = &policy_all_fields[0]
    else {
        panic!("all field metadata should be a record");
    };
    assert_eq!(
        all_field_fields.get("owner"),
        Some(&ReflectValue::Host(HostValue::String(
            "QuestProgress::Active".to_owned()
        )))
    );
    assert_eq!(
        all_field_fields.get("name"),
        Some(&ReflectValue::Host(HostValue::String("count".to_owned())))
    );
    let Some(ReflectValue::Array(policy_variant_fields)) = policy_variant.get("fields") else {
        panic!("variant info fields should be an array");
    };
    assert_eq!(policy_variant_fields.len(), 1);
    let ReflectValue::ScriptRecord {
        fields: policy_variant_field_fields,
        ..
    } = &policy_variant_fields[0]
    else {
        panic!("variant info field metadata should be a record");
    };
    assert_eq!(
        policy_variant_field_fields.get("owner"),
        Some(&ReflectValue::Host(HostValue::String(
            "QuestProgress::Active".to_owned()
        )))
    );
    assert!(
        has_field_with_policy(&registry, &target, "count", &ReflectPolicy::read_only())
            .expect("has visible active variant field")
    );
    assert!(
        !has_field_with_policy(&registry, &target, "secret", &ReflectPolicy::read_only())
            .expect("has hidden active variant field")
    );

    let error = field_with_policy(&registry, &target, "secret", &ReflectPolicy::read_only())
        .expect_err("hidden active variant field");
    assert_eq!(
        error.kind,
        ReflectErrorKind::FieldNotReflectReadable {
            type_name: "QuestProgress::Active".to_owned(),
            field: "secret".to_owned(),
            source_span: None,
        }
    );
}
