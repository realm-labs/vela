use vela_macros::{ScriptHost, service};

#[derive(ScriptHost)]
#[vela(path = "game::Request")]
pub struct Request {
    pub values: Vec<i64>,
}

#[service(path = "game::reward")]
pub trait RewardService: Send + Sync {
    fn apply(&self, amount: i64) -> Result<Vec<String>, String>;

    fn request<'borrow>(&self, request: &'borrow mut Request) -> &'borrow mut Request;
}

fn main() {
    let engine = __vela_register_service_RewardService(
        vela_engine::engine::Engine::builder()
            .register_type::<Request>(),
    )
    .build()
    .unwrap();
    __vela_service_schema_RewardService(&engine.type_bindings()).unwrap();
}
