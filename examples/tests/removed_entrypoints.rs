use vela_engine::engine::Engine;

#[test]
fn removed_service_and_provider_source_cannot_compile() {
    let engine = Engine::builder().build().expect("engine");
    for (name, source) in [
        (
            "service implementation",
            "#[service_impl(api::Handler)] impl Patch { fn run(value: i64) { value } }",
        ),
        (
            "provider implementation",
            "trait Action { fn run(self) -> i64; } struct Worker {} #[provider(id = \"worker\")] impl Action for Worker { fn run(self) -> i64 { 1 } }",
        ),
        ("base dispatch", "fn main() { service::base::run(1); }"),
        (
            "pinned dispatch",
            "fn main() { service::pinned::peer::run(1); }",
        ),
        (
            "imported pinned dispatch",
            "use service::pinned::peer::run as invoke; fn main() { invoke(1); }",
        ),
    ] {
        assert!(
            engine.compile_source(source).is_err(),
            "removed {name} must not compile"
        );
    }
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
