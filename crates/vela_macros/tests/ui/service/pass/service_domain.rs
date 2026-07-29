use vela_engine::service::Service;
use vela_macros::{service, service_domain};

#[service(path = "game::reward")]
pub trait RewardService: Send + Sync {
    fn apply(&self, amount: i64) -> i64;
}

pub struct RustRewardService;

impl RewardService for RustRewardService {
    fn apply(&self, amount: i64) -> i64 {
        amount
    }
}

pub struct RequestContext;

#[service_domain(context = RequestContext)]
pub struct GameLogic {
    pub reward: Service<dyn RewardService>,
}

fn main() {
    let app = GameLogic::builder(vela_engine::engine::Engine::builder())
        .reward(RustRewardService)
        .build()
        .unwrap();
    app.domain().pin().reward().apply(1);
}
