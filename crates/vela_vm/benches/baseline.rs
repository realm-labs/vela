#![allow(clippy::result_large_err)]

use std::error::Error;
use std::hint::black_box;
use std::time::{Duration, Instant};

use vela_bytecode::compiler::options::CompilerOptions;
use vela_bytecode::compiler::{
    compile_function_source, compile_program_source,
    compile_program_source_with_options_and_registry,
};
use vela_bytecode::{LinkedProgram, Linker, UnlinkedCodeObject, UnlinkedProgram};
use vela_common::{HostMethodId, HostObjectId, HostTypeId, SourceId};
use vela_def::{DefPath, FieldId, TypeId};
use vela_host::access::HostAccess;
use vela_host::mock::MockStateAdapter;
use vela_host::path::{HostPath, HostRef};
use vela_host::value::HostValue;
use vela_vm::HostExecution;
use vela_vm::Vm;
use vela_vm::budget::ExecutionBudget;
use vela_vm::heap::{GcBudget, GcConfig, HeapValue, ScriptHeap};
use vela_vm::heap_execution::HeapExecution;
use vela_vm::owned_value::OwnedValue;
use vela_vm::value::Value;

#[path = "baseline/workloads.rs"]
mod workloads;

use workloads::{ExecutionMode, WORKLOADS, Workload};

const QUICK_REPEATS: usize = 2;
const QUICK_ITERATIONS: usize = 8;
const QUICK_WARMUP: usize = 2;
const DEFAULT_REPEATS: usize = 7;
const DEFAULT_ITERATIONS: usize = 100;
const DEFAULT_WARMUP: usize = 10;
const PLAYER_TYPE: HostTypeId = HostTypeId::new(1);
const PLAYER_OBJECT: HostObjectId = HostObjectId::new(42);
const PLAYER_GENERATION: u32 = 1;
const LEVEL_FIELD: FieldId = FieldId::new(1);
const EXP_FIELD: FieldId = FieldId::new(2);
const INVENTORY_FIELD: FieldId = FieldId::new(3);
const GOLD_FIELD: FieldId = FieldId::new(4);
const ITEM_COUNT_FIELD: FieldId = FieldId::new(5);
const ITEMS_FIELD: FieldId = FieldId::new(6);
const ID_FIELD: FieldId = FieldId::new(7);
const QUEST_PROGRESS_FIELD: FieldId = FieldId::new(8);
const QUEST_COUNT_FIELD: FieldId = FieldId::new(9);
const QUEST_GOAL_FIELD: FieldId = FieldId::new(10);
const QUEST_DONE_FIELD: FieldId = FieldId::new(11);
const CONFIG_FIELD: FieldId = FieldId::new(12);
const EXP_TO_NEXT_LEVEL_FIELD: FieldId = FieldId::new(13);
const KILL_REWARDS_FIELD: FieldId = FieldId::new(14);
const REWARDS_FIELD: FieldId = FieldId::new(15);
const EMIT_METHOD: HostMethodId = HostMethodId::new(101);
const ADD_REWARD_METHOD: HostMethodId = HostMethodId::new(102);
const CTX_TYPE: HostTypeId = HostTypeId::new(2);
const MONSTER_TYPE: HostTypeId = HostTypeId::new(3);
const CTX_OBJECT: HostObjectId = HostObjectId::new(100);
const MONSTER_OBJECT: HostObjectId = HostObjectId::new(200);
const GC_SEEDED_GARBAGE_OBJECTS: usize = 128;
const GC_SAFE_POINT_SWEEP_SLOTS: usize = 16;

