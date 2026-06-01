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

    let ReflectValue::Host(HostValue::Array(raw_methods)) =
        methods(&registry, &target).expect("raw methods")
    else {
        panic!("methods should be an array");
    };
    assert_eq!(raw_methods.len(), 4);
    assert!(has_method(&registry, &target, "private").expect("raw has private"));

    let ReflectValue::Host(HostValue::Array(policy_methods)) =
        methods_with_policy(&registry, &target, &ReflectPolicy::read_only())
            .expect("policy methods")
    else {
        panic!("methods should be an array");
    };
    let ReflectValue::Host(HostValue::Array(policy_all_methods)) =
        all_methods_with_policy(&registry, &ReflectPolicy::read_only())
    else {
        panic!("all methods should be an array");
    };
    assert_eq!(policy_methods.len(), 1);
    assert_eq!(policy_all_methods.len(), 1);
    let HostValue::Record { fields, .. } = &policy_methods[0] else {
        panic!("method metadata should be a record");
    };
    assert_eq!(
        fields.get("name"),
        Some(&HostValue::String("visible".to_owned()))
    );
    let ReflectValue::Host(HostValue::Record {
        fields: method_fields,
        ..
    }) = method_with_policy(&registry, &target, "visible", &ReflectPolicy::read_only())
        .expect("visible method")
    else {
        panic!("method metadata should be a record");
    };
    assert_eq!(
        method_fields.get("name"),
        Some(&HostValue::String("visible".to_owned()))
    );
    let HostValue::Record {
        fields: all_method_fields,
        ..
    } = &policy_all_methods[0]
    else {
        panic!("all method metadata should be a record");
    };
    assert_eq!(
        all_method_fields.get("owner"),
        Some(&HostValue::String("Player".to_owned()))
    );
    assert_eq!(
        all_method_fields.get("name"),
        Some(&HostValue::String("visible".to_owned()))
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
    let ReflectValue::Host(HostValue::Array(admin_methods)) =
        methods_with_policy(&registry, &target, &admin_policy).expect("admin methods")
    else {
        panic!("methods should be an array");
    };
    assert_eq!(admin_methods.len(), 3);
    assert!(
        has_method_with_policy(&registry, &target, "private", &admin_policy).expect("has private")
    );
    assert!(has_method_with_policy(&registry, &target, "admin", &admin_policy).expect("has admin"));
    let ReflectValue::Host(HostValue::Record {
        fields: admin_fields,
        ..
    }) = method_with_policy(&registry, &target, "admin", &admin_policy).expect("admin method")
    else {
        panic!("admin method metadata should be a record");
    };
    assert_eq!(
        admin_fields.get("name"),
        Some(&HostValue::String("admin".to_owned()))
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
    let ReflectValue::Host(HostValue::Array(fields)) =
        fields_with_policy(&registry, &target, &ReflectPolicy::read_only())
            .expect("field metadata")
    else {
        panic!("fields should be an array");
    };
    assert_eq!(fields.len(), 1);
    let HostValue::Record {
        fields: field_fields,
        ..
    } = &fields[0]
    else {
        panic!("field metadata should be a record");
    };
    assert_eq!(
        field_fields.get("owner"),
        Some(&HostValue::String("Player".to_owned()))
    );
    assert_eq!(
        field_fields.get("name"),
        Some(&HostValue::String("level".to_owned()))
    );
    let ReflectValue::Host(HostValue::Array(all_fields)) =
        all_fields_with_policy(&registry, &ReflectPolicy::read_only())
    else {
        panic!("all fields should be an array");
    };
    assert_eq!(all_fields.len(), 1);
    let HostValue::Record {
        fields: field_list_item,
        ..
    } = &all_fields[0]
    else {
        panic!("field list item should be a record");
    };
    assert_eq!(
        field_list_item.get("owner"),
        Some(&HostValue::String("Player".to_owned()))
    );
    assert_eq!(
        field_list_item.get("name"),
        Some(&HostValue::String("level".to_owned()))
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
            field: "secret".to_owned()
        }
    );

    let ReflectValue::Host(HostValue::Record { fields, .. }) =
        field(&registry, &target, "secret").expect("raw field metadata")
    else {
        panic!("field metadata should be a record");
    };
    assert_eq!(
        fields.get("name"),
        Some(&HostValue::String("secret".to_owned()))
    );
}

