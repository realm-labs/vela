use super::*;
use crate::budget::ExecutionBudgetKind;
use crate::heap::{GcBudget, HeapValue, ScriptHeap};
use crate::test_support::{
    compile_test_function, compile_test_modules, compile_test_program,
    compile_test_program_with_registry,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use vela_bytecode::{
    CacheSiteKind, Constant, ConstantId, InstructionOffset, LinkedArtifact, LinkedProgram, Linker,
    UnlinkedInstruction,
};
use vela_common::{SourceId, Span};
use vela_def::{FieldId, FunctionId, MethodId, TypeId, VariantId};
use vela_hir::module_graph::{ModuleGraph, ModuleSource};
use vela_host::value::HostValue;
use vela_package::ModulePath;
use vela_reflect::access::{FieldAccess, FunctionAccess, MethodAccess, MethodEffectSet};
use vela_reflect::candidates::ReflectCandidate;
use vela_reflect::error::ReflectErrorKind;
use vela_reflect::modules::{FunctionDesc, ModuleDesc};
use vela_reflect::registry::{
    FieldDesc, MethodDesc, MethodParamDesc, TraitDesc, TraitMethodDesc, TypeDesc, TypeKey,
    TypeKind, VariantDesc,
};

#[cfg(target_pointer_width = "64")]
#[test]
fn value_runtime_slot_stays_compact() {
    assert!(
        std::mem::size_of::<Value>() <= 32,
        "Value runtime slot grew to {} bytes",
        std::mem::size_of::<Value>()
    );
}

#[test]
fn linked_vm_state_resolves_stable_id_across_generation_slot_changes() {
    let old_program = compile_test_program(
        SourceId::new(80),
        "state value: i64 = 0; fn read() { return value; }",
    )
    .expect("old state program should compile");
    let new_program = compile_test_program(
        SourceId::new(81),
        "state added: i64 = 0; state value: i64 = 0; fn read() { return value; }",
    )
    .expect("new state program should compile");
    let old = link_test_program(&old_program);
    let new = link_test_program(&new_program);
    let state = old
        .program()
        .states()
        .iter()
        .find(|state| state.qualified_name == "main::value")
        .expect("value state descriptor")
        .id;
    let old_slot = old
        .program()
        .states()
        .iter()
        .position(|descriptor| descriptor.id == state);
    let new_slot = new
        .program()
        .states()
        .iter()
        .position(|descriptor| descriptor.id == state);
    assert_eq!(old_slot, Some(0));
    assert_eq!(new_slot, Some(1));

    let mut values = VmStateValues::default();
    values.insert(state, Value::I64(41));

    let vm = Vm::new();
    for artifact in [&old, &new] {
        let mut host = HostExecution {
            state_values: Some(&mut values),
        };
        let mut budget = ExecutionBudget::unbounded();
        assert_eq!(
            vm.run_linked_program_with_host_budget_and_caches(
                artifact,
                "read",
                &[],
                &mut host,
                &mut budget,
                None,
            ),
            Ok(OwnedValue::i64(41))
        );
    }
}

mod block_tail_semantics;
mod bytes;
mod char;
mod consts;
mod control_flow;
mod deferred_literals;
mod dynamic_method_dispatch;
mod evaluation_order;
mod execution_core;
mod iteration;
mod linked_array_standard_method_cache;
mod linked_bytecode_profiler;
mod linked_context_execution;
mod linked_execution_core;
mod linked_map_standard_method_cache;
mod linked_readonly_standard_method_cache;
mod linked_set_standard_method_cache;
mod linked_standard_method_cache;
mod linked_standard_method_cache_fixtures;
mod linked_standard_method_cache_support;
mod linked_string_standard_method_cache;
mod modules;
mod numeric_conversions;
mod option_result_standard_method_cache;
mod owned_boundary;
mod program_execution;
mod records_enums;
mod reflection_members;
mod reflection_metadata;
mod reflection_modules;
mod reflection_permissions;
mod reflection_values;
mod runtime_semantics;
mod scalar_blocks;
mod scalar_numeric_ops;
mod script_methods;
mod standard_array_id_dispatch;
mod standard_borrowed_native_dispatch;
mod standard_callback_id_dispatch;
mod standard_id_dispatch;
mod standard_map_set_id_dispatch;
mod standard_option_result_id_dispatch;
mod standard_string_id_dispatch;
mod type_guards;

fn link_test_program(program: &UnlinkedProgram) -> Arc<LinkedArtifact> {
    Linker::new()
        .link_test_program(program)
        .expect("test program should link")
}

fn linked_test_owner(program: LinkedProgram) -> Arc<LinkedArtifact> {
    vela_bytecode::test_support::linked_artifact(program)
}

fn linked_dynamic_method_site(program: &LinkedProgram, entry: &str) -> CacheSiteId {
    linked_entry_code(program, entry)
        .cache_sites
        .sites()
        .iter()
        .find(|site| site.kind == CacheSiteKind::MethodCall)
        .map(|site| site.id)
        .expect("dynamic method fixture should have a method cache site")
}

fn linked_cache_len(program: &LinkedProgram) -> usize {
    program
        .functions()
        .flat_map(|(_, code)| code.cache_sites.sites())
        .map(|site| site.id.index() + 1)
        .max()
        .unwrap_or(0)
}

fn linked_entry_code<'program>(
    program: &'program LinkedProgram,
    entry: &str,
) -> &'program vela_bytecode::LinkedCodeObject {
    program
        .functions()
        .find(|(_, code)| program.debug_name(code.debug_name) == entry)
        .map(|(_, code)| code)
        .expect("linked fixture should have entry")
}

