use vela_macros::{service, service_set};

#[service(path = "game::reward")]
pub trait RewardService: Send + Sync {
    fn apply(&self, amount: i64) -> i64;
}

pub struct RequestContext;

#[service_set(context = RequestContext)]
pub struct GameServices {
    pub reward: dyn RewardService,
}

fn main() {}
