use vela_macros::service;

#[service(path = "game::reward")]
pub trait RewardService: Send + Sync {
    fn apply<T>(&self, value: T);
}

fn main() {}