fn run_linked_test_code(code: UnlinkedCodeObject) -> VmResult<OwnedValue> {
    run_linked_test_code_with_linker(&Vm::new(), code, Linker::new())
}

fn run_linked_test_code_with_budget(
    code: UnlinkedCodeObject,
    budget: &mut ExecutionBudget,
) -> VmResult<OwnedValue> {
    let entry = code.name.clone();
    let mut program = UnlinkedProgram::new();
    program.insert_function(code);
    let linked = link_test_program(&program);
    Vm::new().run_linked_program_with_budget(&linked, &entry, &[], budget)
}

fn run_linked_test_program_with_budget(
    vm: &Vm,
    program: &UnlinkedProgram,
    entry: &str,
    args: &[OwnedValue],
    budget: &mut ExecutionBudget,
) -> VmResult<OwnedValue> {
    let mut linker = Linker::new();
    vm.native_implementation_ids()
        .for_each(|id| linker.add_native_implementation(id));
    let linked = linker
        .link_test_program(program)
        .expect("test program should link");
    vm.run_linked_program_with_budget(&linked, entry, args, budget)
}

fn run_linked_test_entry_with_caches(
    vm: &Vm,
    linked: &Arc<LinkedArtifact>,
    entry: &str,
    args: &[OwnedValue],
    budget: &mut ExecutionBudget,
    inline_caches: &dyn VmInlineCaches,
) -> VmResult<OwnedValue> {
    let function = linked_program_entry(linked.program(), entry)?;
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let args = owned_args_to_runtime(args, linked.program(), &mut heap_execution, Some(budget))?;
    let result = vm.execute_linked_call(
        crate::linked_execution::LinkedExecutionCall {
            owner: Arc::clone(linked),
            function,
            captures: &[],
            args: &args,
            check_param_guards: true,
            call_site: None,
            call_site_offset: None,
            inline_caches: Some(inline_caches),
            bytecode_profiler: None,
        },
        None,
        Some(&mut heap_execution),
        Some(budget),
    );
    owned_heap_result(result, &mut heap_execution, budget)
}

fn run_linked_test_program_with_host_budget(
    vm: &Vm,
    program: &UnlinkedProgram,
    entry: &str,
    args: &[OwnedValue],
    host: &mut HostExecution<'_>,
    budget: &mut ExecutionBudget,
) -> VmResult<OwnedValue> {
    let mut linker = Linker::new();
    vm.native_implementation_ids()
        .for_each(|id| linker.add_native_implementation(id));
    let linked = linker
        .link_test_program(program)
        .expect("test program should link");
    vm.run_linked_program_with_host_budget_and_caches(&linked, entry, args, host, budget, None)
}

