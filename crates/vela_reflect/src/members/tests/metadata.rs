use super::*;

#[test]
fn name_kind_and_field_queries_return_copied_metadata() {
    let registry = registry();
    let target = ReflectValue::HostRef(player_ref());

    assert_eq!(
        name(&registry, &target).expect("name"),
        ReflectValue::Host(HostValue::String("Player".to_owned()))
    );
    assert_eq!(
        kind(&registry, &target).expect("kind"),
        ReflectValue::Host(HostValue::String("host".to_owned()))
    );
    assert_eq!(
        docs(&registry, &target).expect("docs"),
        ReflectValue::Host(HostValue::String("A player host object.".to_owned()))
    );
    assert_eq!(
        attrs(&registry, &target).expect("attrs"),
        ReflectValue::Record(BTreeMap::from([(
            "domain".to_owned(),
            ReflectValue::Host(HostValue::String("gameplay".to_owned()))
        )]))
    );
    assert!(has_field(&registry, &target, "level").expect("has field"));

    let field_metadata = field(&registry, &target, "level").expect("field");
    let player_type = crate::types::type_by_name(&registry, "Player").expect("type info");
    let field_from_type = field(&registry, &player_type, "level").expect("type field");
    assert!(has_field(&registry, &player_type, "level").expect("type has field"));
    let ReflectValue::ScriptRecord { fields, .. } = &field_metadata else {
        panic!("field metadata should be a record");
    };
    assert_eq!(field_metadata, field_from_type);
    assert_eq!(
        fields.get("writable"),
        Some(&ReflectValue::Host(HostValue::Bool(true)))
    );
    assert_eq!(
        fields.get("type"),
        Some(&ReflectValue::Host(HostValue::String("i64".to_owned())))
    );
    assert_eq!(
        fields.get("access"),
        Some(&ReflectValue::ScriptRecord {
            type_name: "ReflectFieldAccess".to_owned(),
            fields: BTreeMap::from([
                (
                    "readable".to_owned(),
                    ReflectValue::Host(HostValue::Bool(true))
                ),
                (
                    "writable".to_owned(),
                    ReflectValue::Host(HostValue::Bool(true))
                ),
                (
                    "reflect_readable".to_owned(),
                    ReflectValue::Host(HostValue::Bool(true))
                ),
                (
                    "reflect_writable".to_owned(),
                    ReflectValue::Host(HostValue::Bool(true))
                ),
                (
                    "required_permissions".to_owned(),
                    ReflectValue::Array(vec![])
                ),
            ]),
        })
    );
    assert_eq!(
        fields.get("docs"),
        Some(&ReflectValue::Host(HostValue::String(
            "Current level.".to_owned()
        )))
    );
    assert_eq!(
        fields.get("attrs"),
        Some(&ReflectValue::Record(BTreeMap::from([(
            "unit".to_owned(),
            ReflectValue::Host(HostValue::String("level".to_owned()))
        )])))
    );
    assert_eq!(
        fields.get("source_span"),
        Some(&span_value(Some(Span::new(SourceId::new(8), 50, 55))))
    );
    assert_eq!(
        source_span(&registry, &field_metadata).expect("field source span"),
        span_value(Some(Span::new(SourceId::new(8), 50, 55)))
    );
    assert_eq!(
        docs(&registry, &field_metadata).expect("field docs"),
        ReflectValue::Host(HostValue::String("Current level.".to_owned()))
    );
    assert_eq!(
        name(&registry, &field_metadata).expect("field metadata name"),
        ReflectValue::Host(HostValue::String("level".to_owned()))
    );
    assert_eq!(
        kind(&registry, &field_metadata).expect("field metadata kind"),
        ReflectValue::Host(HostValue::String("field".to_owned()))
    );
    assert_eq!(
        fields.get("origin"),
        Some(&ReflectValue::Host(HostValue::String("host".to_owned())))
    );
    assert_eq!(
        origin(&registry, &field_metadata).expect("field origin metadata"),
        ReflectValue::Host(HostValue::String("host".to_owned()))
    );
    assert_eq!(
        attrs(&registry, &field_metadata).expect("field attrs"),
        ReflectValue::Record(BTreeMap::from([(
            "unit".to_owned(),
            ReflectValue::Host(HostValue::String("level".to_owned()))
        )]))
    );
    assert_eq!(
        attr(&registry, &field_metadata, "unit").expect("field attr"),
        ReflectValue::Host(HostValue::String("level".to_owned()))
    );
    assert!(has_attr(&registry, &field_metadata, "unit").expect("field has attr"));
    assert_eq!(
        attr(&registry, &field_metadata, "missing").expect("missing field attr"),
        ReflectValue::Host(HostValue::Null)
    );
    assert!(!has_attr(&registry, &field_metadata, "missing").expect("missing field attr"));
    let ReflectValue::Array(all_fields) = all_fields(&registry) else {
        panic!("field list should be an array");
    };
    assert_eq!(all_fields.len(), 3);
    let ReflectValue::ScriptRecord {
        fields: field_list_item,
        ..
    } = &all_fields[1]
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
    let ReflectValue::ScriptRecord {
        fields: variant_field_list_item,
        ..
    } = &all_fields[2]
    else {
        panic!("variant field list item should be a record");
    };
    assert_eq!(
        variant_field_list_item.get("owner"),
        Some(&ReflectValue::Host(HostValue::String(
            "QuestProgress::Active".to_owned()
        )))
    );
    assert_eq!(
        variant_field_list_item.get("name"),
        Some(&ReflectValue::Host(HostValue::String("count".to_owned())))
    );

    let error = field(&registry, &target, "levle").expect_err("unknown field");
    assert_eq!(
        error.kind,
        ReflectErrorKind::UnknownField {
            type_name: "Player".to_owned(),
            field: "levle".to_owned(),
            candidates: vec!["level".to_owned(), "id".to_owned()],
            related: vec![
                crate::candidates::ReflectCandidate::new(
                    "level",
                    Some(Span::new(SourceId::new(8), 50, 55))
                ),
                crate::candidates::ReflectCandidate::new("id", None),
            ],
        }
    );
}

