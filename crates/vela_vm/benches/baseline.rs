#![allow(clippy::result_large_err)]

use std::cell::RefCell;
use std::error::Error;
use std::hint::black_box;
use std::sync::Arc;
use std::time::{Duration, Instant};

use test_compile_support::{
    compile_test_function_with_registry, compile_test_program_with_registry,
};
use vela_bytecode::{LinkedArtifact, Linker, UnlinkedCodeObject, UnlinkedProgram};
use vela_common::SourceId;
use vela_def::{DefPath, FunctionId};

use vela_vm::Vm;
use vela_vm::budget::ExecutionBudget;
use vela_vm::heap::{GcBudget, GcConfig, HeapValue, ScriptHeap};
use vela_vm::heap_execution::HeapExecution;
use vela_vm::owned_value::OwnedValue;
use vela_vm::value::Value;
use vela_vm::{HostExecution, LinkedProgramHostBudgetCall, VmStateValues};

#[path = "baseline/cache_delta.rs"]
mod cache_delta;
#[path = "baseline/cache_support.rs"]
mod cache_support;
#[path = "baseline/config.rs"]
mod config;
#[path = "baseline/report.rs"]
mod report;
#[path = "../src/test_support.rs"]
#[allow(dead_code)]
mod test_compile_support;
#[path = "baseline/vm_state.rs"]
mod vm_state;
#[path = "baseline/workload_sources.rs"]
mod workload_sources;
#[path = "baseline/workloads.rs"]
mod workloads;

use cache_support::{BenchBytecodeProfiler, BenchCacheStats, BenchInlineCaches};
use config::{BenchConfig, BenchParams};
use vm_state::run_vm_state;
use workloads::{ExecutionMode, Workload, workloads};

const GC_SEEDED_GARBAGE_OBJECTS: usize = 128;
const GC_SAFE_POINT_SWEEP_SLOTS: usize = 16;

fn main() -> Result<(), Box<dyn Error>> {
    let config = BenchConfig::from_args();
    let params = config.params;
    report::print_header(&config);

    let mut ran = 0;
    let mut records = Vec::new();
    for workload in workloads() {
        if !config.should_run(workload.name) {
            continue;
        }
        let result = run_workload(workload, params)?;
        ran += 1;
        records.push(report::print_row(workload, &result));
    }
    if ran == 0 {
        return Err(format!("no baseline workloads matched {}", config.filters_label()).into());
    }
    cache_delta::print(&records);

    Ok(())
}

struct BenchResult {
    min_ns: u128,
    mean_ns: u128,
    median_ns: u128,
    p95_ns: u128,
    checksum: u64,
    cache_stats: BenchCacheStats,
    profile_hits: u64,
}

fn run_workload(workload: &Workload, params: BenchParams) -> Result<BenchResult, Box<dyn Error>> {
    let mut vm = Vm::new().with_standard_natives();
    register_bench_natives(&mut vm);
    let compiled = compile_workload(workload, &vm)?;

    for _ in 0..params.warmup {
        let value = run_once(&vm, &compiled)?;
        black_box(value);
    }
    compiled.reset_measurement_stats();

    let mut samples = Vec::with_capacity(params.repeats);
    let mut checksum = 0;
    for _ in 0..params.repeats {
        let started = Instant::now();
        for _ in 0..params.iterations {
            let value = run_once(&vm, &compiled)?;
            checksum = mix(checksum, value_checksum(&value));
            black_box(value);
        }
        samples.push(started.elapsed());
    }

    Ok(summarize(samples, checksum, &compiled))
}

fn register_bench_natives(vm: &mut Vm) {
    vm.register_borrowed_native("bench::mix4", |args, _heap, _budget| {
        let [Value::I64(a), Value::I64(b), Value::I64(c), Value::I64(d)] = args else {
            return Ok(OwnedValue::Unit);
        };
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(
            *a * 3 + *b * 2 - *c + *d,
        )))
    });
    vm.register_borrowed_native("bench::scores", |args, _heap, _budget| {
        if !args.is_empty() {
            return Ok(OwnedValue::Unit);
        }
        Ok(OwnedValue::iterator([2_i64, 3, 5, 8, 13]))
    });
}