fn run_linked_test_program_runtime_with_heap_and_budget(
    vm: &Vm,
    program: &UnlinkedProgram,
    entry: &str,
    args: &[Value],
    heap: &mut HeapExecution<'_>,
    budget: &mut ExecutionBudget,
) -> VmResult<Value> {
    let mut linker = Linker::new();
    vm.native_implementation_ids()
        .for_each(|id| linker.add_native_implementation(id));
    let linked = linker
        .link_test_program(program)
        .expect("test program should link");
    let function = linked
        .functions()
        .find_map(|(handle, code)| (linked.debug_name(code.debug_name) == entry).then_some(handle))
        .ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownFunction {
                name: entry.to_owned(),
            })
        })?;
    vm.execute_linked_call(
        crate::linked_execution::LinkedExecutionCall {
            owner: Arc::clone(&linked),
            function,
            captures: &[],
            args,
            check_param_guards: true,
            call_site: None,
            call_site_offset: None,
            inline_caches: None,
            bytecode_profiler: None,
        },
        None,
        Some(heap),
        Some(budget),
    )
}

fn run_linked_test_code_with_linker(
    vm: &Vm,
    code: UnlinkedCodeObject,
    linker: Linker<'_>,
) -> VmResult<OwnedValue> {
    let entry = code.name.clone();
    let mut program = UnlinkedProgram::new();
    program.insert_function(code);
    let linked = linker
        .link_test_program(&program)
        .expect("test code should link");
    vm.run_linked_program(&linked, &entry, &[])
}

fn reflection_registry() -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    registry.register_trait(
        TraitDesc::new("Damageable")
            .method(TraitMethodDesc::new(MethodId::new(1), "damage").defaulted(true)),
    );
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .kind(TypeKind::ScriptStruct)
            .docs("A player script value.")
            .attr("domain", "gameplay")
            .field(FieldDesc::new(FieldId::new(1), "id"))
            .field(
                FieldDesc::new(level_field(), "level")
                    .writable(true)
                    .type_hint("i64")
                    .docs("Current player level.")
                    .attr("unit", "level"),
            )
            .method(
                MethodDesc::new(MethodId::new(5), "grant_exp")
                    .effects(MethodEffectSet::pure())
                    .param(MethodParamDesc::new("amount").type_hint("i64"))
                    .return_type("bool")
                    .docs("Grant experience.")
                    .attr("effect", "write"),
            )
            .trait_impl(TraitDesc::new("Damageable")),
    );
    registry
}

fn script_reflection_registry() -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(200), "Player"))
            .kind(TypeKind::ScriptStruct)
            .field(FieldDesc::new(FieldId::new(20), "level").writable(true))
            .trait_impl(TraitDesc::new("Damageable")),
    );
    registry
}

fn script_module_reflection_registry() -> TypeRegistry {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(1),
        vela_package::PackageId::anonymous(),
        ModulePath::from_qualified("game::reward"),
        r#"
#[doc("Grant reward.")]
#[event("reward")]
pub fn grant(player: Player, amount: i64 = 1) -> bool {
    return true;
}

pub state counter: i64 = 1;

"#,
    ));
    let mut registry = TypeRegistry::new();
    registry.register_script_modules(&graph);
    registry
}

fn policy_module_reflection_registry() -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    registry.register_module(ModuleDesc::new("game::reward"));
    registry.register_function(
        FunctionDesc::new(FunctionId::new(1), "game::reward::grant").module("game::reward"),
    );
    registry.register_function(
        FunctionDesc::new(FunctionId::new(2), "game::reward::hidden")
            .module("game::reward")
            .access(FunctionAccess::new().reflect_visible(false)),
    );
    registry.register_function(
        FunctionDesc::new(FunctionId::new(3), "game::reward::private")
            .module("game::reward")
            .access(FunctionAccess::new().public(false).reflect_visible(true)),
    );
    registry.register_function(
        FunctionDesc::new(FunctionId::new(4), "game::reward::admin")
            .module("game::reward")
            .access(FunctionAccess::new().require_permission("game::admin")),
    );
    registry
}

