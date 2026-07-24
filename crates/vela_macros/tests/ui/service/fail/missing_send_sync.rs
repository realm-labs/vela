use vela_macros::service;

#[service(path = "game::reward")]
pub trait RewardService {
    fn apply(&self, amount: i64);
}

fn main() {}
