use vela_macros::service;

#[service(path = "game::reward")]
pub trait RewardService: Send + Sync {
    fn apply(&self, amount: i64) -> Result<Vec<String>, String>;
}

fn main() {
    let engine = __vela_register_service_RewardService(
        vela_engine::engine::Engine::builder(),
    )
    .build()
    .unwrap();
    __vela_service_schema_RewardService(&engine.type_bindings()).unwrap();
}