fn policy_method_reflection_registry() -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(500), "Player"))
            .kind(TypeKind::ScriptStruct)
            .method(MethodDesc::new(MethodId::new(1), "visible"))
            .method(
                MethodDesc::new(MethodId::new(2), "hidden")
                    .access(MethodAccess::new().reflect_callable(false)),
            )
            .method(
                MethodDesc::new(MethodId::new(3), "private")
                    .access(MethodAccess::new().public(false).reflect_callable(true)),
            )
            .method(
                MethodDesc::new(MethodId::new(4), "admin")
                    .access(MethodAccess::new().require_permission("player.admin")),
            ),
    );
    registry
}

fn policy_field_reflection_registry() -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(600), "Player"))
            .kind(TypeKind::ScriptStruct)
            .field(FieldDesc::new(FieldId::new(1), "level"))
            .field(
                FieldDesc::new(FieldId::new(2), "secret")
                    .access(FieldAccess::new().reflect_readable(false)),
            ),
    );
    registry
}

fn member_reflection_registry() -> TypeRegistry {
    let mut registry = reflection_registry();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(300), "QuestProgress"))
            .kind(TypeKind::ScriptEnum)
            .variant(
                VariantDesc::new(VariantId::new(10), "Active")
                    .field(FieldDesc::new(FieldId::new(11), "count"))
                    .field(
                        FieldDesc::new(FieldId::new(13), "secret")
                            .access(FieldAccess::new().reflect_readable(false)),
                    ),
            )
            .variant(VariantDesc::new(VariantId::new(12), "Finished")),
    );
    registry
}

fn policy_variant_field_reflection_registry() -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(310), "QuestProgress"))
            .kind(TypeKind::ScriptEnum)
            .variant(
                VariantDesc::new(VariantId::new(10), "Active")
                    .field(FieldDesc::new(FieldId::new(11), "count"))
                    .field(
                        FieldDesc::new(FieldId::new(14), "admin_note")
                            .access(FieldAccess::new().require_permission("quest.admin.inspect")),
                    ),
            ),
    );
    registry
}

fn level_field() -> FieldId {
    FieldId::new(2)
}

fn run_standard_source(source: &str) -> VmResult<OwnedValue> {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let program =
        compile_test_program_with_registry(SourceId::new(1), source, registry.compile_view())
            .expect("standard source should compile");
    let mut linker = Linker::with_registry(&registry);
    let vm = Vm::new().with_standard_natives();
    vm.native_implementation_ids()
        .for_each(|id| linker.add_native_implementation(id));
    let linked = linker
        .link_test_program(&program)
        .expect("standard source should link");
    vm.run_linked_program(&linked, "main", &[])
}

fn compile_standard_program_source(
    source: SourceId,
    text: &str,
) -> vela_bytecode::compiler::error::CompileResult<vela_bytecode::compiler::CompiledProgram> {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    compile_test_program_with_registry(source, text, registry.compile_view())
}

fn compile_standard_program_source_with_native_functions(
    source: SourceId,
    text: &str,
    natives: &[&str],
) -> vela_bytecode::compiler::error::CompileResult<vela_bytecode::compiler::CompiledProgram> {
    let mut registry = vela_stdlib::standard_registry().expect("standard registry should build");
    for native in natives {
        let mut segments = native.split("::").collect::<Vec<_>>();
        let function = segments.pop().unwrap_or(native);
        registry
            .register_function(vela_registry::FunctionDef::new(
                vela_def::DefPath::function("host", segments, function),
                vela_registry::FunctionSignature::default(),
            ))
            .expect("test native function should register");
    }
    compile_test_program_with_registry(source, text, registry.compile_view())
}
