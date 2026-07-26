use std::error::Error;
use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;

use vela_bytecode::{LinkedArtifact, Linker, UnlinkedProgram};
use vela_common::SourceId;
use vela_vm::Vm;
use vela_vm::owned_value::OwnedValue;

use super::cache_support::BenchInlineCaches;
use super::config::BenchParams;
use super::support::{BenchResult, bytes_checksum, mix, summarize, value_checksum};
use super::test_compile_support::compile_test_program_with_registry;
use super::workloads::Workload;

pub(crate) struct VelaRuntime {
    vm: Vm,
}

impl VelaRuntime {
    pub(crate) fn new() -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            vm: Vm::new().with_standard_natives(),
        })
    }

    pub(crate) fn run(
        &self,
        workload: &Workload,
        params: BenchParams,
    ) -> Result<BenchResult, Box<dyn Error>> {
        self.run_with_caches(workload, params, false)
    }

    /// Runs the same workload with warmed M20 inline caches.
    ///
    /// This measures the "cache-enabled" performance tier — the configuration
    /// the production Engine runtime always executes with — against the same
    /// checksum contract as the interpreter-only `vela` row. Cache storage is
    /// the shared single-threaded bench implementation, so the row is an
    /// upper bound that excludes the engine's per-site synchronization.
    pub(crate) fn run_cache_enabled(
        &self,
        workload: &Workload,
        params: BenchParams,
    ) -> Result<BenchResult, Box<dyn Error>> {
        self.run_with_caches(workload, params, true)
    }

    fn run_with_caches(
        &self,
        workload: &Workload,
        params: BenchParams,
        cache_enabled: bool,
    ) -> Result<BenchResult, Box<dyn Error>> {
        let registry = vela_stdlib::standard_registry()
            .map_err(|error| format!("standard registry failed: {error}"))?;
        let program = compile_test_program_with_registry(
            SourceId::new(1),
            workload.vela,
            registry.compile_view(),
        )
        .map_err(|error| format!("{error:?}"))?;
        let program = link_program_for_vm(&self.vm, &program)?;
        let caches = cache_enabled.then(|| BenchInlineCaches::new(program.cache_layout().len()));
        let caches = caches.as_ref();

        for _ in 0..params.warmup {
            let checksum = run_iterations(&self.vm, &program, params.iterations, caches)?;
            black_box(checksum);
        }

        let mut samples = Vec::with_capacity(params.repeats);
        let mut checksum = bytes_checksum(b"vela") ^ bytes_checksum(workload.name.as_bytes());
        for _ in 0..params.repeats {
            let started = Instant::now();
            let iteration_checksum = run_iterations(&self.vm, &program, params.iterations, caches)?;
            samples.push(started.elapsed());
            checksum = mix(checksum, iteration_checksum);
            black_box(iteration_checksum);
        }

        Ok(summarize(samples, checksum))
    }
}

fn run_iterations(
    vm: &Vm,
    program: &Arc<LinkedArtifact>,
    iterations: usize,
    caches: Option<&BenchInlineCaches>,
) -> Result<u64, Box<dyn Error>> {
    let args = [OwnedValue::Scalar(vela_common::ScalarValue::I64(
        iterations as i64,
    ))];
    let value = match caches {
        None => vm.run_linked_program(program, "main", &args)?,
        Some(caches) => {
            let mut adapter = vela_host::mock::MockStateAdapter::default();
            let mut access = vela_host::access::HostAccess;
            let mut host = vela_vm::HostExecution {
                adapter: &mut adapter,
                access: &mut access,
                state_values: None,
            };
            let mut budget = vela_vm::budget::ExecutionBudget::unbounded();
            vm.run_linked_program_host_budget_call(vela_vm::LinkedProgramHostBudgetCall {
                artifact: program,
                entry: "main",
                args: &args,
                host: &mut host,
                budget: &mut budget,
                inline_caches: Some(caches as &dyn vela_vm::VmInlineCaches),
                bytecode_profiler: None,
            })?
        }
    };
    Ok(value_checksum(&value))
}

pub(crate) fn link_program_for_vm(
    vm: &Vm,
    program: &UnlinkedProgram,
) -> Result<Arc<LinkedArtifact>, Box<dyn Error>> {
    let mut linker = Linker::new();
    for id in vm.native_implementation_ids() {
        linker.add_native_implementation(id);
    }
    linker
        .link_test_program(program)
        .map_err(|error| format!("{error:?}").into())
}
