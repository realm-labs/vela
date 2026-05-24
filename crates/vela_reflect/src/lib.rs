//! Controlled reflection metadata and value access.

mod access;
mod error;
mod members;
mod metadata;
mod modules;
mod permissions;
mod registry;
mod script_attrs;
mod script_types;
mod types;

use std::collections::BTreeMap;

pub use access::{FieldAccess, FunctionAccess, FunctionEffectSet, MethodAccess, MethodEffectSet};
pub use error::{ReflectError, ReflectErrorKind, ReflectResult};
pub use members::{
    attrs as attrs_metadata, docs as docs_metadata, field as field_metadata,
    field_names_with_policy, field_with_policy as field_metadata_with_policy, has_field,
    has_field_with_policy, has_method, has_method_with_policy, kind as kind_metadata, methods,
    methods_with_policy, name as name_metadata, trait_by_name as trait_metadata_by_name,
    traits as trait_metadata, variant, variant_is, variants as variant_metadata,
    variants_with_policy as variant_metadata_with_policy,
};
pub use modules::{
    DeclOrigin, FunctionDesc, FunctionParamDesc, ModuleDesc, ModuleExportDesc, ModuleExportKind,
    exports as module_exports, exports_with_policy as module_exports_with_policy,
    function as function_metadata, function_with_policy as function_metadata_with_policy,
    module as module_metadata, module_with_policy as module_metadata_with_policy,
};
pub use permissions::{
    ReflectLookupBudget, ReflectPermission, ReflectPermissionSet, ReflectPolicy,
};
pub use registry::{
    AttrMap, FieldDesc, MethodDesc, SchemaHash, TraitDesc, TraitMethodDesc, TypeDesc, TypeKey,
    TypeKind, TypeRegistry, VariantDesc,
};
pub use types::{type_by_name as type_metadata_by_name, type_names as type_metadata_names};
use vela_host::{HostPath, HostRef, HostValue, PatchTx, ScriptStateAdapter};

#[derive(Clone, Debug, PartialEq)]
pub enum ReflectValue {
    Host(HostValue),
    HostRef(HostRef),
    Record(BTreeMap<String, ReflectValue>),
    ScriptRecord {
        type_name: String,
        fields: BTreeMap<String, ReflectValue>,
    },
    ScriptEnum {
        enum_name: String,
        variant: String,
        fields: BTreeMap<String, ReflectValue>,
    },
}

pub struct ReflectContext<'a> {
    pub registry: &'a TypeRegistry,
    pub adapter: &'a dyn ScriptStateAdapter,
    pub tx: &'a mut PatchTx,
}

pub fn type_of<'a>(registry: &'a TypeRegistry, value: &ReflectValue) -> Option<&'a TypeDesc> {
    match value {
        ReflectValue::HostRef(host_ref) => registry.type_of_host(*host_ref),
        ReflectValue::ScriptRecord { type_name, .. } => registry.type_by_name(type_name),
        ReflectValue::ScriptEnum { enum_name, .. } => registry.type_by_name(enum_name),
        ReflectValue::Host(_) | ReflectValue::Record(_) => None,
    }
}

pub fn fields<'a>(registry: &'a TypeRegistry, key: &TypeKey) -> Option<&'a [FieldDesc]> {
    registry.fields(key)
}

pub fn get(
    ctx: &mut ReflectContext<'_>,
    target: &ReflectValue,
    field: &str,
) -> ReflectResult<ReflectValue> {
    match target {
        ReflectValue::HostRef(host_ref) => {
            let field_desc = ctx.registry.host_field(*host_ref, field)?;
            let type_name = ctx
                .registry
                .type_of_host(*host_ref)
                .map_or_else(|| "<unknown>".to_owned(), |desc| desc.key.name.clone());
            if !field_desc.access.reflect_readable {
                return Err(ReflectError::new(
                    ReflectErrorKind::FieldNotReflectReadable {
                        type_name,
                        field: field.to_owned(),
                    },
                ));
            }
            let value = ctx
                .tx
                .read_path(ctx.adapter, &HostPath::new(*host_ref).field(field_desc.id))
                .map_err(|error| ReflectError::new(ReflectErrorKind::Host(error.to_string())))?;
            Ok(ReflectValue::Host(value))
        }
        ReflectValue::Record(record) => get_record_field(field, record),
        ReflectValue::ScriptRecord { fields, .. } | ReflectValue::ScriptEnum { fields, .. } => {
            get_record_field(field, fields)
        }
        ReflectValue::Host(_) => Err(ReflectError::new(ReflectErrorKind::InvalidTarget)),
    }
}

