use vela_macros::{ScriptHost, export, export_module, methods, trait_export};
use vela_vm::error::VmResult;

#[derive(ScriptHost)]
#[script(path = "game::Player")]
pub struct Player {
    #[script(get, set)]
    level: i64,
}

#[export(path = "game::grant")]
pub fn grant(player: &mut Player, amount: i64) -> VmResult<()> {
    player.level += amount;
    Ok(())
}

#[export_module(path = "rules")]
mod rules {
    pub fn normalize(amount: i64) -> i64 {
        amount.max(0)
    }

    fn helper() {}
}

#[methods(path = "game::Player")]
impl Player {
    pub fn level(&self) -> i64 {
        self.level
    }
}

#[trait_export(path = "game::Damageable")]
pub trait Damageable {
    fn damage(&mut self, amount: i64);
}

fn main() {
    let _ = vela_callable_contract_grant();
    let _ = rules::vela_export_contracts();
    let _ = Player::vela_callable_contract_level();
    let _ = vela_protocol_contract_Damageable();
}
