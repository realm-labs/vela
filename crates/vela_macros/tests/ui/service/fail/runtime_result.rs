use vela_macros::service;

#[service(path = "game::reward")]
pub trait RewardService: Send + Sync {
    fn apply(&self, amount: i64) -> vela_vm::error::VmResult<i64>;
}

fn main() {}
