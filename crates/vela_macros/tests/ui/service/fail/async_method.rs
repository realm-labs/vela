use vela_macros::service;

#[service(path = "game::reward")]
pub trait RewardService: Send + Sync {
    async fn apply(&self, amount: i64) -> i64;
}

fn main() {}