fn main() -> Result<(), Box<dyn Error>> {
    let params = BenchParams::from_args();
    println!(
        "vela_vm_baseline profile={} target={}/{} repeats={} iterations={} warmup={}",
        profile(),
        std::env::consts::OS,
        std::env::consts::ARCH,
        params.repeats,
        params.iterations,
        params.warmup
    );

    for workload in WORKLOADS {
        let result = run_workload(workload, params)?;
        println!(
            "bench={} mode={} min_ns={} mean_ns={} median_ns={} p95_ns={} checksum={}",
            workload.name,
            workload.mode.as_str(),
            result.min_ns,
            result.mean_ns,
            result.median_ns,
            result.p95_ns,
            result.checksum
        );
    }

    Ok(())
}

#[derive(Clone, Copy)]
struct BenchParams {
    repeats: usize,
    iterations: usize,
    warmup: usize,
}

impl BenchParams {
    fn from_args() -> Self {
        if std::env::args().skip(1).any(|arg| arg == "--quick") {
            return Self {
                repeats: QUICK_REPEATS,
                iterations: QUICK_ITERATIONS,
                warmup: QUICK_WARMUP,
            };
        }
        Self {
            repeats: DEFAULT_REPEATS,
            iterations: DEFAULT_ITERATIONS,
            warmup: DEFAULT_WARMUP,
        }
    }
}

struct BenchResult {
    min_ns: u128,
    mean_ns: u128,
    median_ns: u128,
    p95_ns: u128,
    checksum: u64,
}

fn run_workload(workload: &Workload, params: BenchParams) -> Result<BenchResult, Box<dyn Error>> {
    let mut vm = Vm::new().with_standard_natives();
    register_bench_natives(&mut vm);
    let compiled = compile_workload(workload, &vm)?;

    for _ in 0..params.warmup {
        let value = run_once(&vm, &compiled)?;
        black_box(value);
    }

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

    Ok(summarize(samples, checksum))
}

fn register_bench_natives(vm: &mut Vm) {
    vm.register_native("bench::mix4", |args| {
        let [
            OwnedValue::Scalar(vela_common::ScalarValue::I64(a)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(b)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(c)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(d)),
        ] = args
        else {
            return Ok(OwnedValue::Null);
        };
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(
            a * 3 + b * 2 - c + d,
        )))
    });
}

enum CompiledWorkload {
    Function {
        mode: ExecutionMode,
        program: Box<LinkedProgram>,
    },
    ScriptProgram {
        program: Box<LinkedProgram>,
    },
    HostAccess {
        program: Box<LinkedProgram>,
    },
    HostManagedHeapReadConversion {
        program: Box<LinkedProgram>,
    },
    HostManagedHeapHostAccess {
        program: Box<LinkedProgram>,
    },
    GameplayHost {
        program: Box<LinkedProgram>,
    },
}

