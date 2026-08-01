use super::*;
use crate::verification::VerificationErrorKind;
use crate::{
    CacheSiteKind, CallArgument, Register, UnlinkedCodeObject, UnlinkedInstruction,
    UnlinkedInstructionKind, UnlinkedProgram,
};
use vela_def::{
    DefPath, FieldId, FunctionId, MethodId, TypeId, script_inherent_method_id,
    script_trait_method_id, script_type_id,
};
use vela_host::target::HostPathPart;
use vela_package::{ModuleKey, ModulePath, PackageId};

fn semantic_diagnostic_codes(error: TestCompileError) -> Vec<String> {
    error
        .into_semantic_diagnostics()
        .into_iter()
        .filter_map(|diagnostic| diagnostic.code)
        .collect()
}

fn stable_test_trait_method_id(trait_name: &str, method_name: &str) -> MethodId {
    script_trait_method_id(PackageId::anonymous().as_str(), trait_name, method_name)
}

fn stable_test_inherent_method_id(type_name: &str, method_name: &str) -> MethodId {
    script_inherent_method_id(PackageId::anonymous().as_str(), type_name, method_name)
}

fn stable_test_type_id(type_name: &str) -> TypeId {
    script_type_id(PackageId::anonymous().as_str(), type_name, None)
}

fn anonymous_module(path: ModulePath) -> ModuleKey {
    ModuleKey::new(PackageId::anonymous(), path)
}

#[test]
fn compiler_entry_points_return_unlinked_bytecode() {
    let program: super::CompiledProgram = compile_test_program(
        SourceId::new(1),
        r#"
fn main() {
    return 42;
}
"#,
    )
    .expect("program should compile");
    assert!(program.function("main").is_some());

    let code: UnlinkedCodeObject = compile_test_function(
        SourceId::new(2),
        r#"
fn main() {
    return 7;
}
"#,
        "main",
    )
    .expect("function should compile");
    assert_eq!(code.name, "main");
}

#[test]
fn compiler_and_linker_preserve_function_asyncness() {
    let program = compile_test_program(
        SourceId::new(4),
        "async fn pending() { return 7; } fn ready() { return 8; }",
    )
    .expect("async metadata source should compile");

    assert_eq!(
        program
            .function("pending")
            .expect("pending bytecode")
            .asyncness,
        vela_common::CallableAsyncness::Async
    );
    assert_eq!(
        program.function("ready").expect("ready bytecode").asyncness,
        vela_common::CallableAsyncness::Sync
    );

    let artifact = crate::Linker::new()
        .link_compiled_program(program)
        .expect("async metadata program should link");
    let linked = artifact.program();
    let pending = linked
        .entry_point_by_name("pending")
        .and_then(|handle| linked.function(handle))
        .expect("linked pending entry");
    let ready = linked
        .entry_point_by_name("ready")
        .and_then(|handle| linked.function(handle))
        .expect("linked ready entry");
    assert_eq!(pending.asyncness, vela_common::CallableAsyncness::Async);
    assert_eq!(ready.asyncness, vela_common::CallableAsyncness::Sync);
}

#[test]
fn compiler_and_linker_emit_sealed_scoped_task_instruction() {
    let program = compile_test_program(
        SourceId::new(5),
        r#"
async fn repair(value: i64) -> i64 { return value + 1; }
fn main() {
    task::spawn_scoped(repair(41));
}
"#,
    )
    .expect("scoped task source should compile");
    let main = program.function("main").expect("main bytecode");
    assert!(matches!(
        &main.instructions[1].kind,
        UnlinkedInstructionKind::Task(task)
            if task.worker_name == "repair"
                && task.args.len() == 1
                && task.continuation.is_none()
    ));

    let artifact = crate::Linker::new()
        .link_compiled_program(program)
        .expect("scoped task program should link");
    assert!(
        artifact
            .required_features()
            .contains(crate::ArtifactFeatureSet::host_scoped_tasks())
    );
    let target = artifact.task_targets().first().expect("sealed task target");
    assert_eq!(target.worker_debug_name, "repair");
    assert_eq!(target.worker_signature.parameter_detachability.len(), 1);
    assert!(target.continuation.is_none());
}

