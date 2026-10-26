use rstest::rstest;
use vela_engine::engine::Engine;

#[rstest]
#[case::service_implementation(
    "#[service_impl(api::Handler)] impl Patch { fn run(value: i64) { value } }"
)]
#[case::provider_implementation(
    "trait Action { fn run(self) -> i64; } struct Worker {} #[provider(id = \"worker\")] impl Action for Worker { fn run(self) -> i64 { 1 } }"
)]
#[case::base_dispatch("fn main() { service::base::run(1); }")]
#[case::pinned_dispatch("fn main() { service::pinned::peer::run(1); }")]
#[case::imported_pinned_dispatch(
    "use service::pinned::peer::run as invoke; fn main() { invoke(1); }"
)]
fn removed_service_and_provider_source_cannot_compile(#[case] source: &str) {
    let engine = Engine::builder().build().expect("engine");
    assert!(
        engine.compile_source(source).is_err(),
        "removed source must not compile: {source}"
    );
}

#[test]
fn ordinary_trait_implementations_and_defaults_still_compile() {
    let engine = Engine::builder().build().expect("engine");
    engine
        .compile_source(
            "trait Action { fn run(self) -> i64 { 7 } } struct Worker {} impl Action for Worker {} fn main() -> i64 { Worker {}.run() }",
        )
        .expect("ordinary traits do not require Provider registration");
}
