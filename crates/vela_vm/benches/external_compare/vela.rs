use std::error::Error;
use std::hint::black_box;
use std::time::Instant;

use vela_bytecode::compiler::compile_program_source_with_registry;
use vela_bytecode::{LinkedProgram, Linker, UnlinkedProgram};
use vela_common::SourceId;
use vela_vm::Vm;
use vela_vm::owned_value::OwnedValue;

use super::config::BenchParams;
use super::support::{BenchResult, bytes_checksum, mix, summarize, value_checksum};
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
        let registry = vela_stdlib::standard_registry()
            .map_err(|error| format!("standard registry failed: {error}"))?;
        let program = compile_program_source_with_registry(
            SourceId::new(1),
            workload.vela,
            registry.compile_view(),
        )
        .map_err(|error| format!("{error:?}"))?;
        let program = link_program_for_vm(&self.vm, &program)?;

        for _ in 0..params.warmup {
            let checksum = run_iterations(&self.vm, &program, params.iterations)?;
            black_box(checksum);
        }

        let mut samples = Vec::with_capacity(params.repeats);
        let mut checksum = bytes_checksum(b"vela") ^ bytes_checksum(workload.name.as_bytes());
        for _ in 0..params.repeats {
            let started = Instant::now();
            let iteration_checksum = run_iterations(&self.vm, &program, params.iterations)?;
            samples.push(started.elapsed());
            checksum = mix(checksum, iteration_checksum);
            black_box(iteration_checksum);
        }

        Ok(summarize(samples, checksum))
    }
}

fn run_iterations(
    vm: &Vm,
    program: &LinkedProgram,
    iterations: usize,
) -> Result<u64, Box<dyn Error>> {
    let value = vm.run_linked_program(
        program,
        "main",
        &[OwnedValue::Scalar(vela_common::ScalarValue::I64(
            iterations as i64,
        ))],
    )?;
    Ok(value_checksum(&value))
}

pub(crate) fn link_program_for_vm(
    vm: &Vm,
    program: &UnlinkedProgram,
) -> Result<LinkedProgram, Box<dyn Error>> {
    let mut linker = Linker::new();
    for id in vm.native_implementation_ids() {
        linker.add_native_implementation(id);
    }
    linker
        .link_program(program)
        .map_err(|error| format!("{error:?}").into())
}