pub fn set(
    ctx: &mut ReflectContext<'_>,
    target: &ReflectValue,
    field: &str,
    value: ReflectValue,
) -> ReflectResult<()> {
    match target {
        ReflectValue::HostRef(host_ref) => {
            let field_desc = ctx.registry.host_field(*host_ref, field)?;
            let type_name = ctx
                .registry
                .type_of_host(*host_ref)
                .map_or_else(|| "<unknown>".to_owned(), |desc| desc.key.name.clone());
            if !field_desc.writable {
                return Err(ReflectError::new(ReflectErrorKind::FieldNotWritable {
                    type_name,
                    field: field.to_owned(),
                }));
            }
            if !field_desc.access.reflect_writable {
                return Err(ReflectError::new(
                    ReflectErrorKind::FieldNotReflectWritable {
                        type_name,
                        field: field.to_owned(),
                    },
                ));
            }
            let ReflectValue::Host(value) = value else {
                return Err(ReflectError::new(ReflectErrorKind::InvalidValue));
            };
            ctx.tx
                .set_path(HostPath::new(*host_ref).field(field_desc.id), value, None)
                .map_err(|error| ReflectError::new(ReflectErrorKind::Host(error.to_string())))?;
            Ok(())
        }
        ReflectValue::Record(_)
        | ReflectValue::ScriptRecord { .. }
        | ReflectValue::ScriptEnum { .. }
        | ReflectValue::Host(_) => Err(ReflectError::new(ReflectErrorKind::InvalidTarget)),
    }
}

pub fn call(
    ctx: &mut ReflectContext<'_>,
    target: &ReflectValue,
    method: &str,
    args: Vec<ReflectValue>,
) -> ReflectResult<ReflectValue> {
    call_impl(ctx, target, method, args, None)
}

pub fn call_with_policy(
    ctx: &mut ReflectContext<'_>,
    target: &ReflectValue,
    method: &str,
    args: Vec<ReflectValue>,
    policy: &ReflectPolicy,
) -> ReflectResult<ReflectValue> {
    call_impl(ctx, target, method, args, Some(policy))
}

fn call_impl(
    ctx: &mut ReflectContext<'_>,
    target: &ReflectValue,
    method: &str,
    args: Vec<ReflectValue>,
    policy: Option<&ReflectPolicy>,
) -> ReflectResult<ReflectValue> {
    let ReflectValue::HostRef(host_ref) = target else {
        return Err(ReflectError::new(ReflectErrorKind::InvalidTarget));
    };
    let type_name = ctx
        .registry
        .type_of_host(*host_ref)
        .map_or_else(|| "<unknown>".to_owned(), |desc| desc.key.name.clone());
    let method_desc = ctx.registry.host_method(*host_ref, method)?;
    if let Some(policy) = policy {
        policy.require_method_access(&type_name, method_desc)?;
    }
    let args = args
        .into_iter()
        .map(host_arg)
        .collect::<ReflectResult<Vec<_>>>()?;
    ctx.tx
        .call_method(HostPath::new(*host_ref), method_desc.id, args, None)
        .map_err(|error| ReflectError::new(ReflectErrorKind::Host(error.to_string())))?;
    Ok(ReflectValue::Host(HostValue::Null))
}