#[test]
fn graph_requests_compile_program_and_stable_function_roots() {
    let built = vela_hir::source_ingestion::build_single_source(
        SourceId::new(3),
        "fn helper() { return 2; } fn main() { return helper(); }",
    )
    .expect("HIR source set");
    let function = built
        .function(&anonymous_module(ModulePath::root()), "main")
        .expect("stable main declaration");
    let options = CompilerOptions::default();
    let program = compile_program(ProgramCompilationRequest {
        sources: &built,
        options: &options,
        registry: None,
    })
    .expect("graph program request");
    let code = compile_function(FunctionCompilationRequest {
        function,
        options: &options,
        registry: None,
    })
    .expect("stable function request");

    assert!(program.function("main").is_some());
    assert_eq!(code.name, "main");
}

#[test]
fn compilation_requests_reject_invalid_scope_and_function_roots() {
    let options = CompilerOptions::default();
    let empty = vela_hir::source_ingestion::build_module_source_set(&[]).expect("empty source set");
    let error = compile_program(ProgramCompilationRequest {
        sources: &empty,
        options: &options,
        registry: None,
    })
    .expect_err("empty module graph must be rejected");
    assert_eq!(
        error.kind,
        CompileErrorKind::InvalidCompilationRequest(
            error::CompilationRequestError::EmptyModuleGraph
        )
    );

    let built = vela_hir::source_ingestion::build_single_source(
        SourceId::new(31),
        "const VALUE = 1; fn first() { return VALUE; }",
    )
    .expect("single source set");
    assert!(
        built
            .function(&anonymous_module(ModulePath::root()), "VALUE")
            .is_none()
    );
    assert!(
        built
            .function(&anonymous_module(ModulePath::root()), "missing")
            .is_none()
    );
}

#[test]
fn function_selection_is_bound_to_its_source_set_even_when_hir_ids_collide() {
    let alpha = vela_hir::source_ingestion::build_single_source(
        SourceId::new(33),
        "fn alpha() { return 1; }",
    )
    .expect("alpha source set");
    let beta = vela_hir::source_ingestion::build_single_source(
        SourceId::new(34),
        "fn beta() { return 2; }",
    )
    .expect("beta source set");
    let alpha = alpha
        .function(&anonymous_module(ModulePath::root()), "alpha")
        .expect("alpha function");
    let beta = beta
        .function(&anonymous_module(ModulePath::root()), "beta")
        .expect("beta function");
    assert_eq!(alpha.declaration(), beta.declaration());

    let code = compile_function(FunctionCompilationRequest {
        function: beta,
        options: &CompilerOptions::default(),
        registry: None,
    })
    .expect("bound beta function compiles");
    assert_eq!(code.name, "beta");
}

#[test]
fn one_module_graph_keeps_module_qualified_function_compilation() {
    let sources = [ModuleSource::new(
        SourceId::new(35),
        vela_package::PackageId::anonymous(),
        ModulePath::from_qualified("game::one"),
        "fn main() { return 1; }",
    )];
    let built =
        vela_hir::source_ingestion::build_module_source_set(&sources).expect("one-module graph");
    let function = built
        .function(
            &anonymous_module(ModulePath::from_qualified("game::one")),
            "main",
        )
        .expect("qualified main function");

    let code = compile_function(FunctionCompilationRequest {
        function,
        options: &CompilerOptions::default(),
        registry: None,
    })
    .expect("module function compiles");
    assert_eq!(code.name, "game::one::main");
}

