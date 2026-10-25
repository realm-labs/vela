use super::*;

#[test]
fn method_call_policy_preserves_visibility_permissions_and_effect_checks() {
    let method = MethodDesc::new(MethodId::new(5), "grant_exp")
        .effects(MethodEffectSet::host_write())
        .access(
            MethodAccess::new()
                .public(false)
                .require_permission("player.admin"),
        );
    let call_only =
        ReflectPolicy::new(ReflectPermissionSet::new().with(ReflectPermission::CallMethods));
    assert_eq!(
        call_only
            .require_method_call_access("Player", &method)
            .expect_err("policy must reject this call")
            .kind,
        ReflectErrorKind::PermissionDenied {
            permission: ReflectPermission::AccessPrivate
        },
    );
    let private = call_only.with_permissions(
        ReflectPermissionSet::new()
            .with(ReflectPermission::CallMethods)
            .with(ReflectPermission::AccessPrivate),
    );
    assert!(matches!(
        private
            .require_method_call_access("Player", &method)
            .expect_err("policy must reject this call")
            .kind,
        ReflectErrorKind::MethodPermissionDenied { .. },
    ));
    let authorized = private.with_method_permission("player.admin");
    assert_eq!(
        authorized
            .require_method_call_access("Player", &method)
            .expect_err("policy must reject this call")
            .kind,
        ReflectErrorKind::MethodEffectPermissionDenied {
            method: "grant_exp".to_owned(),
            permission: ReflectPermission::CallHostWriteMethods,
            source_span: None,
        },
    );
    let read = authorized.with_permissions(
        ReflectPermissionSet::new()
            .with(ReflectPermission::CallMethods)
            .with(ReflectPermission::AccessPrivate)
            .with(ReflectPermission::CallHostReadMethods),
    );
    assert_eq!(
        read.require_method_call_access("Player", &method)
            .expect_err("policy must reject this call")
            .kind,
        ReflectErrorKind::MethodEffectPermissionDenied {
            method: "grant_exp".to_owned(),
            permission: ReflectPermission::CallHostWriteMethods,
            source_span: None,
        },
    );
    assert!(
        ReflectPolicy::all()
            .with_method_permission("player.admin")
            .require_method_call_access("Player", &method)
            .is_ok()
    );
}

#[test]
fn method_call_policy_rejects_non_callable_and_missing_call_permission() {
    let method = MethodDesc::new(MethodId::new(5), "grant_exp");
    assert_eq!(
        ReflectPolicy::read_only()
            .require_method_call_access("Player", &method)
            .expect_err("policy must reject this call")
            .kind,
        ReflectErrorKind::PermissionDenied {
            permission: ReflectPermission::CallMethods
        },
    );
    let hidden = method.access(MethodAccess::new().reflect_callable(false));
    assert!(matches!(
        ReflectPolicy::all()
            .require_method_call_access("Player", &hidden)
            .expect_err("policy must reject this call")
            .kind,
        ReflectErrorKind::MethodNotReflectCallable { .. },
    ));
}
