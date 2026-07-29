use vela_macros::service_domain;

pub struct RequestContext;
pub struct RustRewardService;

#[service_domain(context = RequestContext)]
pub struct GameLogic {
    pub reward: RustRewardService,
}

fn main() {}