pub fn implements(
    registry: &TypeRegistry,
    target: &ReflectValue,
    trait_name: &str,
) -> ReflectResult<bool> {
    let known_traits = registry.known_trait_names();
    if !known_traits.iter().any(|candidate| candidate == trait_name) {
        return Err(ReflectError::new(ReflectErrorKind::UnknownTrait {
            trait_name: trait_name.to_owned(),
            candidates: name_candidates(trait_name, known_traits.iter().map(String::as_str)),
        }));
    }

    match target {
        ReflectValue::HostRef(host_ref) => {
            let desc = registry.type_of_host(*host_ref).ok_or_else(|| {
                ReflectError::new(ReflectErrorKind::UnknownType {
                    host_type_id: host_ref.type_id,
                })
            })?;
            Ok(desc
                .traits
                .iter()
                .any(|trait_desc| trait_desc.name == trait_name))
        }
        ReflectValue::ScriptRecord { type_name, .. }
        | ReflectValue::ScriptEnum {
            enum_name: type_name,
            ..
        } => {
            let Some(desc) = registry.type_by_name(type_name) else {
                return Ok(false);
            };
            Ok(desc
                .traits
                .iter()
                .any(|trait_desc| trait_desc.name == trait_name))
        }
        ReflectValue::Host(_) | ReflectValue::Record(_) => {
            Err(ReflectError::new(ReflectErrorKind::InvalidTarget))
        }
    }
}

fn get_record_field(
    field: &str,
    record: &BTreeMap<String, ReflectValue>,
) -> ReflectResult<ReflectValue> {
    record
        .get(field)
        .cloned()
        .ok_or_else(|| ReflectError::new(record_unknown_field(field, record)))
}

fn host_arg(value: ReflectValue) -> ReflectResult<HostValue> {
    let ReflectValue::Host(value) = value else {
        return Err(ReflectError::new(ReflectErrorKind::InvalidValue));
    };
    Ok(value)
}

fn record_unknown_field(field: &str, record: &BTreeMap<String, ReflectValue>) -> ReflectErrorKind {
    ReflectErrorKind::UnknownField {
        type_name: "record".to_owned(),
        field: field.to_owned(),
        candidates: name_candidates(field, record.keys().map(String::as_str)),
    }
}

pub(crate) fn name_candidates<'a>(
    name: &str,
    candidates: impl Iterator<Item = &'a str>,
) -> Vec<String> {
    let mut candidates = candidates
        .map(|candidate| (edit_distance(name, candidate), candidate.to_owned()))
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    candidates
        .into_iter()
        .take(3)
        .map(|(_, candidate)| candidate)
        .collect()
}

fn edit_distance(left: &str, right: &str) -> usize {
    let left = left.chars().collect::<Vec<_>>();
    let right = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right.len()).collect::<Vec<_>>();
    let mut current = vec![0; right.len() + 1];

    for (left_index, left_ch) in left.iter().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_ch) in right.iter().enumerate() {
            let substitution = usize::from(left_ch != right_ch);
            current[right_index + 1] = (previous[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(previous[right_index] + substitution);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[right.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_common::{FieldId, HostMethodId, HostObjectId, HostTypeId, SourceId, Span, TypeId};
    use vela_host::{HostObjectSnapshot, MockStateAdapter, PatchOp};

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
                candidates: vec!["level".to_owned(), "id".to_owned()]
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
                candidates: vec!["grant_exp".to_owned()]
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
                "Damageable"
            )
            .expect("implements check")
        );
        assert!(
            !implements(&registry, &ReflectValue::HostRef(player_ref()), "Trackable")
                .expect("known unimplemented trait check")
        );

        let error = implements(&registry, &ReflectValue::HostRef(player_ref()), "Damagable")
            .expect_err("unknown trait should diagnose");
        assert_eq!(
            error.kind,
            ReflectErrorKind::UnknownTrait {
                trait_name: "Damagable".to_owned(),
                candidates: vec!["Damageable".to_owned(), "Trackable".to_owned()]
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
                "game.Damageable",
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
                "game.Trackable",
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
                "game.Trackable",
            )
            .expect("script record negative implements check")
        );
    }
}
