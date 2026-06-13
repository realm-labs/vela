use std::error::Error;
use std::hint::black_box;
use std::time::Instant;

use mlua::{Function, Lua};
use rhai::{AST, Engine, Scope};

use super::config::BenchParams;
use super::support::{BenchResult, bytes_checksum, mix, summarize};
use super::workloads::Workload;

pub(crate) struct LuaRuntime;

impl LuaRuntime {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn run(
        &self,
        workload: &Workload,
        params: BenchParams,
    ) -> Result<BenchResult, Box<dyn Error>> {
        let lua = Lua::new();
        lua.load(workload.lua).exec()?;
        let run = lua.globals().get::<Function>("run")?;

        for _ in 0..params.warmup {
            let checksum: i64 = run.call(params.iterations as i64)?;
            black_box(checksum);
        }

        let mut samples = Vec::with_capacity(params.repeats);
        let mut checksum = bytes_checksum(b"lua54") ^ bytes_checksum(workload.name.as_bytes());
        for _ in 0..params.repeats {
            let started = Instant::now();
            let iteration_checksum: i64 = run.call(params.iterations as i64)?;
            samples.push(started.elapsed());
            checksum = mix(checksum, iteration_checksum as u64);
            black_box(iteration_checksum);
        }

        Ok(summarize(samples, checksum))
    }
}

pub(crate) struct RhaiRuntime {
    engine: Engine,
}

impl RhaiRuntime {
    pub(crate) fn new() -> Self {
        let mut engine = Engine::new();
        engine.set_max_call_levels(256);
        engine.set_max_expr_depths(256, 256);
        Self { engine }
    }

    pub(crate) fn run(
        &self,
        workload: &Workload,
        params: BenchParams,
    ) -> Result<BenchResult, Box<dyn Error>> {
        let ast = self.engine.compile(workload.rhai)?;

        for _ in 0..params.warmup {
            let checksum = self.call_run(&ast, params.iterations)?;
            black_box(checksum);
        }

        let mut samples = Vec::with_capacity(params.repeats);
        let mut checksum = bytes_checksum(b"rhai") ^ bytes_checksum(workload.name.as_bytes());
        for _ in 0..params.repeats {
            let started = Instant::now();
            let iteration_checksum = self.call_run(&ast, params.iterations)?;
            samples.push(started.elapsed());
            checksum = mix(checksum, iteration_checksum as u64);
            black_box(iteration_checksum);
        }

        Ok(summarize(samples, checksum))
    }

    fn call_run(&self, ast: &AST, iterations: usize) -> Result<i64, Box<dyn Error>> {
        let mut scope = Scope::new();
        Ok(self
            .engine
            .call_fn::<i64>(&mut scope, ast, "run", (iterations as i64,))?)
    }
}
