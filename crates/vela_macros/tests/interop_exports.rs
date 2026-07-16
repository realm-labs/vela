use vela_engine::context::NativeCallContext;
use vela_engine::interop::{BoundaryMode, CallableKind};
use vela_engine::native::EffectSet;
use vela_macros::{ScriptHost, export, methods, trait_export};
use vela_vm::error::VmResult;

#[derive(Debug, ScriptHost)]
#[script(path = "game::Player")]
pub struct Player {
    #[script(get, set)]
    level: i64,
}

#[trait_export(path = "game::Damageable")]
pub trait Damageable {
    fn take_damage(&mut self, amount: i64);
    fn is_alive(&self) -> bool;
}

impl Damageable for Player {
    fn take_damage(&mut self, amount: i64) {
        self.level -= amount.max(0);
    }

    fn is_alive(&self) -> bool {
        self.level > 0
    }
}

#[export(path = "game::normalize")]
pub fn normalize(amount: i64) -> i64 {
    amount.max(0)
}

#[export(path = "game::grant_exp")]
pub fn grant_exp(player: &mut Player, amount: i64) -> VmResult<()> {
    player.level += amount.max(0);
    Ok(())
}

#[export(path = "game::roll", effects(random))]
pub fn roll(_ctx: &mut NativeCallContext<'_, '_>, player: &Player) -> i64 {
    player.level
}

#[methods(path = "game::Player")]
impl Player {
    pub fn current_level(&self) -> i64 {
        self.level
    }

    pub fn increment(&mut self, amount: i64) {
        self.level += amount;
    }

    fn rust_only_helper(&self) -> i64 {
        self.level
    }
}

#[test]
fn ordinary_exports_emit_normalized_callable_contracts() {
    let normalize = vela_callable_contract_normalize();
    assert_eq!(normalize.identity.kind, CallableKind::RustFunction);
    assert_eq!(normalize.effects, EffectSet::pure());
    assert_eq!(normalize.parameters[0].mode, BoundaryMode::Value);

    let grant = vela_callable_contract_grant_exp();
    assert_eq!(grant.effects, EffectSet::host_write());
    assert_eq!(grant.parameters[0].mode, BoundaryMode::ExclusiveHost);

    let roll = vela_callable_contract_roll();
    assert_eq!(
        roll.effects,
        EffectSet::host_read().union(EffectSet::random())
    );
    assert_eq!(roll.parameters[0].mode, BoundaryMode::HiddenContext);
}

#[test]
fn method_groups_share_receiver_classification() {
    let shared = Player::vela_callable_contract_current_level();
    let exclusive = Player::vela_callable_contract_increment();

    assert_eq!(shared.effects, EffectSet::host_read());
    assert_eq!(shared.parameters[0].mode, BoundaryMode::SharedHost);
    assert_eq!(exclusive.effects, EffectSet::host_write());
    assert_eq!(exclusive.parameters[0].mode, BoundaryMode::ExclusiveHost);

    let mut player = Player { level: 3 };
    assert_eq!(player.current_level(), 3);
    player.increment(2);
    assert_eq!(player.current_level(), 5);
    assert_eq!(player.rust_only_helper(), 5);
}

#[test]
fn trait_export_uses_explicit_vela_protocol_identity() {
    let protocol = vela_protocol_contract_Damageable();

    assert_eq!(protocol.identity.public_path, "game::Damageable");
    assert_eq!(protocol.methods.len(), 2);
    assert_eq!(protocol.methods[0].effects, EffectSet::host_write());
    assert_eq!(protocol.methods[1].effects, EffectSet::host_read());

    let mut player = Player { level: 5 };
    player.take_damage(2);
    assert!(player.is_alive());
}
