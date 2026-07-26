use vela_macros::service_set;

pub struct RequestContext;
pub struct RustRewardService;

#[service_set(context = RequestContext)]
pub struct GameServices {
    #[vela(default = RustRewardService)]
    pub reward: RustRewardService,
}

fn main() {}
