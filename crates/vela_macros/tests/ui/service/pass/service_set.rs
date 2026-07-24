use vela_macros::{service, service_set};

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

#[service_set(context = RequestContext)]
pub struct GameServices {
    #[vela::default(RustRewardService)]
    pub reward: dyn RewardService,
}

fn main() {
    let engine = GameServices::register_types(
        vela_engine::engine::Engine::builder(),
    )
    .build()
    .unwrap();
    let services = GameServices::new(&engine.type_bindings()).unwrap();
    services.pin().reward().apply(1);
}
