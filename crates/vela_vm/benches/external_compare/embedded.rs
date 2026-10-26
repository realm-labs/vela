use std::error::Error;
use std::hint::black_box;
use std::time::Instant;

use mlua::{Function, Lua};
use rhai::{AST, Engine, Scope};

use super::config::BenchParams;
use super::preflight;
use super::support::{BenchResult, bytes_checksum, mix, summarize};
use super::workloads::Workload;

pub(crate) struct LuaRuntime;

impl LuaRuntime {
    pub(crate) fn preflight(&self, workload: &Workload) -> Result<(), Box<dyn Error>> {
        let lua = Lua::new();
        let hooks = std::cell::Cell::new(0_u64);
        lua.set_hook(
            mlua::HookTriggers::new().every_nth_instruction(1_000),
            move |_, _| {
                hooks.set(hooks.get() + 1);
                if hooks.get() > 1_000 {
                    Err(mlua::Error::RuntimeError(
                        "preflight instruction limit exceeded".into(),
                    ))
                } else {
                    Ok(mlua::VmState::Continue)
                }
            },
        )?;
        lua.load(workload.lua).exec()?;
        let run = lua.globals().get::<Function>("run")?;
        for iterations in preflight::ITERATIONS {
            let value: mlua::Value = run.call(iterations as i64)?;
            let mlua::Value::Integer(actual) = value else {
                return Err(format!(
                    "lua54 {} preflight expected Lua Integer, got {value:?}",
                    workload.name
                )
                .into());
            };
            preflight::validate("lua54", workload.name, iterations, actual)?;
        }
        Ok(())
    }

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
    pub(crate) fn preflight(&self, workload: &Workload) -> Result<(), Box<dyn Error>> {
        let mut runtime = Self::new();
        runtime.engine.set_max_operations(1_000_000);
        let ast = runtime.engine.compile(workload.rhai)?;
        for iterations in preflight::ITERATIONS {
            let actual = runtime.call_run(&ast, iterations)?;
            preflight::validate("rhai", workload.name, iterations, actual)?;
        }
        Ok(())
    }

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