#[test]
fn module_graph_request_keeps_roots_methods_and_metadata_in_one_scope() {
    let sources = [
        ModuleSource::new(
            SourceId::new(41),
            vela_package::PackageId::anonymous(),
            ModulePath::from_qualified("game::one"),
            "struct One {} impl One { fn value(self) { return 1; } } fn first() { return 1; }",
        ),
        ModuleSource::new(
            SourceId::new(42),
            vela_package::PackageId::anonymous(),
            ModulePath::from_qualified("game::two"),
            "struct Two {} impl Two { fn value(self) { return 2; } } fn second() { return 2; }",
        ),
    ];
    let built = vela_hir::source_ingestion::build_module_source_set(&sources).expect("source set");
    let options = CompilerOptions::default();
    let program = compile_program(ProgramCompilationRequest {
        sources: &built,
        options: &options,
        registry: None,
    })
    .expect("complete module scope compiles");

    assert!(program.function("game::one::first").is_some());
    assert!(program.function("game::two::second").is_some());
    assert!(
        program
            .script_method(stable_test_type_id("game::one::One"), "value")
            .is_some()
    );
    assert!(
        program
            .script_method(stable_test_type_id("game::two::Two"), "value")
            .is_some()
    );
    assert_eq!(
        program
            .script_metadata()
            .expect("retained graph")
            .module_ids()
            .count(),
        2
    );
}

#[test]
fn compiler_lowers_verified_mir_budget_points_to_instruction_metadata() {
    let program = compile_test_program(
        SourceId::new(3),
        "fn helper(value) { return value + 1; } fn main() { return helper(4); }",
    )
    .expect("budget schedule should compile");
    let main = program.function("main").expect("main function");
    let call = main
        .instructions
        .iter()
        .find(|instruction| {
            matches!(
                instruction.kind,
                UnlinkedInstructionKind::CallFunction { .. }
            )
        })
        .expect("MIR call point lowers to bytecode");

    assert_eq!(call.execution_units, 1);
    assert!(
        !program
            .function("helper")
            .expect("helper function")
            .instructions
            .iter()
            .any(|instruction| instruction.execution_units != 0)
    );
}

#[test]
fn linked_artifact_rejects_bytecode_that_drops_verified_mir_budget_points() {
    let mut program = compile_test_program(
        SourceId::new(4),
        "fn helper() { return 1; } fn main() { return helper(); }",
    )
    .expect("budget verification fixture compiles");
    program
        .bytecode
        .function_mut("main")
        .expect("main function")
        .instructions
        .iter_mut()
        .for_each(|instruction| instruction.execution_units = 0);

    let result = crate::Linker::new().link_compiled_program(program);
    assert!(
        matches!(
            result,
            Err(crate::linker::LinkError::MirBudgetLayoutMismatch { .. })
        ),
        "{result:?}"
    );
}

