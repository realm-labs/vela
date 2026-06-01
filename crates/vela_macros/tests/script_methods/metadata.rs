use super::*;

#[test]
fn script_methods_generates_native_method_metadata() {
    let owner = TypeKey::new(TypeId::new(1001), "Player");
    let descs = Player::vela_native_method_descs();

    assert_eq!(descs.len(), 6);
    assert_eq!(
        descs[0],
        NativeMethodDesc::new(owner.clone(), HostMethodId::new(7), "grant_exp")
            .param("amount", TypeHint::Int)
            .returns(TypeHint::Null)
            .effects(EffectSet::host_write())
            .access(
                FunctionAccess::public()
                    .reflect_callable(true)
                    .require_permission("player.write"),
            )
            .attr("domain", "player")
            .docs("Grants copied experience through the host patch path."),
    );
    assert_eq!(
        descs[1],
        NativeMethodDesc::new(owner.clone(), HostMethodId::new(8), "grant_score")
            .param("amount", TypeHint::Int)
            .returns(TypeHint::Int)
            .effects(EffectSet::host_write())
            .access(
                FunctionAccess::public()
                    .reflect_callable(true)
                    .require_permission("player.write"),
            )
            .docs("Grants copied score through a callable native method."),
    );
    assert_eq!(
        descs[2],
        NativeMethodDesc::new(owner.clone(), HostMethodId::new(9), "preview_bonus")
            .param("bonus", TypeHint::Int)
            .returns(TypeHint::Int)
            .effects(EffectSet::host_read())
            .access(FunctionAccess::public().reflect_callable(true))
            .docs("Previews an optional copied bonus through a callable native method."),
    );
    assert_eq!(
        descs[3],
        NativeMethodDesc::new(owner.clone(), HostMethodId::new(10), "sum_score")
            .param("a", TypeHint::Int)
            .param("b", TypeHint::Int)
            .param("c", TypeHint::Int)
            .param("d", TypeHint::Int)
            .param("e", TypeHint::Int)
            .returns(TypeHint::Int)
            .effects(EffectSet::host_write())
            .access(
                FunctionAccess::public()
                    .reflect_callable(true)
                    .require_permission("player.write"),
            )
            .docs("Sums five copied method values through a callable native method."),
    );
    assert_eq!(
        descs[4],
        NativeMethodDesc::new(owner.clone(), HostMethodId::new(12), "sum6_score")
            .param("a", TypeHint::Int)
            .param("b", TypeHint::Int)
            .param("c", TypeHint::Int)
            .param("d", TypeHint::Int)
            .param("e", TypeHint::Int)
            .param("f", TypeHint::Int)
            .returns(TypeHint::Int)
            .effects(EffectSet::host_write())
            .access(
                FunctionAccess::public()
                    .reflect_callable(true)
                    .require_permission("player.write"),
            )
            .docs("Sums six copied method values through a callable native method."),
    );
    assert_eq!(
        descs[5],
        NativeMethodDesc::new(owner.clone(), HostMethodId::new(11), "checked_preview")
            .param("ok", TypeHint::Bool)
            .returns(TypeHint::Any)
            .effects(EffectSet::host_read())
            .access(FunctionAccess::public().reflect_callable(true))
            .docs("Previews a dynamic copied Result through a callable native method."),
    );
    assert_eq!(descs[0].owner, Player::vela_host_type_desc().key);
    assert_eq!(
        <Player as vela_engine::ScriptHostMethodMetadata>::script_host_method_descs(),
        descs,
    );
}

#[test]
fn script_methods_coexists_with_host_schema_metadata() {
    let schema = Player::vela_host_type_desc();
    assert_eq!(
        schema,
        TypeDesc::new(TypeKey::new(TypeId::new(1001), "Player"))
            .kind(TypeKind::Host)
            .schema_hash(schema.schema_hash.expect("schema hash should be generated"))
            .host_type(HostTypeId::new(1001))
            .field(
                FieldDesc::new(FieldId::new(1), "level")
                    .access(
                        vela_reflect::FieldAccess::new()
                            .readable(true)
                            .writable(true)
                            .reflect_readable(true)
                            .reflect_writable(true),
                    )
                    .attr("rust_name", "level")
                    .type_hint("int"),
            ),
    );
}
