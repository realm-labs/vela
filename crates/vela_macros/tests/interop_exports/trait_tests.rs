use super::*;

#[test]
fn explicit_trait_impl_exports_install_ufcs_method_thunks() {
    let mut runtime = host_export_runtime(
        "fn main(player: Player) { player.take_damage(3); return player.is_alive(); }",
    );
    let mut player = Player { level: 5 };

    let result = runtime
        .call(
            "main",
            CallArgs::new().with_host_mut("player", &mut player),
            CallOptions::unbounded(),
        )
        .expect("explicit trait implementation exports should execute");

    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
    assert_eq!(player.level, 2);
}

#[test]
fn declaration_only_external_trait_adapter_calls_existing_impl() {
    let engine = Engine::builder()
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .install_generated_type::<ExternalNpc>()
        .install_registration(ExternalNpc::vela_methods())
        .install_registration(VelaExternalExternalNpcExternalDamageExports::vela_methods())
        .build()
        .expect("declaration-only adapter should register");
    let program = engine
        .compile_source(
            "fn main(npc: Npc) { npc.hit(3); return npc.active() && npc.current_hp() == 2; }",
        )
        .expect("external trait methods should compile as ordinary methods");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let mut npc = ExternalNpc { hp: 5 };

    let result = runtime
        .call(
            "main",
            CallArgs::new().with_host_mut("npc", &mut npc),
            CallOptions::unbounded(),
        )
        .expect("generated UFCS thunks should call the existing trait impl");

    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
    assert_eq!(npc.hp, 2);
}
