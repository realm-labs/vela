use super::*;

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
            method: "grant_exp".to_owned(),
            source_span: None,
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
            permission: "player.admin".to_owned(),
            source_span: None,
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
            permission: ReflectPermission::CallHostWriteMethods,
            source_span: None,
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
fn reflect_call_with_policy_filters_unknown_method_candidates() {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(101), "Player"))
            .host_type(HostTypeId::new(1))
            .method(
                MethodDesc::new(HostMethodId::new(5), "grant_exp")
                    .access(MethodAccess::new().reflect_callable(true)),
            )
            .method(
                MethodDesc::new(HostMethodId::new(6), "grant_exp_hidden")
                    .access(MethodAccess::new().reflect_callable(false)),
            )
            .method(
                MethodDesc::new(HostMethodId::new(7), "grant_exp_private")
                    .access(MethodAccess::new().public(false).reflect_callable(true)),
            )
            .method(
                MethodDesc::new(HostMethodId::new(8), "grant_exp_admin")
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
        "grant_exp_hiddden",
        Vec::new(),
        &ReflectPolicy::read_only(),
    )
    .expect_err("unknown method");

    assert_eq!(
        error.kind,
        ReflectErrorKind::UnknownMethod {
            type_name: "Player".to_owned(),
            method: "grant_exp_hiddden".to_owned(),
            candidates: vec!["grant_exp".to_owned()],
            related: vec![ReflectCandidate::new("grant_exp", None)],
        }
    );
    assert!(ctx.tx.patches().is_empty());
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