fn run_once(vm: &Vm, workload: &CompiledWorkload) -> Result<OwnedValue, Box<dyn Error>> {
    match workload {
        CompiledWorkload::Function {
            mode: ExecutionMode::Inline,
            program,
        } => Ok(vm.run_linked_program(program, "main", &[])?),
        CompiledWorkload::ScriptProgram { program } => {
            Ok(vm.run_linked_program(program, "main", &[])?)
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
        CompiledWorkload::Function {
            mode: ExecutionMode::HostAccess,
            ..
        }
        | CompiledWorkload::Function {
            mode: ExecutionMode::HostManagedHeapHostAccess,
            ..
        }
        | CompiledWorkload::Function {
            mode: ExecutionMode::HostManagedHeapReadConversion,
            ..
        }
        | CompiledWorkload::Function {
            mode: ExecutionMode::GameplayHost,
            ..
        }
        | CompiledWorkload::Function {
            mode: ExecutionMode::ScriptProgram,
            ..
        } => unreachable!("host workloads compile to programs"),
        CompiledWorkload::HostAccess { program } => run_host_access(vm, program),
        CompiledWorkload::HostManagedHeapReadConversion { program } => {
            run_managed_heap_host_read_conversion(vm, program)
        }
        CompiledWorkload::HostManagedHeapHostAccess { program } => {
            run_managed_heap_host_conversion(vm, program)
        }
        CompiledWorkload::GameplayHost { program } => run_gameplay_monster_kill(vm, program),
    }
}

fn compile_workload(workload: &Workload, vm: &Vm) -> Result<CompiledWorkload, String> {
    match workload.mode {
        ExecutionMode::HostAccess
        | ExecutionMode::HostManagedHeapReadConversion
        | ExecutionMode::HostManagedHeapHostAccess => {
            let program = compile_program_source_with_options_and_registry(
                SourceId::new(1),
                workload.source,
                &host_access_compiler_options(),
                host_access_definition_registry().compile_view(),
            )
            .map_err(|error| format!("{error:?}"))?;
            let linked = Box::new(link_program_for_vm(vm, &program)?);
            Ok(match workload.mode {
                ExecutionMode::HostAccess => CompiledWorkload::HostAccess { program: linked },
                ExecutionMode::HostManagedHeapHostAccess => {
                    CompiledWorkload::HostManagedHeapHostAccess { program: linked }
                }
                ExecutionMode::HostManagedHeapReadConversion => {
                    CompiledWorkload::HostManagedHeapReadConversion { program: linked }
                }
                _ => unreachable!("only host patch modes are handled here"),
            })
        }
        ExecutionMode::GameplayHost => {
            let program = compile_program_source_with_options_and_registry(
                SourceId::new(1),
                workload.source,
                &host_access_compiler_options(),
                gameplay_definition_registry().compile_view(),
            )
            .map_err(|error| format!("{error:?}"))?;
            Ok(CompiledWorkload::GameplayHost {
                program: Box::new(link_program_for_vm(vm, &program)?),
            })
        }
        ExecutionMode::ManagedHeap | ExecutionMode::GcPacing => {
            let code = compile_function_source(SourceId::new(1), workload.source, "main")
                .map_err(|error| format!("{error:?}"))?;
            Ok(CompiledWorkload::Function {
                mode: workload.mode,
                program: Box::new(link_single_function_for_vm(vm, code)?),
            })
        }
        ExecutionMode::ScriptProgram => {
            let program = compile_program_source(SourceId::new(1), workload.source)
                .map_err(|error| format!("{error:?}"))?;
            Ok(CompiledWorkload::ScriptProgram {
                program: Box::new(link_program_for_vm(vm, &program)?),
            })
        }
        ExecutionMode::Inline => {
            let code = compile_function_source(SourceId::new(1), workload.source, "main")
                .map_err(|error| format!("{error:?}"))?;
            Ok(CompiledWorkload::Function {
                mode: workload.mode,
                program: Box::new(link_single_function_for_vm(vm, code)?),
            })
        }
    }
}

fn link_single_function_for_vm(vm: &Vm, code: UnlinkedCodeObject) -> Result<LinkedProgram, String> {
    let mut program = UnlinkedProgram::new();
    program.insert_function(code);
    link_program_for_vm(vm, &program)
}

fn link_program_for_vm(vm: &Vm, program: &UnlinkedProgram) -> Result<LinkedProgram, String> {
    let mut linker = Linker::new();
    for id in vm.native_implementation_ids() {
        linker.add_native_implementation(id);
    }
    linker
        .link_program(program)
        .map_err(|error| format!("{error:?}"))
}

fn host_access_compiler_options() -> CompilerOptions {
    CompilerOptions::new().with_host_index_capability(
        "Items",
        vela_bytecode::compiler::options::HostIndexCapabilityInfo {
            value_type: Some("Item".to_owned()),
            ..Default::default()
        },
    )
}

fn host_access_definition_registry() -> vela_registry::DefinitionRegistry {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = register_bench_host_type(&mut registry, "Player", PLAYER_TYPE);
    let inventory = register_bench_host_type(&mut registry, "Inventory", HostTypeId::new(4));
    let _items = register_bench_host_type(&mut registry, "Items", HostTypeId::new(5));
    let item = register_bench_host_type(&mut registry, "Item", HostTypeId::new(6));

    register_bench_host_field(&mut registry, player, "Player", "level", LEVEL_FIELD, None);
    register_bench_host_field(&mut registry, player, "Player", "exp", EXP_FIELD, None);
    register_bench_host_field(
        &mut registry,
        player,
        "Player",
        "inventory",
        INVENTORY_FIELD,
        Some("Inventory"),
    );
    register_bench_host_field(
        &mut registry,
        inventory,
        "Inventory",
        "gold",
        GOLD_FIELD,
        None,
    );
    register_bench_host_field(
        &mut registry,
        inventory,
        "Inventory",
        "items",
        ITEMS_FIELD,
        Some("Items"),
    );
    register_bench_host_field(
        &mut registry,
        inventory,
        "Inventory",
        "rewards",
        REWARDS_FIELD,
        None,
    );
    register_bench_host_field(&mut registry, item, "Item", "count", ITEM_COUNT_FIELD, None);
    register_bench_host_method(
        &mut registry,
        player,
        "Player",
        "add_reward",
        ADD_REWARD_METHOD,
        &["item_id", "count"],
    );

    registry
}

fn gameplay_definition_registry() -> vela_registry::DefinitionRegistry {
    let mut registry = host_access_definition_registry();
    let ctx = register_bench_host_type(&mut registry, "Context", CTX_TYPE);
    let monster = register_bench_host_type(&mut registry, "Monster", MONSTER_TYPE);
    let config = register_bench_host_type(&mut registry, "Config", HostTypeId::new(7));
    let quest_progress =
        register_bench_host_type(&mut registry, "QuestProgress", HostTypeId::new(8));

    let player = registry
        .compile_view()
        .resolve_type(&DefPath::ty("host", std::iter::empty::<&str>(), "Player"))
        .expect("Player bench type should exist");
    register_bench_host_field(&mut registry, player, "Player", "id", ID_FIELD, None);
    register_bench_host_field(
        &mut registry,
        player,
        "Player",
        "quest_progress",
        QUEST_PROGRESS_FIELD,
        Some("QuestProgress"),
    );
    register_bench_host_field(
        &mut registry,
        player,
        "Player",
        "quest_goal",
        QUEST_GOAL_FIELD,
        None,
    );
    register_bench_host_field(
        &mut registry,
        quest_progress,
        "QuestProgress",
        "quest_count",
        QUEST_COUNT_FIELD,
        None,
    );
    register_bench_host_field(
        &mut registry,
        quest_progress,
        "QuestProgress",
        "quest_done",
        QUEST_DONE_FIELD,
        None,
    );
    register_bench_host_field(
        &mut registry,
        ctx,
        "Context",
        "config",
        CONFIG_FIELD,
        Some("Config"),
    );
    register_bench_host_field(
        &mut registry,
        config,
        "Config",
        "exp_to_next_level",
        EXP_TO_NEXT_LEVEL_FIELD,
        None,
    );
    register_bench_host_field(
        &mut registry,
        config,
        "Config",
        "kill_rewards",
        KILL_REWARDS_FIELD,
        None,
    );
    register_bench_host_field(&mut registry, monster, "Monster", "exp", EXP_FIELD, None);
    register_bench_host_field(&mut registry, monster, "Monster", "id", ID_FIELD, None);
    register_bench_host_method(
        &mut registry,
        ctx,
        "Context",
        "emit",
        EMIT_METHOD,
        &["event", "a", "b"],
    );

    registry
}

fn register_bench_host_type(
    registry: &mut vela_registry::DefinitionRegistry,
    name: &str,
    host_type: HostTypeId,
) -> TypeId {
    registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty("host", std::iter::empty::<&str>(), name))
                .host_runtime_id(host_type.get().into()),
        )
        .expect("bench host type should register")
}