#[test]
fn method_trait_and_variant_queries_return_copied_metadata() {
    let registry = registry();

    assert!(
        has_method(&registry, &ReflectValue::HostRef(player_ref()), "grant_exp")
            .expect("has method")
    );
    let ReflectValue::Array(method_records) =
        methods(&registry, &ReflectValue::HostRef(player_ref())).expect("methods")
    else {
        panic!("methods should be an array");
    };
    let player_type = crate::types::type_by_name(&registry, "Player").expect("type info");
    let ReflectValue::Array(type_methods) = methods(&registry, &player_type).expect("type methods")
    else {
        panic!("type methods should be an array");
    };
    assert_eq!(method_records.len(), 1);
    assert_eq!(type_methods, method_records);
    assert!(has_method(&registry, &player_type, "grant_exp").expect("type has method"));
    let ReflectValue::ScriptRecord { fields, .. } = &method_records[0] else {
        panic!("method metadata should be a record");
    };
    assert_eq!(
        fields.get("return"),
        Some(&ReflectValue::Host(HostValue::String("bool".to_owned())))
    );
    assert_eq!(
        fields.get("returns"),
        Some(&ReflectValue::Host(HostValue::String("bool".to_owned())))
    );
    let Some(ReflectValue::Array(raw_params)) = fields.get("params") else {
        panic!("method params should be an array");
    };
    assert_eq!(raw_params.len(), 1);
    let ReflectValue::ScriptRecord {
        fields: param_fields,
        ..
    } = &raw_params[0]
    else {
        panic!("method param should be a record");
    };
    assert_eq!(
        param_fields.get("name"),
        Some(&ReflectValue::Host(HostValue::String("amount".to_owned())))
    );
    assert_eq!(
        param_fields.get("type"),
        Some(&ReflectValue::Host(HostValue::String("i64".to_owned())))
    );
    let Some(ReflectValue::ScriptRecord {
        fields: effect_fields,
        ..
    }) = fields.get("effects")
    else {
        panic!("method effects should be a record");
    };
    assert_eq!(
        effect_fields.get("writes_host"),
        Some(&ReflectValue::Host(HostValue::Bool(true)))
    );
    let Some(ReflectValue::ScriptRecord {
        fields: access_fields,
        ..
    }) = fields.get("access")
    else {
        panic!("method access should be a record");
    };
    assert_eq!(
        access_fields.get("reflect_callable"),
        Some(&ReflectValue::Host(HostValue::Bool(true)))
    );
    assert_eq!(
        access_fields.get("required_permissions"),
        Some(&ReflectValue::Array(vec![ReflectValue::Host(
            HostValue::String("player.grant_exp".to_owned())
        )]))
    );
    assert_eq!(
        fields.get("source_span"),
        Some(&span_value(Some(Span::new(SourceId::new(8), 60, 80))))
    );
    let ReflectValue::Array(all_methods) = all_methods(&registry) else {
        panic!("method list should be an array");
    };
    assert_eq!(all_methods.len(), 1);
    let ReflectValue::ScriptRecord {
        fields: method_list_item,
        ..
    } = &all_methods[0]
    else {
        panic!("method list item should be a record");
    };
    assert_eq!(
        method_list_item.get("owner"),
        Some(&ReflectValue::Host(HostValue::String("Player".to_owned())))
    );
    assert_eq!(
        method_list_item.get("name"),
        Some(&ReflectValue::Host(HostValue::String(
            "grant_exp".to_owned()
        )))
    );
    let single_method_value = method(&registry, &ReflectValue::HostRef(player_ref()), "grant_exp")
        .expect("method metadata");
    assert_eq!(
        method(&registry, &player_type, "grant_exp").expect("type method"),
        single_method_value
    );
    let ReflectValue::ScriptRecord {
        fields: single_method,
        ..
    } = &single_method_value
    else {
        panic!("single method metadata should be a record");
    };
    assert_eq!(
        single_method.get("name"),
        Some(&ReflectValue::Host(HostValue::String(
            "grant_exp".to_owned()
        )))
    );
    assert_eq!(
        single_method.get("origin"),
        Some(&ReflectValue::Host(HostValue::String("host".to_owned())))
    );
    assert_eq!(
        origin(&registry, &single_method_value).expect("method origin metadata"),
        ReflectValue::Host(HostValue::String("host".to_owned()))
    );
    assert_eq!(
        owner(&registry, &single_method_value).expect("method owner metadata"),
        ReflectValue::Host(HostValue::String("Player".to_owned()))
    );
    assert_eq!(
        single_method.get("attrs"),
        Some(&ReflectValue::Record(BTreeMap::from([(
            "effect".to_owned(),
            ReflectValue::Host(HostValue::String("write".to_owned()))
        )])))
    );
    let ReflectValue::ScriptRecord {
        fields: helper_effects,
        ..
    } = effects(&registry, &single_method_value).expect("method effects metadata")
    else {
        panic!("method effects metadata should be a record");
    };
    assert_eq!(
        helper_effects.get("reads_host"),
        Some(&ReflectValue::Host(HostValue::Bool(true)))
    );
    assert_eq!(
        helper_effects.get("writes_host"),
        Some(&ReflectValue::Host(HostValue::Bool(true)))
    );
    let nested_effects = single_method
        .get("effects")
        .expect("method effects record")
        .clone();
    assert_eq!(
        effects(&registry, &nested_effects).expect("nested effects metadata"),
        ReflectValue::ScriptRecord {
            type_name: "ReflectEffectSet".to_owned(),
            fields: helper_effects,
        }
    );
    let ReflectValue::Array(helper_params) =
        params(&registry, &single_method_value).expect("method params metadata")
    else {
        panic!("method params metadata should be an array");
    };
    assert_eq!(helper_params.len(), 1);
    let ReflectValue::ScriptRecord {
        fields: param_fields,
        ..
    } = &helper_params[0]
    else {
        panic!("method param should be a record");
    };
    assert_eq!(
        param_fields.get("name"),
        Some(&ReflectValue::Host(HostValue::String("amount".to_owned())))
    );
    assert_eq!(
        params(
            &registry,
            single_method.get("params").expect("method params record")
        )
        .expect("nested params metadata"),
        ReflectValue::Array(helper_params)
    );
    assert_eq!(
        returns(&registry, &single_method_value).expect("method returns metadata"),
        ReflectValue::Host(HostValue::String("bool".to_owned()))
    );
    let ReflectValue::ScriptRecord {
        fields: helper_access,
        ..
    } = access(&registry, &single_method_value).expect("method access metadata")
    else {
        panic!("method access metadata should be a record");
    };
    assert_eq!(
        helper_access.get("reflect_callable"),
        Some(&ReflectValue::Host(HostValue::Bool(true)))
    );
    assert_eq!(
        access(
            &registry,
            single_method.get("access").expect("method access record")
        )
        .expect("nested access metadata"),
        ReflectValue::ScriptRecord {
            type_name: "ReflectMethodAccess".to_owned(),
            fields: helper_access,
        }
    );
    let unknown = method(&registry, &ReflectValue::HostRef(player_ref()), "grant_xp")
        .expect_err("unknown method");
    assert_eq!(
        unknown.kind,
        ReflectErrorKind::UnknownMethod {
            type_name: "Player".to_owned(),
            method: "grant_xp".to_owned(),
            candidates: vec!["grant_exp".to_owned()],
            related: vec![crate::candidates::ReflectCandidate::new(
                "grant_exp",
                Some(Span::new(SourceId::new(8), 60, 80))
            )],
        }
    );

    let ReflectValue::Array(trait_records) =
        traits(&registry, &ReflectValue::HostRef(player_ref())).expect("traits")
    else {
        panic!("traits should be an array");
    };
    assert_eq!(
        traits(&registry, &player_type).expect("type traits"),
        ReflectValue::Array(trait_records.clone())
    );
    assert_eq!(trait_records.len(), 1);
    assert!(has_trait(&registry, "Damageable"));
    assert!(!has_trait(&registry, "Trackable"));

    let target = ReflectValue::ScriptEnum {
        enum_name: "QuestProgress".to_owned(),
        variant: "Active".to_owned(),
        fields: BTreeMap::new(),
    };
    assert_eq!(
        variant(&target).expect("variant"),
        ReflectValue::Host(HostValue::String("Active".to_owned()))
    );
    assert!(variant_is(&registry, &target, "Active").expect("variant is"));
    let ReflectValue::Array(variant_records) = variants(&registry, &target).expect("variants")
    else {
        panic!("variants should be an array");
    };
    let quest_type = crate::types::type_by_name(&registry, "QuestProgress").expect("type info");
    assert_eq!(
        variants(&registry, &quest_type).expect("type variants"),
        ReflectValue::Array(variant_records.clone())
    );
    assert_eq!(variant_records.len(), 2);
    assert!(has_variant(&registry, &target, "Active").expect("has active"));
    assert!(has_variant(&registry, &quest_type, "Active").expect("type has active"));
    assert!(!has_variant(&registry, &target, "Paused").expect("has paused"));
    assert!(has_field(&registry, &target, "count").expect("has active field"));
    assert!(!has_field(&registry, &target, "missing").expect("missing active field"));
    let ReflectValue::Array(active_fields) =
        fields_with_policy(&registry, &target, &ReflectPolicy::read_only())
            .expect("active variant fields")
    else {
        panic!("active variant fields should be an array");
    };
    assert_eq!(active_fields.len(), 1);
    let active_field = field(&registry, &target, "count").expect("active variant field");
    assert_eq!(active_fields[0], active_field);
    let error = field(&registry, &target, "cout").expect_err("unknown active variant field");
    assert_eq!(
        error.kind,
        ReflectErrorKind::UnknownField {
            type_name: "QuestProgress::Active".to_owned(),
            field: "cout".to_owned(),
            candidates: vec!["count".to_owned()],
            related: vec![crate::candidates::ReflectCandidate::new("count", None)],
        }
    );
    let ReflectValue::ScriptRecord {
        fields: variant_fields,
        ..
    } = &variant_records[0]
    else {
        panic!("variant metadata should be a record");
    };
    assert_eq!(
        variant_fields.get("source_span"),
        Some(&span_value(Some(Span::new(SourceId::new(8), 90, 100))))
    );
    let single_variant_value = variant_info(&registry, &target, "Active").expect("variant info");
    assert_eq!(
        variant_info(&registry, &quest_type, "Active").expect("type variant info"),
        single_variant_value
    );
    let ReflectValue::ScriptRecord {
        fields: single_variant,
        ..
    } = &single_variant_value
    else {
        panic!("single variant metadata should be a record");
    };
    assert_eq!(
        single_variant.get("name"),
        Some(&ReflectValue::Host(HostValue::String("Active".to_owned())))
    );
    assert_eq!(
        single_variant.get("origin"),
        Some(&ReflectValue::Host(HostValue::String("host".to_owned())))
    );
    assert_eq!(
        origin(&registry, &single_variant_value).expect("variant origin metadata"),
        ReflectValue::Host(HostValue::String("host".to_owned()))
    );
    assert_eq!(
        single_variant.get("source_span"),
        Some(&span_value(Some(Span::new(SourceId::new(8), 90, 100))))
    );
    let ReflectValue::Array(all_variants) = all_variants(&registry) else {
        panic!("variant list should be an array");
    };
    assert_eq!(all_variants.len(), 2);
    let ReflectValue::ScriptRecord {
        fields: variant_list_item,
        ..
    } = &all_variants[0]
    else {
        panic!("variant list item should be a record");
    };
    assert_eq!(
        variant_list_item.get("owner"),
        Some(&ReflectValue::Host(HostValue::String(
            "QuestProgress".to_owned()
        )))
    );
    assert_eq!(
        variant_list_item.get("name"),
        Some(&ReflectValue::Host(HostValue::String("Active".to_owned())))
    );
}