fn assert_moved_budget_charge_is_rejected(
    mut program: super::CompiledProgram,
    class: vela_mir::MirBudgetClass,
) {
    let code = program
        .bytecode
        .function_mut("main")
        .expect("budget fixture main");
    let source = code
        .instructions
        .iter()
        .position(|instruction| {
            instruction
                .mir_budget_charges
                .iter()
                .any(|charge| charge.class == class)
        })
        .expect("fixture carries requested budget class");
    let target = (source + 1..code.instructions.len())
        .find(|index| code.instructions[*index].mir_budget_charges.is_empty())
        .expect("fixture has an instruction after the charged boundary");
    let units = code.instructions[source].execution_units;
    let charges = std::mem::take(&mut code.instructions[source].mir_budget_charges);
    let origin = code.instructions[source].mir_origin.take();
    let span = code.instructions[source].span;
    code.instructions[source].execution_units = 0;
    code.instructions[target].execution_units = code.instructions[target]
        .execution_units
        .checked_add(units)
        .expect("test charge units fit");
    code.instructions[target].mir_budget_charges = charges;
    code.instructions[target].mir_origin = origin;
    code.instructions[target].span = span;

    let native_ids = program
        .bytecode
        .functions()
        .flat_map(|code| &code.instructions)
        .filter_map(|instruction| match instruction.kind {
            UnlinkedInstructionKind::CallNative { native, .. } => Some(native),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut linker = crate::Linker::new();
    for native in native_ids {
        linker.add_native_implementation(native);
    }
    let result = linker.link_compiled_program(program);
    assert!(
        matches!(
            result,
            Err(crate::LinkError::MirBudgetLayoutMismatch { .. })
        ),
        "{result:?}"
    );
}

#[test]
fn linked_artifact_rejects_equal_total_budget_charge_moves_across_boundaries() {
    assert_moved_budget_charge_is_rejected(
        compile_test_program(
            SourceId::new(44),
            "fn helper() { return 1; } fn main() { let value = helper(); return value + 1; }",
        )
        .expect("call fixture compiles"),
        vela_mir::MirBudgetClass::Call,
    );
    assert_moved_budget_charge_is_rejected(
        compile_test_program(
            SourceId::new(45),
            "fn main() { let values = [1, 2]; return values.len(); }",
        )
        .expect("allocation fixture compiles"),
        vela_mir::MirBudgetClass::Allocation,
    );
    assert_moved_budget_charge_is_rejected(
        compile_test_program(
            SourceId::new(46),
            "fn main(value) { let checked: i64 = value; return checked + 1; }",
        )
        .expect("guard fixture compiles"),
        vela_mir::MirBudgetClass::DynamicWork,
    );
    assert_moved_budget_charge_is_rejected(
        compile_test_program(
            SourceId::new(47),
            "fn main(value) { let field = reflect::get(value, \"name\"); return field; }",
        )
        .expect("reflection fixture compiles"),
        vela_mir::MirBudgetClass::Reflection,
    );

    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "BudgetPlayer",
            ))
            .host_runtime_id(910),
        )
        .expect("budget host type registers");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field("host", std::iter::empty::<&str>(), "BudgetPlayer", "level"),
                player,
            )
            .host_runtime_id(911),
        )
        .expect("budget host field registers");
    assert_moved_budget_charge_is_rejected(
        compile_test_program_with_registry(
            SourceId::new(48),
            "fn main(player: BudgetPlayer) { let level = player.level; return level + 1; }",
            registry.compile_view(),
        )
        .expect("HostAccess fixture compiles"),
        vela_mir::MirBudgetClass::HostAccess,
    );
}

#[test]
fn compiled_linking_keeps_semantically_distinct_equal_budget_generations_sealed() {
    let first = compile_test_program(SourceId::new(40), "fn main() { return 1; }")
        .expect("first generation compiles");
    let second = compile_test_program(SourceId::new(40), "fn main() { return 2; }")
        .expect("second generation compiles");
    let first_mir = std::sync::Arc::clone(first.verified_mir());
    let second_mir = std::sync::Arc::clone(second.verified_mir());
    assert_eq!(
        first.function_names().collect::<Vec<_>>(),
        second.function_names().collect::<Vec<_>>()
    );
    let first_units = first
        .function("main")
        .expect("first main")
        .instructions
        .iter()
        .map(|instruction| instruction.execution_units)
        .sum::<u32>();
    let second_units = second
        .function("main")
        .expect("second main")
        .instructions
        .iter()
        .map(|instruction| instruction.execution_units)
        .sum::<u32>();
    assert_eq!(first_units, second_units);

    let first = crate::Linker::new()
        .link_compiled_program(first)
        .expect("first generation links");
    let second = crate::Linker::new()
        .link_compiled_program(second)
        .expect("second generation links");
    assert!(std::sync::Arc::ptr_eq(first.verified_mir(), &first_mir));
    assert!(std::sync::Arc::ptr_eq(second.verified_mir(), &second_mir));
    assert!(!std::sync::Arc::ptr_eq(
        first.verified_mir(),
        second.verified_mir()
    ));
}

#[test]
fn compiled_linking_rejects_missing_added_and_reordered_executable_identities() {
    fn fixture() -> super::CompiledProgram {
        compile_test_program(
            SourceId::new(41),
            "fn alpha() { return || 1; } fn beta() { return 2; }",
        )
        .expect("identity fixture compiles")
    }

    let mut missing = fixture();
    missing.mir_executables = missing.mir_executables[..missing.mir_executables.len() - 1]
        .to_vec()
        .into_boxed_slice();
    assert!(matches!(
        crate::Linker::new().link_compiled_program(missing),
        Err(crate::LinkError::MirExecutableCountMismatch { .. })
    ));

    let mut added = fixture();
    let mut layouts = added.mir_executables.to_vec();
    layouts.push(layouts[0]);
    added.mir_executables = layouts.into_boxed_slice();
    assert!(matches!(
        crate::Linker::new().link_compiled_program(added),
        Err(crate::LinkError::MirExecutableCountMismatch { .. })
    ));

    let mut reordered = fixture();
    reordered.mir_executables.swap(0, 1);
    assert!(matches!(
        crate::Linker::new().link_compiled_program(reordered),
        Err(crate::LinkError::MirExecutableIdentityMismatch { .. })
    ));
}

