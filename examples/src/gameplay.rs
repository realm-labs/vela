use vela_engine::engine::Engine;
use vela_engine::error::EngineResult;

pub use self::fixture::{GameHostFixture, GameHostOptions};

mod fixture;
mod ids;
mod schema;

pub fn build_engine(options: GameEngineOptions) -> EngineResult<Engine> {
    schema::build_gameplay_engine(options)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GameEngineOptions {
    pub allow_random: bool,
}
