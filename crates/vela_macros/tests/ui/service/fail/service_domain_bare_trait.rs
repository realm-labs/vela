use vela_macros::{service, service_domain};

#[service(path = "game::reward")]
pub trait RewardService: Send + Sync {
    fn apply(&self, amount: i64) -> i64;
}

pub struct RequestContext;

#[service_domain(context = RequestContext)]
pub struct GameLogic {
    pub reward: dyn RewardService,
}

fn main() {}