#[test]
fn every_bound_handle_resolves_one_verified_mir_function() {
    let artifact = crate::Linker::new()
        .link_compiled_program(
            compile_test_program(
                SourceId::new(42),
                "fn main() { let outer = || { let inner = || 1; return inner(); }; return outer(); }",
            )
            .expect("nested identity fixture compiles"),
        )
        .expect("nested identity fixture links");
    assert_eq!(artifact.mir_executables().len(), artifact.function_count());
    for (handle, _) in artifact.functions() {
        let layout = artifact
            .mir_executable(handle)
            .expect("every linked handle has a MIR layout");
        let owner = artifact
            .verified_mir()
            .root(layout.root)
            .expect("layout root resolves");
        assert!(owner.program().function(layout.function).is_some());
    }
}

#[test]
fn debug_availability_is_initialized_and_lexically_bounded_across_nested_regions() {
    let program = compile_test_program(
        SourceId::new(43),
        r#"
fn main(value: i64 = 1) {
    let captured = value;
    let callback = |input: i64| {
        let nested = input + 1;
        return nested + captured;
    };
    for item in [1] {
        let loop_local = item + 1;
        callback(loop_local);
    }
    match value {
        1 => { let arm_local = 2; arm_local; },
        _ => { let other_arm_local = 3; other_arm_local; },
    }
    return callback(value);
}
"#,
    )
    .expect("lexical debug fixture compiles");

    let mut observed = std::collections::BTreeSet::new();
    for (_, owner) in program.verified_mir().roots() {
        for (function_id, function) in owner.program().functions() {
            let analyses = owner.analyses(function_id).expect("sealed analyses");
            for (debug_id, debug) in function.debug_locals() {
                observed.insert(debug.name.clone());
                for (statement_id, available) in &analyses.debug_availability.statement_before {
                    let scopes = function
                        .statement_lexical_scopes(*statement_id)
                        .expect("availability statement owns MIR scope facts");
                    assert!(
                        !available.contains(&debug_id) || scopes.contains(&debug.scope),
                        "{} remained visible outside MIR scope {:?}",
                        debug.name,
                        debug.scope,
                    );
                    assert!(
                        scopes.contains(&debug.scope) || !available.contains(&debug_id),
                        "{} leaked into a statement outside its MIR scope",
                        debug.name,
                    );
                }
                for (block, available) in &analyses.debug_availability.block_entry {
                    let scopes = function
                        .block_lexical_scopes(*block)
                        .expect("availability block owns MIR scope facts");
                    assert!(
                        scopes.contains(&debug.scope) || !available.contains(&debug_id),
                        "{} leaked into a block outside its MIR scope",
                        debug.name,
                    );
                }
            }
        }
    }
    for expected in [
        "value",
        "captured",
        "callback",
        "input",
        "nested",
        "item",
        "loop_local",
        "arm_local",
        "other_arm_local",
    ] {
        assert!(
            observed.contains(expected),
            "missing debug local {expected}"
        );
    }
}

#[test]
fn contextual_compound_assignment_keeps_dynamic_call_results_generic() {
    let program = compile_test_program(
        SourceId::new(5),
        r#"
fn main() {
    let total = 0;
    let add = |value| value + 1;
    total += add(2);
    return total;
}
"#,
    )
    .expect("dynamic closure result must not become an unproved typed MIR operand");
    let main = program.function("main").expect("main function");

    assert!(
        main.instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, UnlinkedInstructionKind::Add { .. }))
    );
}