enum CompiledWorkload {
    Function {
        mode: ExecutionMode,
        program: Arc<LinkedArtifact>,
    },
    CacheEnabledFunction {
        program: Arc<LinkedArtifact>,
        caches: Option<BenchInlineCaches>,
        profiler: BenchBytecodeProfiler,
    },
    ScriptProgram {
        program: Arc<LinkedArtifact>,
    },
    VmState {
        program: Arc<LinkedArtifact>,
        values: RefCell<VmStateValues>,
    },
}

fn run_once(vm: &Vm, workload: &CompiledWorkload) -> Result<OwnedValue, Box<dyn Error>> {
    match workload {
        CompiledWorkload::Function {
            mode: ExecutionMode::Inline,
            program,
        } => Ok(vm.run_linked_program(program, "main", &[])?),
        CompiledWorkload::Function {
            mode: ExecutionMode::Budgeted,
            program,
        } => {
            let mut budget = ExecutionBudget::new(1_000, usize::MAX, usize::MAX);
            Ok(vm.run_linked_program_with_budget(program, "main", &[], &mut budget)?)
        }
        CompiledWorkload::Function {
            mode: ExecutionMode::CacheEnabled,
            ..
        } => unreachable!("cache-enabled workloads compile to cache-enabled functions"),
        CompiledWorkload::Function {
            mode: ExecutionMode::ProfileOnly,
            ..
        } => unreachable!("profile-only workloads compile to instrumented functions"),
        CompiledWorkload::CacheEnabledFunction {
            program,
            caches,
            profiler,
        } => run_instrumented_function(vm, program, caches.as_ref(), profiler),
        CompiledWorkload::ScriptProgram { program } => {
            Ok(vm.run_linked_program(program, "main", &[])?)
        }
        CompiledWorkload::VmState { program, values } => {
            run_vm_state(vm, program, &mut values.borrow_mut())
        }
        CompiledWorkload::Function {
            mode: ExecutionMode::ManagedHeap,
            program,
        } => {
            let mut budget = ExecutionBudget::unbounded();
            Ok(vm.run_linked_program_with_budget(program, "main", &[], &mut budget)?)
        }
        CompiledWorkload::Function {
            mode: ExecutionMode::GcPacing,
            program,
        } => run_gc_pacing(vm, program),
        CompiledWorkload::Function { .. } => {
            unreachable!("non-function workloads compile to programs")
        }
    }
}