fn register_bench_host_field(
    registry: &mut vela_registry::DefinitionRegistry,
    owner: TypeId,
    owner_name: &str,
    name: &str,
    field: FieldId,
    type_hint: Option<&str>,
) {
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field("host", std::iter::empty::<&str>(), owner_name, name),
                owner,
            )
            .host_runtime_id(field.get())
            .writable(true)
            .type_hint(type_hint.map(str::to_owned)),
        )
        .expect("bench host field should register");
}

fn register_bench_host_method(
    registry: &mut vela_registry::DefinitionRegistry,
    owner: TypeId,
    owner_name: &str,
    name: &str,
    method: HostMethodId,
    params: &[&str],
) {
    registry
        .register_method(
            vela_registry::MethodDef::new(
                DefPath::method("host", std::iter::empty::<&str>(), owner_name, name),
                owner,
                vela_registry::FunctionSignature::new(
                    params
                        .iter()
                        .map(|param| vela_registry::ParamDef::new(*param, None::<String>)),
                    None,
                ),
            )
            .host_runtime_id(method.get()),
        )
        .expect("bench host method should register");
}

fn run_gc_pacing(vm: &Vm, program: &LinkedProgram) -> Result<OwnedValue, Box<dyn Error>> {
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

fn run_host_access(vm: &Vm, program: &LinkedProgram) -> Result<OwnedValue, Box<dyn Error>> {
    let player = HostRef::new(PLAYER_TYPE, PLAYER_OBJECT, PLAYER_GENERATION);
    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(
        HostPath::new(player).field(LEVEL_FIELD),
        HostValue::Scalar(vela_common::ScalarValue::I64(10)),
    );
    adapter.insert_diagnostic_path_value(
        HostPath::new(player).field(EXP_FIELD),
        HostValue::Scalar(vela_common::ScalarValue::I64(90)),
    );
    adapter.insert_diagnostic_path_value(
        HostPath::new(player)
            .field(INVENTORY_FIELD)
            .field(GOLD_FIELD),
        HostValue::Scalar(vela_common::ScalarValue::I64(5)),
    );
    adapter.insert_diagnostic_path_value(
        HostPath::new(player)
            .field(INVENTORY_FIELD)
            .field(ITEMS_FIELD)
            .key("gold")
            .field(ITEM_COUNT_FIELD),
        HostValue::Scalar(vela_common::ScalarValue::I64(7)),
    );
    adapter.insert_method_return(ADD_REWARD_METHOD, HostValue::Null);
    let mut tx = HostAccess::new();
    let mut budget = ExecutionBudget::unbounded();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };
    let value = vm.run_linked_program_with_host_budget_and_caches(
        program,
        "main",
        &[OwnedValue::HostRef(player)],
        &mut host,
        &mut budget,
        None,
    )?;
    Ok(OwnedValue::i64(value_checksum(&value) as i64))
}