#[test]
fn compiler_boundary_rejects_invalid_program_bytecode() {
    let mut code = UnlinkedCodeObject::new("main", 1);
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(2),
    }));
    let mut program = UnlinkedProgram::new();
    program.insert_function(code);

    let error = verify_program(program).expect_err("invalid bytecode should fail verification");
    let CompileErrorKind::BytecodeVerification(error) = error.kind else {
        panic!("expected bytecode verification error");
    };
    assert_eq!(error.function, "main");
    assert_eq!(error.instruction, Some(0));
    assert_eq!(
        error.kind,
        VerificationErrorKind::RegisterOutOfBounds {
            register: Register(2),
            register_count: 1,
        }
    );
}

#[test]
fn compiler_boundary_rejects_invalid_function_bytecode() {
    let mut code = UnlinkedCodeObject::new("main", 1);
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(2),
    }));

    let error = verify_code_object(code).expect_err("invalid bytecode should fail verification");
    let CompileErrorKind::BytecodeVerification(error) = error.kind else {
        panic!("expected bytecode verification error");
    };
    assert_eq!(error.function, "main");
    assert_eq!(error.instruction, Some(0));
}

#[test]
fn compiler_records_cache_site_metadata_for_cacheable_instructions() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty("host", std::iter::empty::<&str>(), "Player"))
                .host_runtime_id(77),
        )
        .expect("Player type should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field("host", std::iter::empty::<&str>(), "Player", "level"),
                player,
            )
            .host_runtime_id(1),
        )
        .expect("Player::level field should register");
    registry
        .register_function(
            vela_registry::FunctionDef::new(
                DefPath::function("host", std::iter::empty::<&str>(), "give_reward"),
                vela_registry::FunctionSignature::new(
                    [vela_registry::ParamDef::new("amount", Some("i64"))],
                    None::<vela_registry::TypeHintDef>,
                ),
            )
            .with_id(FunctionId::new(7)),
        )
        .expect("give_reward function should register");
    let program = compile_test_program_with_registry(
        SourceId::new(1),
        r#"
extern state bonus: Player;

struct Reward {
    gold: i64,
}

impl Reward {
    fn score(self, amount) {
        return self.gold + amount;
    }
}

fn main(player: Player) {
    let reward = Reward { gold: bonus.level };
    reward.gold = reward.gold + 1;
    let current = player.level;
    player.level = current + reward.gold;
    give_reward(reward.score(1));
    return player.level;
}
"#,
        registry.compile_view(),
    )
    .expect("program should compile");
    let main = program.function("main").expect("main should exist");
    let site_kinds = main
        .cache_sites
        .sites()
        .iter()
        .map(|site| site.kind)
        .collect::<Vec<_>>();

    assert!(site_kinds.contains(&CacheSiteKind::ExternStateRead));
    assert!(site_kinds.contains(&CacheSiteKind::NativeCall));
    assert!(site_kinds.contains(&CacheSiteKind::MethodCall));
    assert!(site_kinds.contains(&CacheSiteKind::RecordFieldRead));
    assert!(site_kinds.contains(&CacheSiteKind::RecordFieldWrite));
    assert!(site_kinds.contains(&CacheSiteKind::HostPathRead));
    assert!(site_kinds.contains(&CacheSiteKind::HostPathWrite));
    let record_write_site = main
        .cache_sites
        .sites()
        .iter()
        .find(|site| site.kind == CacheSiteKind::RecordFieldWrite)
        .expect("record field write cache site should exist");
    assert!(matches!(
        main.instructions
            .get(record_write_site.instruction_offset.0)
            .map(|instruction| &instruction.kind),
        Some(UnlinkedInstructionKind::SetRecordSlot { .. })
    ));
    let load_state_site = main
        .instructions
        .iter()
        .find_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::LoadExternState { cache_site, .. } => *cache_site,
            _ => None,
        })
        .expect("load state should carry cache site");
    assert_eq!(
        main.cache_sites
            .get(load_state_site)
            .expect("load state cache site should exist")
            .kind,
        CacheSiteKind::ExternStateRead
    );
    let native_call_site = main
        .instructions
        .iter()
        .find_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::CallNative { cache_site, .. } => *cache_site,
            _ => None,
        })
        .expect("native call should carry cache site");
    assert_eq!(
        main.cache_sites
            .get(native_call_site)
            .expect("native call cache site should exist")
            .kind,
        CacheSiteKind::NativeCall
    );
    for (index, site) in main.cache_sites.sites().iter().enumerate() {
        assert_eq!(site.id.index(), index);
        assert_eq!(site.function, "main");
        assert!(main.instructions.get(site.instruction_offset.0).is_some());
    }
}

