use vela_macros::{ScriptHost, service};

#[derive(ScriptHost)]
#[script(path = "game::Request")]
pub struct Request {
    value: i64,
}

#[vela_macros::script_methods]
impl Request {}

#[service(path = "game::reward")]
pub trait RewardService: Send + Sync {
    async fn apply(&self, first: &mut Request, second: &mut Request, amount: i64) -> i64;
}

struct RustRewardService;

impl RewardService for RustRewardService {
    async fn apply(&self, first: &mut Request, second: &mut Request, amount: i64) -> i64 {
        first.value += 1;
        second.value += 1;
        amount + first.value + second.value
    }
}

fn assert_object_safe(service: &dyn __vela_service_dispatch_RewardService::Dispatch) {
    let mut first = Request { value: 1 };
    let mut second = Request { value: 2 };
    drop(service.apply(&mut first, &mut second, 3));
}

fn main() {
    assert_object_safe(&RustRewardService);
}