fn run_managed_heap_host_conversion(
    vm: &Vm,
    program: &LinkedProgram,
) -> Result<OwnedValue, Box<dyn Error>> {
    let player = HostRef::new(PLAYER_TYPE, PLAYER_OBJECT, PLAYER_GENERATION);
    let level_path = HostPath::new(player).field(LEVEL_FIELD);
    let exp_path = HostPath::new(player).field(EXP_FIELD);
    let damage_path = HostPath::new(player)
        .field(INVENTORY_FIELD)
        .field(GOLD_FIELD);
    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(
        level_path.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(0)),
    );
    adapter.insert_diagnostic_path_value(
        exp_path.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(0)),
    );
    adapter.insert_diagnostic_path_value(
        damage_path.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(0)),
    );
    let mut tx = HostAccess::new();
    let mut budget = ExecutionBudget::unbounded();
    let value = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        vm.run_linked_program_with_host_budget_and_caches(
            program,
            "main",
            &[OwnedValue::HostRef(player)],
            &mut host,
            &mut budget,
            None,
        )?
    };
    Ok(OwnedValue::i64(
        value_checksum(&value) as i64
            + host_int(&adapter, level_path)?
            + host_int(&adapter, exp_path)?
            + host_int(&adapter, damage_path)?,
    ))
}