#[test]
fn fields_with_policy_require_field_permissions() {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(601), "Player"))
            .host_type(HostTypeId::new(6))
            .field(FieldDesc::new(FieldId::new(1), "level"))
            .field(FieldDesc::new(FieldId::new(2), "title").access(
                crate::access::FieldAccess::new().require_permission("player.title.inspect"),
            )),
    );
    let target = ReflectValue::HostRef(HostRef::new(HostTypeId::new(6), HostObjectId::new(1), 1));

    let ReflectValue::Host(HostValue::Array(fields)) =
        fields_with_policy(&registry, &target, &ReflectPolicy::read_only())
            .expect("field metadata")
    else {
        panic!("fields should be an array");
    };
    assert_eq!(fields.len(), 1);
    let HostValue::Record {
        fields: field_fields,
        ..
    } = &fields[0]
    else {
        panic!("field metadata should be a record");
    };
    assert_eq!(
        field_fields.get("name"),
        Some(&HostValue::String("level".to_owned()))
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
        }
    );

    let policy = ReflectPolicy::read_only().with_field_permission("player.title.inspect");
    let ReflectValue::Host(HostValue::Record { fields, .. }) =
        field_with_policy(&registry, &target, "title", &policy).expect("allowed field")
    else {
        panic!("field metadata should be a record");
    };
    assert_eq!(
        fields.get("name"),
        Some(&HostValue::String("title".to_owned()))
    );
    let Some(HostValue::Record { fields: access, .. }) = fields.get("access") else {
        panic!("field access should be a record");
    };
    assert_eq!(
        access.get("required_permissions"),
        Some(&HostValue::Array(vec![HostValue::String(
            "player.title.inspect".to_owned()
        )]))
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

    let ReflectValue::Host(HostValue::Array(raw_variants)) =
        variants(&registry, &target).expect("raw variants")
    else {
        panic!("variants should be an array");
    };
    let HostValue::Record { fields, .. } = &raw_variants[0] else {
        panic!("variant metadata should be a record");
    };
    assert!(matches!(
        fields.get("fields"),
        Some(HostValue::Array(raw_fields)) if raw_fields.len() == 2
    ));

    let ReflectValue::Host(HostValue::Array(policy_variants)) =
        variants_with_policy(&registry, &target, &ReflectPolicy::read_only())
            .expect("policy variants")
    else {
        panic!("variants should be an array");
    };
    let ReflectValue::Host(HostValue::Record {
        fields: policy_variant,
        ..
    }) = variant_info_with_policy(&registry, &target, "Active", &ReflectPolicy::read_only())
        .expect("policy variant info")
    else {
        panic!("variant info should be a record");
    };
    let ReflectValue::Host(HostValue::Array(policy_all_variants)) =
        all_variants_with_policy(&registry, &ReflectPolicy::read_only())
    else {
        panic!("all variants should be an array");
    };
    let HostValue::Record { fields, .. } = &policy_variants[0] else {
        panic!("variant metadata should be a record");
    };
    let Some(HostValue::Array(policy_fields)) = fields.get("fields") else {
        panic!("variant fields should be an array");
    };
    assert_eq!(policy_fields.len(), 1);
    let HostValue::Record { fields, .. } = &policy_fields[0] else {
        panic!("field metadata should be a record");
    };
    assert_eq!(
        fields.get("name"),
        Some(&HostValue::String("count".to_owned()))
    );
    let HostValue::Record {
        fields: all_variant_fields,
        ..
    } = &policy_all_variants[0]
    else {
        panic!("variant metadata should be a record");
    };
    assert_eq!(
        all_variant_fields.get("owner"),
        Some(&HostValue::String("QuestProgress".to_owned()))
    );
    let Some(HostValue::Array(all_policy_fields)) = all_variant_fields.get("fields") else {
        panic!("variant fields should be an array");
    };
    assert_eq!(all_policy_fields.len(), 1);
    let Some(HostValue::Array(policy_variant_fields)) = policy_variant.get("fields") else {
        panic!("variant info fields should be an array");
    };
    assert_eq!(policy_variant_fields.len(), 1);
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
            type_name: "QuestProgress.Active".to_owned(),
            field: "secret".to_owned(),
        }
    );
}
