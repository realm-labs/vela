use vela_macros::service;

#[service(path = "game::reward")]
pub trait RewardService: Send + Sync {
    fn apply(&mut self, amount: i64);
}

fn main() {}
