use super::*;

#[test]
fn script_methods_generates_native_method_metadata() {
    let owner = TypeKey::new(Player::vela_type_id(), "Player");
    let descs = Player::vela_native_method_descs();

    assert_eq!(descs.len(), 7);
    assert_eq!(
        descs[0],
        NativeMethodDesc::new(owner.clone(), method_id("grant_exp"), "grant_exp")
            .param("amount", TypeHint::i64())
            .returns(TypeHint::null())
            .effects(EffectSet::host_write())
            .access(FunctionAccess::public().reflect_callable(true),)
            .attr("domain", "player")
            .docs("Grants copied experience through the host patch path."),
    );
    assert_eq!(
        descs[1],
        NativeMethodDesc::new(owner.clone(), method_id("grant_score"), "grant_score")
            .param("amount", TypeHint::i64())
            .returns(TypeHint::i64())
            .effects(EffectSet::host_write())
            .access(FunctionAccess::public().reflect_callable(true),)
            .docs("Grants copied score through a callable native method."),
    );
    assert_eq!(
        descs[2],
        NativeMethodDesc::new(owner.clone(), method_id("preview_bonus"), "preview_bonus")
            .param("bonus", TypeHint::Any)
            .returns(TypeHint::Any)
            .effects(EffectSet::host_read())
            .access(FunctionAccess::public().reflect_callable(true))
            .docs("Previews an optional copied bonus through a callable native method."),
    );
    assert_eq!(
        descs[3],
        NativeMethodDesc::new(owner.clone(), method_id("sum_score"), "sum_score")
            .param("a", TypeHint::i64())
            .param("b", TypeHint::i64())
            .param("c", TypeHint::i64())
            .param("d", TypeHint::i64())
            .param("e", TypeHint::i64())
            .returns(TypeHint::i64())
            .effects(EffectSet::host_write())
            .access(FunctionAccess::public().reflect_callable(true),)
            .docs("Sums five copied method values through a callable native method."),
    );
    assert_eq!(
        descs[4],
        NativeMethodDesc::new(owner.clone(), method_id("sum6_score"), "sum6_score")
            .param("a", TypeHint::i64())
            .param("b", TypeHint::i64())
            .param("c", TypeHint::i64())
            .param("d", TypeHint::i64())
            .param("e", TypeHint::i64())
            .param("f", TypeHint::i64())
            .returns(TypeHint::i64())
            .effects(EffectSet::host_write())
            .access(FunctionAccess::public().reflect_callable(true),)
            .docs("Sums six copied method values through a callable native method."),
    );
    assert_eq!(
        descs[5],
        NativeMethodDesc::new(
            owner.clone(),
            method_id("checked_preview"),
            "checked_preview"
        )
        .param("ok", TypeHint::boolean())
        .returns(TypeHint::Any)
        .effects(EffectSet::host_read())
        .access(FunctionAccess::public().reflect_callable(true))
        .docs("Previews a dynamic copied Result through a callable native method."),
    );
    assert_eq!(
        descs[6],
        NativeMethodDesc::new(owner.clone(), method_id("inspect_path"), "inspect_path")
            .param("path", TypeHint::PathProxy)
            .returns(TypeHint::i64())
            .effects(EffectSet::host_read())
            .access(FunctionAccess::public().reflect_callable(true))
            .docs("Measures an extra copied path proxy argument."),
    );
    assert_eq!(descs[0].owner, Player::vela_host_type_desc().key);
    assert_eq!(
        <Player as vela_engine::schema::ScriptHostMethodMetadata>::script_host_method_descs(),
        descs,
    );
}

#[test]
fn script_methods_coexists_with_host_schema_metadata() {
    let schema = Player::vela_host_type_desc();
    assert_eq!(
        schema,
        TypeDesc::new(TypeKey::new(Player::vela_type_id(), "Player"))
            .kind(TypeKind::Host)
            .schema_hash(schema.schema_hash.expect("schema hash should be generated"))
            .host_type(Player::vela_host_type_id())
            .attr("module", "game::player")
            .field(
                FieldDesc::new(Player::vela_field_id_level(), "level")
                    .access(
                        vela_reflect::access::FieldAccess::new()
                            .readable(true)
                            .writable(true)
                            .reflect_readable(true)
                            .reflect_writable(true),
                    )
                    .attr("rust_name", "level")
                    .type_hint("u32"),
            ),
    );
}