#[test]
fn compiler_evaluates_schema_default_block_lets_from_hir_locals() {
    compile_test_program(
        SourceId::new(1),
        r#"
struct Reward {
    count: i64 = { let base = 1; base + 1 },
}

fn main() {
    let reward = Reward {};
    return reward.count;
}
"#,
    )
    .expect("schema default block let should compile as a constant");
}

#[test]
fn compiler_uses_field_owned_bodies_for_enum_schema_defaults() {
    compile_test_program(
        SourceId::new(1),
        r#"
enum Reward {
    Record { count: i64 = { let base = 1; base + 1 } },
    Tuple(count: i64 = { let base = 2; base + 1 }),
}

fn main() {
    let record = Reward::Record {};
    let tuple = Reward::Tuple();
    return record.count + tuple.0;
}
"#,
    )
    .expect("record and tuple schema defaults should resolve by their HIR body IDs");
}

#[test]
fn compiler_resolves_host_field_after_dynamic_index_from_hir_receiver() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "IndexedPlayer",
            ))
            .host_runtime_id(11),
        )
        .expect("IndexedPlayer type should register");
    let _inventory = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "IndexedInventory",
            ))
            .host_runtime_id(12),
        )
        .expect("IndexedInventory type should register");
    let item = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "IndexedItem",
            ))
            .host_runtime_id(13),
        )
        .expect("IndexedItem type should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field(
                    "host",
                    std::iter::empty::<&str>(),
                    "IndexedPlayer",
                    "inventory",
                ),
                player,
            )
            .type_hint(Some("IndexedInventory"))
            .host_runtime_id(20),
        )
        .expect("IndexedPlayer::inventory field should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field("host", std::iter::empty::<&str>(), "IndexedItem", "count"),
                item,
            )
            .type_hint(Some("i64"))
            .host_runtime_id(21),
        )
        .expect("IndexedItem::count field should register");
    let options = options::CompilerOptions::new().with_host_index_capability(
        "IndexedInventory",
        options::HostIndexCapabilityInfo {
            readable: true,
            key_type: Some("String".to_owned()),
            value_type: Some("IndexedItem".to_owned()),
            ..Default::default()
        },
    );

    let program = compile_test_program_with_options_and_registry(
        SourceId::new(1),
        r#"
fn item_count(player: IndexedPlayer, item_id: String) {
    return player.inventory[item_id].count;
}
"#,
        &options,
        registry.compile_view(),
    )
    .expect("host field after dynamic index should compile");
    let function = program
        .function("item_count")
        .expect("item_count should exist");
    assert!(
        function
            .cache_sites
            .sites()
            .iter()
            .any(|site| site.kind == CacheSiteKind::HostPathRead),
        "host field after dynamic index should lower to a host path read"
    );
    let target = function
        .instructions
        .iter()
        .find_map(|instruction| match instruction.kind {
            UnlinkedInstructionKind::HostRead { target, .. } => function.host_target(target),
            _ => None,
        })
        .expect("host path read should carry a target plan");
    assert_eq!(
        target.parts.as_slice(),
        &[
            HostPathPart::Field(FieldId::new(20)),
            HostPathPart::DynKey { arg: 0 },
            HostPathPart::Field(FieldId::new(21)),
        ]
    );
}

