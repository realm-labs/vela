use std::error::Error;

use vela_engine::engine::Engine;
use vela_engine::error::EngineResult;
use vela_engine::runtime::{CallOptions, Runtime};

pub use self::fixture::{GameHostFixture, GameHostOptions};

mod fixture;
mod ids;
mod schema;

pub fn build_engine(options: GameEngineOptions) -> EngineResult<Engine> {
    schema::build_gameplay_engine(options)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GameEngineOptions {
    pub schema: GameSchema,
    pub host_read: bool,
    pub host_write: bool,
    pub event_emit: bool,
    pub time: bool,
    pub random_function: bool,
    pub allow_random: bool,
    pub reflection: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GameSchema {
    pub context: bool,
    pub player: bool,
    pub monster: bool,
    pub inventory: bool,
    pub quest: bool,
    pub config: bool,
    pub reward: bool,
}

pub struct GameScript<'a> {
    label: &'a str,
    source: &'a str,
    engine: GameEngineOptions,
    host: GameHostOptions,
}

impl<'a> GameScript<'a> {
    pub fn new(label: &'a str, source: &'a str) -> Self {
        Self {
            label,
            source,
            engine: GameEngineOptions::default(),
            host: GameHostOptions::default(),
        }
    }

    pub fn context(mut self) -> Self {
        self.engine.schema.context = true;
        self
    }

    pub fn player(mut self) -> Self {
        self.engine.schema.player = true;
        self
    }

    pub fn monster(mut self) -> Self {
        self.engine.schema.monster = true;
        self
    }

    pub fn inventory(mut self) -> Self {
        self.engine.schema.inventory = true;
        self
    }

    pub fn quest(mut self) -> Self {
        self.engine.schema.quest = true;
        self
    }

    pub fn config(mut self) -> Self {
        self.engine.schema.context = true;
        self.engine.schema.config = true;
        self
    }

    pub fn reward(mut self) -> Self {
        self.engine.schema.reward = true;
        self
    }

    pub fn host_read(mut self) -> Self {
        self.engine.host_read = true;
        self
    }

    pub fn host_write(mut self) -> Self {
        self.engine.host_write = true;
        self
    }

    pub fn event_emit(mut self) -> Self {
        self.engine.event_emit = true;
        self
    }

    pub fn time(mut self) -> Self {
        self.engine.time = true;
        self
    }

    pub fn random(mut self) -> Self {
        self.engine.random_function = true;
        self.engine.allow_random = true;
        self
    }

    pub fn random_function(mut self) -> Self {
        self.engine.random_function = true;
        self
    }

    pub fn reflection(mut self) -> Self {
        self.engine.reflection = true;
        self
    }

    pub fn stale_player_arg(mut self) -> Self {
        self.host.stale_player_arg = true;
        self
    }

    pub fn deny_player_level_read(mut self) -> Self {
        self.host.deny_player_level_read = true;
        self
    }

    pub fn deny_player_level_write(mut self) -> Self {
        self.host.deny_player_level_write = true;
        self
    }

    pub fn deny_context_emit_call(mut self) -> Self {
        self.host.deny_context_emit_call = true;
        self
    }

    pub fn run(self) -> Result<(), Box<dyn Error>> {
        let engine = build_engine(self.engine).map_err(|error| format!("{error:?}"))?;
        let program = engine.compile_source(self.source).map_err(|error| {
            crate::diagnostics::render_engine_source_error(self.label, self.source, &error)
        })?;
        let main = program
            .function("main")
            .ok_or("script must define fn main(...)")?;
        let mut host = GameHostFixture::for_main(main, self.host);
        let args = host.main_args(main)?;

        let mut runtime = Runtime::new(engine, program);
        let output = runtime
            .call_with_adapter(
                "main",
                args,
                CallOptions::new(10_000, 1024 * 1024, 64),
                host.adapter_mut(),
            )
            .map_err(|error| {
                crate::diagnostics::render_vm_error(self.label, self.source, &error)
            })?;
        let output = runtime.value_to_owned(&output)?;
        host.print_result(output)
    }
}