fn compile_workload(workload: &Workload, vm: &Vm) -> Result<CompiledWorkload, String> {
    match workload.mode {
        ExecutionMode::VmState => {
            let registry = bench_compile_registry()?;
            let program = compile_test_program_with_registry(
                SourceId::new(1),
                workload.source,
                registry.compile_view(),
            )
            .map_err(|error| format!("{error:?}"))?;
            let program = link_program_for_vm(vm, &program)?;
            let state = program
                .program()
                .states()
                .first()
                .ok_or_else(|| "VM-state benchmark has no state descriptor".to_owned())?
                .id;
            let mut values = VmStateValues::default();
            values.insert(state, Value::I64(0));
            Ok(CompiledWorkload::VmState {
                program,
                values: RefCell::new(values),
            })
        }
        ExecutionMode::ManagedHeap | ExecutionMode::GcPacing => {
            let registry = bench_compile_registry()?;
            let code = compile_test_function_with_registry(
                SourceId::new(1),
                workload.source,
                "main",
                registry.compile_view(),
            )
            .map_err(|error| format!("{error:?}"))?;
            Ok(CompiledWorkload::Function {
                mode: workload.mode,
                program: link_single_function_for_vm(vm, code)?,
            })
        }
        ExecutionMode::ScriptProgram
        | ExecutionMode::ScriptProgramProfileOnly
        | ExecutionMode::ScriptProgramCacheEnabled => {
            let registry = bench_compile_registry()?;
            let program = compile_test_program_with_registry(
                SourceId::new(1),
                workload.source,
                registry.compile_view(),
            )
            .map_err(|error| format!("{error:?}"))?;
            if matches!(workload.mode, ExecutionMode::ScriptProgramProfileOnly) {
                let linked = link_artifact_for_vm(vm, &program)?;
                return Ok(CompiledWorkload::CacheEnabledFunction {
                    caches: None,
                    profiler: BenchBytecodeProfiler::default(),
                    program: linked,
                });
            }
            if matches!(workload.mode, ExecutionMode::ScriptProgramCacheEnabled) {
                let artifact = link_artifact_for_vm(vm, &program)?;
                let cache_count = artifact.cache_layout().len();
                let linked = artifact;
                return Ok(CompiledWorkload::CacheEnabledFunction {
                    caches: Some(BenchInlineCaches::new(cache_count)),
                    profiler: BenchBytecodeProfiler::default(),
                    program: linked,
                });
            }
            Ok(CompiledWorkload::ScriptProgram {
                program: link_program_for_vm(vm, &program)?,
            })
        }
        ExecutionMode::Inline
        | ExecutionMode::Budgeted
        | ExecutionMode::ProfileOnly
        | ExecutionMode::CacheEnabled => {
            let registry = bench_compile_registry()?;
            let code = compile_test_function_with_registry(
                SourceId::new(1),
                workload.source,
                "main",
                registry.compile_view(),
            )
            .map_err(|error| format!("{error:?}"))?;
            if matches!(workload.mode, ExecutionMode::CacheEnabled) {
                let (program, cache_site_count) = link_single_function_image_for_vm(vm, code)?;
                return Ok(CompiledWorkload::CacheEnabledFunction {
                    caches: Some(BenchInlineCaches::new(cache_site_count)),
                    profiler: BenchBytecodeProfiler::default(),
                    program,
                });
            }
            if matches!(workload.mode, ExecutionMode::ProfileOnly) {
                let (program, _) = link_single_function_image_for_vm(vm, code)?;
                return Ok(CompiledWorkload::CacheEnabledFunction {
                    caches: None,
                    profiler: BenchBytecodeProfiler::default(),
                    program,
                });
            }
            let program = link_single_function_for_vm(vm, code)?;
            Ok(CompiledWorkload::Function {
                mode: workload.mode,
                program,
            })
        }
    }
}

impl CompiledWorkload {
    fn reset_measurement_stats(&self) {
        if let Self::CacheEnabledFunction {
            caches, profiler, ..
        } = self
        {
            if let Some(caches) = caches {
                caches.reset_measurement_counts();
            }
            profiler.reset();
        }
    }

    fn cache_stats(&self) -> BenchCacheStats {
        match self {
            Self::CacheEnabledFunction { caches, .. } => caches
                .as_ref()
                .map_or_else(BenchCacheStats::default, BenchInlineCaches::stats),
            _ => BenchCacheStats::default(),
        }
    }

    fn profile_hit_count(&self) -> u64 {
        match self {
            Self::CacheEnabledFunction { profiler, .. } => profiler.hit_count(),
            _ => 0,
        }
    }
}

fn run_instrumented_function(
    vm: &Vm,
    program: &Arc<LinkedArtifact>,
    caches: Option<&BenchInlineCaches>,
    profiler: &BenchBytecodeProfiler,
) -> Result<OwnedValue, Box<dyn Error>> {
    let mut host = HostExecution { state_values: None };
    let mut budget = ExecutionBudget::unbounded();
    Ok(
        vm.run_linked_program_host_budget_call(LinkedProgramHostBudgetCall {
            artifact: program,
            entry: "main",
            args: &[],
            host: &mut host,
            budget: &mut budget,
            inline_caches: caches.map(|caches| caches as &dyn vela_vm::VmInlineCaches),
            bytecode_profiler: Some(profiler),
        })?,
    )
}