#[test]
fn compiler_checks_host_index_key_type_from_hir_operand() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "KeyedPlayer",
            ))
            .host_runtime_id(41),
        )
        .expect("KeyedPlayer type should register");
    let _inventory = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "KeyedInventory",
            ))
            .host_runtime_id(42),
        )
        .expect("KeyedInventory type should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field(
                    "host",
                    std::iter::empty::<&str>(),
                    "KeyedPlayer",
                    "inventory",
                ),
                player,
            )
            .type_hint(Some("KeyedInventory"))
            .host_runtime_id(44),
        )
        .expect("KeyedPlayer::inventory field should register");
    let options = options::CompilerOptions::new().with_host_index_capability(
        "KeyedInventory",
        options::HostIndexCapabilityInfo {
            readable: true,
            key_type: Some("i64".to_owned()),
            ..Default::default()
        },
    );

    let error = compile_test_program_with_options_and_registry(
        SourceId::new(1),
        r#"
fn item_count(player: KeyedPlayer) {
    return player.inventory["gold"];
}
"#,
        &options,
        registry.compile_view(),
    )
    .expect_err("wrong host index key type should fail");

    assert_eq!(
        semantic_diagnostic_codes(error),
        ["analysis::host_index_key_mismatch"]
    );
}

#[test]
fn compiler_checks_read_only_host_field_from_hir_receiver() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "ReadOnlyPlayer",
            ))
            .host_runtime_id(51),
        )
        .expect("ReadOnlyPlayer type should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field(
                    "host",
                    std::iter::empty::<&str>(),
                    "ReadOnlyPlayer",
                    "level",
                ),
                player,
            )
            .type_hint(Some("i64"))
            .writable(false)
            .host_runtime_id(52),
        )
        .expect("ReadOnlyPlayer::level field should register");

    let error = compile_test_program_with_registry(
        SourceId::new(1),
        r#"
fn bump(player: ReadOnlyPlayer) {
    player.level = 2;
    return 1;
}
"#,
        registry.compile_view(),
    )
    .expect_err("read-only host field assignment should fail");

    assert_eq!(
        semantic_diagnostic_codes(error),
        ["analysis::field_not_writable"]
    );
}

#[test]
fn compiler_resolves_param_default_field_receiver_from_hir() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "DefaultPlayer",
            ))
            .host_runtime_id(31),
        )
        .expect("DefaultPlayer type should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field("host", std::iter::empty::<&str>(), "DefaultPlayer", "level"),
                player,
            )
            .type_hint(Some("i64"))
            .host_runtime_id(32),
        )
        .expect("DefaultPlayer::level field should register");

    let program = compile_test_program_with_registry(
        SourceId::new(1),
        r#"
fn sample(player: DefaultPlayer, level = player.level) {
    return level;
}
"#,
        registry.compile_view(),
    )
    .expect("parameter-default host field should compile");
    let function = program.function("sample").expect("sample should exist");
    assert!(
        function
            .cache_sites
            .sites()
            .iter()
            .any(|site| site.kind == CacheSiteKind::HostPathRead),
        "parameter-default field access should lower through HostAccess"
    );
}

#[test]
fn compiler_resolves_method_call_receiver_from_hir() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
struct Counter {
    value: i64,
}

impl Counter {
    fn add(self, amount: i64) {
        return self.value + amount;
    }
}

fn sample(counter: Counter) {
    return counter.add(2);
}
"#,
    )
    .expect("script method call should compile");
    let function = program.function("sample").expect("sample should exist");
    assert!(
        function.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::CallMethodId { .. }
        )),
        "script method call should lower to a resolved method call"
    );
}

#[test]
fn compiler_resolves_record_field_read_receiver_from_hir() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
struct Counter {
    value: i64,
}

fn sample(counter: Counter) {
    return counter.value;
}
"#,
    )
    .expect("record field read should compile");
    let function = program.function("sample").expect("sample should exist");
    assert!(
        function.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::GetRecordSlot { .. }
        )),
        "typed record field read should lower to a resolved slot"
    );
}

mod block_tail_semantics;
mod call_diagnostics;
mod closures_and_bindings;
mod contract_delegation;
mod diagnostic_contracts;
mod diagnostics;
mod expressions;
mod literal_validation;
mod literals_and_calls;
mod loops_and_errors;
mod module_resolution;
mod phase0_frozen_contracts;
mod script_methods;
mod service_impl;
mod state;
mod type_contract_constructors;
mod value_method_shapes;
