use vela_macros::service;

#[service(path = "game::reward")]
pub trait RewardService: Send + Sync {
    fn apply(&self, amount: i64) -> Result<Vec<String>, String>;
    fn count(&self, groups: &[Vec<String>]) -> i64;
}

pub struct RustRewardService;

impl RewardService for RustRewardService {
    fn apply(&self, amount: i64) -> Result<Vec<String>, String> {
        Ok(vec![format!("reward-{amount}")])
    }

    fn count(&self, groups: &[Vec<String>]) -> i64 {
        groups.iter().map(Vec::len).sum::<usize>() as i64
    }
}

#[test]
fn generated_service_contract_seals_against_its_registration_bundle() {
    let engine = __vela_register_service_RewardService(vela_engine::engine::Engine::builder())
        .build()
        .expect("generated service type closure should seal");
    let schema = __vela_service_schema_RewardService(&engine.type_bindings())
        .expect("generated schema should match the sealed registry");

    assert_eq!(schema.path(), "game::reward");
    assert_eq!(schema.methods().len(), 2);
    assert_eq!(schema.methods()[0].path, "game::reward::apply");
    assert_eq!(
        RustRewardService.apply(7).expect("ordinary Rust default"),
        vec!["reward-7"]
    );
    assert_eq!(
        RustRewardService.count(&[vec!["a".to_owned(), "b".to_owned()]]),
        2
    );
}