fn bench_compile_registry() -> Result<vela_registry::DefinitionRegistry, String> {
    let mut registry = vela_stdlib::standard_registry().map_err(|error| format!("{error:?}"))?;
    registry
        .register_function(
            vela_registry::FunctionDef::new(
                DefPath::function("host", ["bench"], "mix4"),
                vela_registry::FunctionSignature::new(
                    [
                        vela_registry::ParamDef::new("a", Some("i64")),
                        vela_registry::ParamDef::new("b", Some("i64")),
                        vela_registry::ParamDef::new("c", Some("i64")),
                        vela_registry::ParamDef::new("d", Some("i64")),
                    ],
                    Some("i64".to_owned()),
                ),
            )
            .with_id(function_id_for_native_name("bench::mix4")),
        )
        .map_err(|error| format!("{error:?}"))?;
    registry
        .register_function(
            vela_registry::FunctionDef::new(
                DefPath::function("host", ["bench"], "scores"),
                vela_registry::FunctionSignature::new([], Some("iterator".to_owned())),
            )
            .with_id(function_id_for_native_name("bench::scores")),
        )
        .map_err(|error| format!("{error:?}"))?;
    Ok(registry)
}

fn function_id_for_native_name(name: &str) -> FunctionId {
    if let Some((module, function)) = name.rsplit_once("::")
        && let Some(id) = vela_stdlib::std_function_id(module, function)
    {
        return id;
    }
    let mut segments = name.split("::").collect::<Vec<_>>();
    let function = segments.pop().unwrap_or(name);
    FunctionId::from_def_id(DefPath::function("host", segments, function).id())
}

fn link_single_function_for_vm(
    vm: &Vm,
    code: UnlinkedCodeObject,
) -> Result<Arc<LinkedArtifact>, String> {
    let mut program = UnlinkedProgram::new();
    program.insert_function(code);
    link_program_for_vm(vm, &program)
}

fn link_single_function_image_for_vm(
    vm: &Vm,
    code: UnlinkedCodeObject,
) -> Result<(Arc<LinkedArtifact>, usize), String> {
    let mut program = UnlinkedProgram::new();
    program.insert_function(code);
    let artifact = link_artifact_for_vm(vm, &program)?;
    let cache_count = artifact.cache_layout().len();
    Ok((artifact, cache_count))
}

fn link_program_for_vm(vm: &Vm, program: &UnlinkedProgram) -> Result<Arc<LinkedArtifact>, String> {
    link_artifact_for_vm(vm, program)
}

fn link_artifact_for_vm(vm: &Vm, program: &UnlinkedProgram) -> Result<Arc<LinkedArtifact>, String> {
    let mut linker = Linker::new();
    for id in vm.native_implementation_ids() {
        linker.add_native_implementation(id);
    }
    linker
        .link_test_program(program)
        .map_err(|error| format!("{error:?}"))
}

fn run_gc_pacing(vm: &Vm, program: &Arc<LinkedArtifact>) -> Result<OwnedValue, Box<dyn Error>> {
    let mut heap = ScriptHeap::new();
    heap.set_gc_config(GcConfig {
        max_pause_micros: 50,
        heap_growth_factor: 1.0,
    });
    seed_gc_garbage(&mut heap);

    let mut budget = ExecutionBudget::unbounded();
    let (value, gc_checksum) = {
        let mut heap_execution = HeapExecution::new(&mut heap)
            .with_safe_point_gc_budget(GcBudget::sweep_slots(GC_SAFE_POINT_SWEEP_SLOTS));
        let value = vm.run_linked_program_with_heap_and_budget(
            program,
            "main",
            &[],
            &mut heap_execution,
            &mut budget,
        )?;
        let stats = heap_execution.last_gc_step().cloned();
        let gc_checksum = stats.map_or(0, |stats| {
            (stats.marked as u64)
                ^ ((stats.sweep_slots_visited as u64) << 8)
                ^ ((stats.swept as u64) << 24)
                ^ ((stats.bytes_freed as u64) << 32)
                ^ u64::from(stats.complete)
        });
        (value, gc_checksum)
    };

    Ok(OwnedValue::i64(
        runtime_value_checksum(&value) as i64
            + gc_checksum as i64
            + heap.live_object_count() as i64
            + heap.allocated_bytes() as i64,
    ))
}

fn seed_gc_garbage(heap: &mut ScriptHeap) {
    for index in 0..GC_SEEDED_GARBAGE_OBJECTS {
        let _ = heap.allocate(HeapValue::String(format!("garbage:{index:03}")));
    }
}

