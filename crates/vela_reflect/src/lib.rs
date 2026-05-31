//! Controlled reflection metadata and value access.

mod access;
mod candidates;
mod descriptor_targets;
mod error;
mod error_diagnostics;
mod members;
mod metadata;
mod metadata_records;
mod modules;
mod permissions;
mod registry;
mod script_attrs;
mod script_types;
mod types;
mod value;
mod value_access;

pub use access::{FieldAccess, FunctionAccess, FunctionEffectSet, MethodAccess, MethodEffectSet};
pub use candidates::ReflectCandidate;
pub use error::{ReflectError, ReflectErrorKind, ReflectResult};
pub use members::{
    access as access_metadata, all_fields as field_metadata_list,
    all_fields_with_policy as field_metadata_list_with_policy, all_methods as method_metadata_list,
    all_methods_with_policy as method_metadata_list_with_policy, all_traits as trait_metadata_list,
    all_variants as variant_metadata_list,
    all_variants_with_policy as variant_metadata_list_with_policy, attr as attr_metadata,
    attrs as attrs_metadata, docs as docs_metadata, effects as effects_metadata,
    field as field_metadata, field_with_policy as field_metadata_with_policy,
    fields_with_policy as field_metadata_for_target_with_policy, has_attr as has_attr_metadata,
    has_field, has_field_with_policy, has_method, has_method_with_policy, has_trait, has_variant,
    id as id_metadata, kind as kind_metadata, method as method_metadata,
    method_with_policy as method_metadata_with_policy, methods, methods_with_policy,
    name as name_metadata, origin as origin_metadata, owner as owner_metadata,
    params as params_metadata, required_permissions as required_permissions_metadata,
    returns as returns_metadata, source_span as source_span_metadata,
    trait_by_name as trait_metadata_by_name, traits as trait_metadata, variant, variant_info,
    variant_info_with_policy, variant_is, variants as variant_metadata,
    variants_with_policy as variant_metadata_with_policy,
};
pub use modules::{
    DeclOrigin, FunctionDesc, FunctionParamDesc, ModuleDesc, ModuleExportDesc, ModuleExportKind,
    exports as module_exports, exports_for_target as module_exports_for_target,
    exports_for_target_with_policy as module_exports_for_target_with_policy,
    exports_with_policy as module_exports_with_policy, function as function_metadata,
    function_with_policy as function_metadata_with_policy, functions as function_metadata_list,
    functions_with_policy as function_metadata_list_with_policy, has_function,
    has_function_with_policy, has_module, has_module_with_policy, module as module_metadata,
    module_with_policy as module_metadata_with_policy, modules as module_metadata_list,
    modules_with_policy as module_metadata_list_with_policy,
};
pub use permissions::{
    ReflectLookupBudget, ReflectPermission, ReflectPermissionSet, ReflectPolicy, has_permission,
    permission_names,
};
pub use registry::{
    AttrMap, FieldDesc, MethodDesc, MethodParamDesc, SchemaHash, TraitDesc, TraitMethodDesc,
    TypeDesc, TypeKey, TypeKind, TypeRegistry, VariantDesc,
};
pub use types::{has_type, type_by_name as type_metadata_by_name, type_list as type_metadata_list};
pub use value::{
    ReflectContext, ReflectValue, call, call_with_policy, fields, get, get_with_policy, implements,
    set, set_with_policy, type_of,
};

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use vela_common::{FieldId, HostMethodId, HostObjectId, HostTypeId, SourceId, Span, TypeId};
    use vela_host::{
        HostObjectSnapshot, HostPath, HostRef, HostValue, MockStateAdapter, PatchOp, PatchTx,
    };

    fn player_ref() -> HostRef {
        HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 3)
    }

    fn registry() -> TypeRegistry {
        let mut registry = TypeRegistry::new();
        registry.register_trait(TraitDesc::new("Trackable"));
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
                .host_type(HostTypeId::new(1))
                .field(FieldDesc::new(FieldId::new(1), "id"))
                .field(FieldDesc::new(FieldId::new(2), "level").writable(true))
                .method(MethodDesc::new(HostMethodId::new(5), "grant_exp"))
                .trait_impl(TraitDesc::new("Damageable")),
        );
        registry
    }

    fn adapter_with_level(value: HostValue) -> MockStateAdapter {
        let mut adapter = MockStateAdapter::new();
        adapter.insert_value(HostPath::new(player_ref()).field(FieldId::new(2)), value);
        adapter
    }

    fn trait_name(name: &str) -> ReflectValue {
        ReflectValue::Host(HostValue::String(name.to_owned()))
    }

    #[test]
    fn reflect_set_host_ref_creates_patch() {
        let registry = registry();
        let adapter = adapter_with_level(HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut ctx = ReflectContext {
            registry: &registry,
            adapter: &adapter,
            tx: &mut tx,
        };

        set(
            &mut ctx,
            &ReflectValue::HostRef(player_ref()),
            "level",
            ReflectValue::Host(HostValue::Int(10)),
        )
        .expect("reflect set");

        assert_eq!(ctx.tx.patches().len(), 1);
        assert_eq!(ctx.tx.patches()[0].op, PatchOp::Set(HostValue::Int(10)));
    }

    #[test]
    fn reflect_get_host_ref_reads_overlay_before_adapter() {
        let registry = registry();
        let adapter = adapter_with_level(HostValue::Int(9));
        let mut tx = PatchTx::new();
        tx.set_path(
            HostPath::new(player_ref()).field(FieldId::new(2)),
            HostValue::Int(12),
            Some(Span::new(SourceId::new(1), 0, 1)),
        )
        .expect("set overlay");
        let mut ctx = ReflectContext {
            registry: &registry,
            adapter: &adapter,
            tx: &mut tx,
        };

        let value =
            get(&mut ctx, &ReflectValue::HostRef(player_ref()), "level").expect("reflect get");

        assert_eq!(value, ReflectValue::Host(HostValue::Int(12)));
    }

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
    fn reflect_set_read_only_host_field_fails() {
        let registry = registry();
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
            "id",
            ReflectValue::Host(HostValue::Int(10)),
        )
        .expect_err("read-only set");

        assert_eq!(
            error.kind,
            ReflectErrorKind::FieldNotWritable {
                type_name: "Player".to_owned(),
                field: "id".to_owned()
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

        let error =
            get(&mut ctx, &ReflectValue::HostRef(player_ref()), "secret").expect_err("read");

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
                fields: BTreeMap::from([(
                    "level".to_owned(),
                    ReflectValue::Host(HostValue::Int(10)),
                )]),
            }
        );
        assert!(ctx.tx.patches().is_empty());
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

        let error = get(&mut ctx, &ReflectValue::HostRef(player_ref()), "levle")
            .expect_err("unknown field");

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

        let error =
            get(&mut ctx, &ReflectValue::HostRef(stale_ref), "level").expect_err("stale get");

        assert!(matches!(error.kind, ReflectErrorKind::Host(_)));
        assert_eq!(
            vela_host::PatchTx::require_fresh_ref(
                stale_ref,
                &HostObjectSnapshot {
                    type_id: fresh_ref.type_id,
                    object_id: fresh_ref.object_id,
                    generation: 3,
                }
            )
            .expect_err("stale ref")
            .kind,
            vela_host::HostErrorKind::StaleGeneration {
                expected: 2,
                actual: 3
            }
        );
    }

    #[test]
    fn reflect_call_host_ref_records_patch() {
        let registry = registry();
        let mut adapter = adapter_with_level(HostValue::Int(9));
        adapter.insert_method_return(HostMethodId::new(5), HostValue::Null);
        let mut tx = PatchTx::new();
        {
            let mut ctx = ReflectContext {
                registry: &registry,
                adapter: &adapter,
                tx: &mut tx,
            };

            let value = call(
                &mut ctx,
                &ReflectValue::HostRef(player_ref()),
                "grant_exp",
                vec![ReflectValue::Host(HostValue::Int(20))],
            )
            .expect("reflect call");

            assert_eq!(value, ReflectValue::Host(HostValue::Null));
            assert_eq!(ctx.tx.patches().len(), 1);
            assert_eq!(
                ctx.tx.patches()[0].op,
                PatchOp::CallHostMethod {
                    method: HostMethodId::new(5),
                    args: vec![HostValue::Int(20)]
                }
            );
            assert!(adapter.method_calls().is_empty());
        }

        tx.apply(&mut adapter).expect("apply reflect call");
        assert_eq!(
            adapter.method_calls(),
            &[(
                HostPath::new(player_ref()),
                HostMethodId::new(5),
                vec![HostValue::Int(20)]
            )]
        );
    }

    #[test]
    fn reflect_call_rejects_non_host_args() {
        let registry = registry();
        let adapter = adapter_with_level(HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut ctx = ReflectContext {
            registry: &registry,
            adapter: &adapter,
            tx: &mut tx,
        };

        let error = call(
            &mut ctx,
            &ReflectValue::HostRef(player_ref()),
            "grant_exp",
            vec![ReflectValue::Record(BTreeMap::new())],
        )
        .expect_err("invalid arg");

        assert_eq!(error.kind, ReflectErrorKind::InvalidValue);
        assert!(ctx.tx.patches().is_empty());
    }

    #[test]
    fn reflect_call_with_policy_denies_unapproved_methods_before_patch() {
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
                .host_type(HostTypeId::new(1))
                .method(
                    MethodDesc::new(HostMethodId::new(5), "grant_exp")
                        .access(MethodAccess::new().reflect_callable(false)),
                )
                .method(
                    MethodDesc::new(HostMethodId::new(6), "admin_grant")
                        .access(MethodAccess::new().require_permission("player.admin")),
                ),
        );
        let adapter = adapter_with_level(HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut ctx = ReflectContext {
            registry: &registry,
            adapter: &adapter,
            tx: &mut tx,
        };

        let error = call_with_policy(
            &mut ctx,
            &ReflectValue::HostRef(player_ref()),
            "grant_exp",
            vec![ReflectValue::Host(HostValue::Int(20))],
            &ReflectPolicy::all(),
        )
        .expect_err("not reflect callable");
        assert_eq!(
            error.kind,
            ReflectErrorKind::MethodNotReflectCallable {
                type_name: "Player".to_owned(),
                method: "grant_exp".to_owned()
            }
        );
        assert!(ctx.tx.patches().is_empty());

        let error = call_with_policy(
            &mut ctx,
            &ReflectValue::HostRef(player_ref()),
            "admin_grant",
            vec![ReflectValue::Host(HostValue::Int(20))],
            &ReflectPolicy::all(),
        )
        .expect_err("missing method permission");
        assert_eq!(
            error.kind,
            ReflectErrorKind::MethodPermissionDenied {
                method: "admin_grant".to_owned(),
                permission: "player.admin".to_owned()
            }
        );
        assert!(ctx.tx.patches().is_empty());
    }

    #[test]
    fn reflect_call_with_policy_denies_effectful_methods_without_effect_permission() {
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
                .host_type(HostTypeId::new(1))
                .method(
                    MethodDesc::new(HostMethodId::new(5), "grant_exp")
                        .effects(MethodEffectSet::host_write())
                        .access(MethodAccess::new().reflect_callable(true)),
                ),
        );
        let adapter = adapter_with_level(HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut ctx = ReflectContext {
            registry: &registry,
            adapter: &adapter,
            tx: &mut tx,
        };
        let policy = ReflectPolicy::new(
            ReflectPermissionSet::new()
                .with(ReflectPermission::CallMethods)
                .with(ReflectPermission::CallHostReadMethods)
                .with(ReflectPermission::InspectHostPath),
        );

        let error = call_with_policy(
            &mut ctx,
            &ReflectValue::HostRef(player_ref()),
            "grant_exp",
            vec![ReflectValue::Host(HostValue::Int(20))],
            &policy,
        )
        .expect_err("host-write method should require effect permission");

        assert_eq!(
            error.kind,
            ReflectErrorKind::MethodEffectPermissionDenied {
                method: "grant_exp".to_owned(),
                permission: ReflectPermission::CallHostWriteMethods
            }
        );
        assert!(ctx.tx.patches().is_empty());

        let allowed_permissions = policy
            .permissions()
            .clone()
            .with(ReflectPermission::CallHostWriteMethods);
        let policy = policy.with_permissions(allowed_permissions);
        let value = call_with_policy(
            &mut ctx,
            &ReflectValue::HostRef(player_ref()),
            "grant_exp",
            vec![ReflectValue::Host(HostValue::Int(20))],
            &policy,
        )
        .expect("effect permission should allow method call");

        assert_eq!(value, ReflectValue::Host(HostValue::Null));
        assert_eq!(ctx.tx.patches().len(), 1);
    }

    #[test]
    fn reflect_call_with_policy_denies_private_methods_without_permission() {
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
                .host_type(HostTypeId::new(1))
                .method(
                    MethodDesc::new(HostMethodId::new(5), "admin_grant").access(
                        MethodAccess::new()
                            .public(false)
                            .reflect_callable(true)
                            .require_permission("player.admin"),
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
        let policy = ReflectPolicy::new(
            ReflectPermissionSet::new()
                .with(ReflectPermission::CallMethods)
                .with(ReflectPermission::InspectHostPath),
        )
        .with_method_permission("player.admin");

        let error = call_with_policy(
            &mut ctx,
            &ReflectValue::HostRef(player_ref()),
            "admin_grant",
            vec![ReflectValue::Host(HostValue::Int(20))],
            &policy,
        )
        .expect_err("private method should require AccessPrivate");

        assert_eq!(
            error.kind,
            ReflectErrorKind::PermissionDenied {
                permission: ReflectPermission::AccessPrivate
            }
        );
        assert!(ctx.tx.patches().is_empty());
    }

    #[test]
    fn reflect_call_with_policy_allows_private_methods_with_permission() {
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
                .host_type(HostTypeId::new(1))
                .method(
                    MethodDesc::new(HostMethodId::new(5), "admin_grant").access(
                        MethodAccess::new()
                            .public(false)
                            .reflect_callable(true)
                            .require_permission("player.admin"),
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
        let policy = ReflectPolicy::new(
            ReflectPermissionSet::new()
                .with(ReflectPermission::CallMethods)
                .with(ReflectPermission::AccessPrivate)
                .with(ReflectPermission::InspectHostPath),
        )
        .with_method_permission("player.admin");

        let value = call_with_policy(
            &mut ctx,
            &ReflectValue::HostRef(player_ref()),
            "admin_grant",
            vec![ReflectValue::Host(HostValue::Int(20))],
            &policy,
        )
        .expect("private method call");

        assert_eq!(value, ReflectValue::Host(HostValue::Null));
        assert_eq!(ctx.tx.patches().len(), 1);
        assert_eq!(
            ctx.tx.patches()[0].op,
            PatchOp::CallHostMethod {
                method: HostMethodId::new(5),
                args: vec![HostValue::Int(20)]
            }
        );
    }

    #[test]
    fn unknown_methods_include_candidate_hints() {
        let registry = registry();
        let adapter = adapter_with_level(HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut ctx = ReflectContext {
            registry: &registry,
            adapter: &adapter,
            tx: &mut tx,
        };

        let error = call(
            &mut ctx,
            &ReflectValue::HostRef(player_ref()),
            "grant_xp",
            Vec::new(),
        )
        .expect_err("unknown method");

        assert_eq!(
            error.kind,
            ReflectErrorKind::UnknownMethod {
                type_name: "Player".to_owned(),
                method: "grant_xp".to_owned(),
                candidates: vec!["grant_exp".to_owned()],
                related: vec![ReflectCandidate::new("grant_exp", None)],
            }
        );
    }

    #[test]
    fn reflect_implements_uses_registry_metadata() {
        let registry = registry();

        assert!(
            implements(
                &registry,
                &ReflectValue::HostRef(player_ref()),
                &trait_name("Damageable"),
            )
            .expect("implements check")
        );
        assert!(
            !implements(
                &registry,
                &ReflectValue::HostRef(player_ref()),
                &trait_name("Trackable")
            )
            .expect("known unimplemented trait check")
        );
        let player_type = type_metadata_by_name(&registry, "Player").expect("type metadata");
        assert!(
            implements(&registry, &player_type, &trait_name("Damageable"))
                .expect("copied type descriptor implements check")
        );
        assert!(
            !implements(&registry, &player_type, &trait_name("Trackable"))
                .expect("copied type descriptor negative implements check")
        );
        let damageable_trait =
            trait_metadata_by_name(&registry, "Damageable").expect("trait metadata");
        assert!(
            implements(&registry, &player_type, &damageable_trait)
                .expect("copied trait descriptor implements check")
        );

        let error = implements(
            &registry,
            &ReflectValue::HostRef(player_ref()),
            &trait_name("Damagable"),
        )
        .expect_err("unknown trait should diagnose");
        assert_eq!(
            error.kind,
            ReflectErrorKind::UnknownTrait {
                trait_name: "Damagable".to_owned(),
                candidates: vec!["Damageable".to_owned(), "Trackable".to_owned()],
                related: vec![
                    ReflectCandidate::new("Damageable", None),
                    ReflectCandidate::new("Trackable", None),
                ],
            }
        );
    }

    #[test]
    fn reflect_implements_uses_script_type_metadata() {
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(200), "game.Player"))
                .kind(TypeKind::ScriptStruct)
                .trait_impl(TraitDesc::new("game.Damageable")),
        );
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(201), "game.Progress"))
                .kind(TypeKind::ScriptEnum)
                .trait_impl(TraitDesc::new("game.Trackable")),
        );

        assert!(
            implements(
                &registry,
                &ReflectValue::ScriptRecord {
                    type_name: "game.Player".to_owned(),
                    fields: BTreeMap::new(),
                },
                &trait_name("game.Damageable"),
            )
            .expect("script record implements check")
        );
        assert!(
            implements(
                &registry,
                &ReflectValue::ScriptEnum {
                    enum_name: "game.Progress".to_owned(),
                    variant: "Active".to_owned(),
                    fields: BTreeMap::new(),
                },
                &trait_name("game.Trackable"),
            )
            .expect("script enum implements check")
        );
        assert!(
            !implements(
                &registry,
                &ReflectValue::ScriptRecord {
                    type_name: "game.Player".to_owned(),
                    fields: BTreeMap::new(),
                },
                &trait_name("game.Trackable"),
            )
            .expect("script record negative implements check")
        );
    }
}