fn run_managed_heap_host_read_conversion(
    vm: &Vm,
    program: &LinkedProgram,
) -> Result<OwnedValue, Box<dyn Error>> {
    let player = HostRef::new(PLAYER_TYPE, PLAYER_OBJECT, PLAYER_GENERATION);
    let level_path = HostPath::new(player).field(LEVEL_FIELD);
    let exp_path = HostPath::new(player).field(EXP_FIELD);
    let damage_path = HostPath::new(player)
        .field(INVENTORY_FIELD)
        .field(GOLD_FIELD);
    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(
        level_path.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(3)),
    );
    adapter.insert_diagnostic_path_value(
        exp_path.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(5)),
    );
    adapter.insert_diagnostic_path_value(
        damage_path.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(7)),
    );
    let mut tx = HostAccess::new();
    let mut budget = ExecutionBudget::unbounded();
    let value = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        vm.run_linked_program_with_host_budget_and_caches(
            program,
            "main",
            &[OwnedValue::HostRef(player)],
            &mut host,
            &mut budget,
            None,
        )?
    };
    Ok(OwnedValue::i64(
        value_checksum(&value) as i64
            + host_int(&adapter, level_path)?
            + host_int(&adapter, exp_path)?
            + host_int(&adapter, damage_path)?,
    ))
}

fn run_gameplay_monster_kill(
    vm: &Vm,
    program: &LinkedProgram,
) -> Result<OwnedValue, Box<dyn Error>> {
    let player = HostRef::new(PLAYER_TYPE, PLAYER_OBJECT, PLAYER_GENERATION);
    let ctx = HostRef::new(CTX_TYPE, CTX_OBJECT, 1);
    let monster = HostRef::new(MONSTER_TYPE, MONSTER_OBJECT, 1);
    let inventory_gold_count_path = HostPath::new(player)
        .field(INVENTORY_FIELD)
        .field(ITEMS_FIELD)
        .key("gold")
        .field(ITEM_COUNT_FIELD);
    let quest_count_path = HostPath::new(player)
        .field(QUEST_PROGRESS_FIELD)
        .field(QUEST_COUNT_FIELD);
    let quest_done_path = HostPath::new(player)
        .field(QUEST_PROGRESS_FIELD)
        .field(QUEST_DONE_FIELD);

    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(
        HostPath::new(player).field(LEVEL_FIELD),
        HostValue::Scalar(vela_common::ScalarValue::I64(1)),
    );
    adapter.insert_diagnostic_path_value(
        HostPath::new(player).field(EXP_FIELD),
        HostValue::Scalar(vela_common::ScalarValue::I64(90)),
    );
    adapter.insert_diagnostic_path_value(
        HostPath::new(player).field(ID_FIELD),
        HostValue::Scalar(vela_common::ScalarValue::I64(7)),
    );
    adapter.insert_diagnostic_path_value(
        quest_count_path.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(2)),
    );
    adapter.insert_diagnostic_path_value(
        HostPath::new(player).field(QUEST_GOAL_FIELD),
        HostValue::Scalar(vela_common::ScalarValue::I64(3)),
    );
    adapter.insert_diagnostic_path_value(quest_done_path.clone(), HostValue::Bool(false));
    adapter.insert_diagnostic_path_value(
        inventory_gold_count_path.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(0)),
    );
    adapter.insert_diagnostic_path_value(
        HostPath::new(ctx)
            .field(CONFIG_FIELD)
            .field(EXP_TO_NEXT_LEVEL_FIELD),
        HostValue::Scalar(vela_common::ScalarValue::I64(100)),
    );
    adapter.insert_diagnostic_path_value(
        HostPath::new(monster).field(EXP_FIELD),
        HostValue::Scalar(vela_common::ScalarValue::I64(20)),
    );
    adapter.insert_diagnostic_path_value(
        HostPath::new(monster).field(ID_FIELD),
        HostValue::Scalar(vela_common::ScalarValue::I64(11)),
    );
    adapter.insert_method_return(EMIT_METHOD, HostValue::Null);
    adapter.insert_method_return(ADD_REWARD_METHOD, HostValue::Null);

    let mut tx = HostAccess::new();
    let mut budget = ExecutionBudget::unbounded();
    let value = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        vm.run_linked_program_with_host_budget_and_caches(
            program,
            "main",
            &[
                OwnedValue::HostRef(ctx),
                OwnedValue::HostRef(player),
                OwnedValue::HostRef(monster),
            ],
            &mut host,
            &mut budget,
            None,
        )?
    };
    Ok(OwnedValue::i64(
        value_checksum(&value) as i64
            + adapter.method_calls().len() as i64
            + host_int(&adapter, HostPath::new(player).field(LEVEL_FIELD))?
            + host_int(&adapter, HostPath::new(player).field(EXP_FIELD))?
            + host_int(&adapter, inventory_gold_count_path)?
            + host_int(&adapter, quest_count_path)?
            + i64::from(host_bool(&adapter, quest_done_path)?),
    ))
}

