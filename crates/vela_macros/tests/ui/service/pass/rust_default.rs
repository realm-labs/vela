use vela_macros::{ScriptHost, service};

#[derive(ScriptHost)]
#[script(path = "game::Request")]
pub struct Request {
    pub values: Vec<i64>,
}

#[vela_macros::script_methods]
impl Request {}

#[service(path = "game::reward")]
pub trait RewardService: Send + Sync {
    fn apply(&self, amount: i64) -> Result<Vec<String>, String>;

    fn values<'borrow>(&self, request: &'borrow mut Request) -> &'borrow mut Vec<i64>;
}

fn main() {
    let engine = __vela_register_service_RewardService(
        vela_engine::engine::Engine::builder()
            .register_rust_type::<Request>(Request::vela_type_binding()),
    )
    .build()
    .unwrap();
    __vela_service_schema_RewardService(&engine.type_bindings()).unwrap();
}