fn summarize(
    mut samples: Vec<Duration>,
    checksum: u64,
    workload: &CompiledWorkload,
) -> BenchResult {
    samples.sort_unstable();
    let min_ns = samples.first().map_or(0, Duration::as_nanos);
    let median_ns = percentile_ns(&samples, 50);
    let p95_ns = percentile_ns(&samples, 95);
    let mean_ns = if samples.is_empty() {
        0
    } else {
        samples.iter().map(Duration::as_nanos).sum::<u128>() / samples.len() as u128
    };
    BenchResult {
        min_ns,
        mean_ns,
        median_ns,
        p95_ns,
        checksum,
        cache_stats: workload.cache_stats(),
        profile_hits: workload.profile_hit_count(),
    }
}

fn percentile_ns(samples: &[Duration], percentile: usize) -> u128 {
    if samples.is_empty() {
        return 0;
    }
    let index = ((samples.len() - 1) * percentile).div_ceil(100);
    samples[index].as_nanos()
}

fn value_checksum(value: &OwnedValue) -> u64 {
    match value {
        OwnedValue::Unit => 0x02,
        OwnedValue::Bool(value) => u64::from(*value) ^ 0x03,
        OwnedValue::Char(value) => u64::from(*value as u32) ^ 0x04,
        OwnedValue::Scalar(value) => scalar_checksum(*value),
        OwnedValue::String(value) => bytes_checksum(value.as_bytes()),
        OwnedValue::Bytes(value) => bytes_checksum(value),
        OwnedValue::Tuple(values) | OwnedValue::Array(values) | OwnedValue::Set(values) => values
            .iter()
            .fold(0x05, |checksum, value| mix(checksum, value_checksum(value))),
        OwnedValue::Map(values) => values.iter().fold(0x06, |checksum, entry| {
            mix(
                mix(checksum, value_checksum(&entry.key)),
                value_checksum(&entry.value),
            )
        }),
        OwnedValue::Record { type_name, fields } => fields.values().fold(
            mix(0x07, bytes_checksum(type_name.as_bytes())),
            |checksum, value| mix(checksum, value_checksum(value)),
        ),
        OwnedValue::Enum {
            enum_name,
            variant,
            fields,
        } => fields.values().fold(
            mix(
                mix(0x08, bytes_checksum(enum_name.as_bytes())),
                bytes_checksum(variant.as_bytes()),
            ),
            |checksum, value| mix(checksum, value_checksum(value)),
        ),
        OwnedValue::Range(_) => 0x09,
        OwnedValue::Closure(_) => 0x0a,
        OwnedValue::Iterator(_) => 0x0b,
    }
}

fn runtime_value_checksum(value: &Value) -> u64 {
    if let Some(value) = value.as_scalar() {
        return scalar_checksum(value);
    }
    match value {
        Value::Missing => 0x01,
        Value::Unit => 0x02,
        Value::Bool(value) => u64::from(*value) ^ 0x03,
        Value::Char(value) => u64::from(*value as u32) ^ 0x04,
        Value::HeapRef(_) => 0x0a,
        _ => unreachable!("scalar values return before checksum match"),
    }
}

fn scalar_checksum(value: vela_common::ScalarValue) -> u64 {
    match value {
        vela_common::ScalarValue::I8(value) => value as i64 as u64,
        vela_common::ScalarValue::I16(value) => value as i64 as u64,
        vela_common::ScalarValue::I32(value) => value as i64 as u64,
        vela_common::ScalarValue::I64(value) => value as u64,
        vela_common::ScalarValue::U8(value) => u64::from(value),
        vela_common::ScalarValue::U16(value) => u64::from(value),
        vela_common::ScalarValue::U32(value) => u64::from(value),
        vela_common::ScalarValue::U64(value) => value,
        vela_common::ScalarValue::F32(value) => u64::from(value.to_bits()),
        vela_common::ScalarValue::F64(value) => value.to_bits(),
    }
}

fn bytes_checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |checksum, byte| {
        (checksum ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn mix(lhs: u64, rhs: u64) -> u64 {
    lhs.rotate_left(5) ^ rhs.wrapping_mul(0x9e37_79b9_7f4a_7c15)
}

fn profile() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}