fn host_int(adapter: &MockStateAdapter, path: HostPath) -> Result<i64, Box<dyn Error>> {
    match adapter.read_diagnostic_path(&path)? {
        HostValue::Scalar(vela_common::ScalarValue::I64(value)) => Ok(value),
        value => Err(format!("expected int host value, got {value:?}").into()),
    }
}

fn host_bool(adapter: &MockStateAdapter, path: HostPath) -> Result<bool, Box<dyn Error>> {
    match adapter.read_diagnostic_path(&path)? {
        HostValue::Bool(value) => Ok(value),
        value => Err(format!("expected bool host value, got {value:?}").into()),
    }
}

fn summarize(mut samples: Vec<Duration>, checksum: u64) -> BenchResult {
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
        OwnedValue::Missing => 0x01,
        OwnedValue::Null => 0x02,
        OwnedValue::Bool(value) => u64::from(*value) ^ 0x03,
        OwnedValue::Scalar(value) => scalar_checksum(*value),
        OwnedValue::String(value) => bytes_checksum(value.as_bytes()),
        OwnedValue::Bytes(value) => bytes_checksum(value),
        OwnedValue::Array(values) | OwnedValue::Set(values) => values
            .iter()
            .fold(0x05, |checksum, value| mix(checksum, value_checksum(value))),
        OwnedValue::Map(values) => values.iter().fold(0x06, |checksum, (key, value)| {
            mix(
                mix(checksum, bytes_checksum(key.as_bytes())),
                value_checksum(value),
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
        OwnedValue::Closure(_) | OwnedValue::HostRef(_) | OwnedValue::PathProxy(_) => 0x0a,
        OwnedValue::Iterator(_) => 0x0b,
    }
}

fn runtime_value_checksum(value: &Value) -> u64 {
    match value {
        Value::Missing => 0x01,
        Value::Null => 0x02,
        Value::Bool(value) => u64::from(*value) ^ 0x03,
        Value::Scalar(value) => scalar_checksum(*value),
        Value::Range(_) => 0x09,
        Value::HeapRef(_) | Value::HostRef(_) => 0x0a,
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

impl ExecutionMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Inline => "inline",
            Self::ScriptProgram => "script_program",
            Self::ManagedHeap => "managed_heap",
            Self::HostAccess => "host_access",
            Self::HostManagedHeapReadConversion => "host_managed_heap_read_conversion",
            Self::HostManagedHeapHostAccess => "host_managed_heap_access",
            Self::GameplayHost => "gameplay_host",
            Self::GcPacing => "gc_pacing",
        }
    }
}
