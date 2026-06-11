use super::*;
use crate::budget::ExecutionBudgetKind;
use crate::heap::{GcBudget, HeapValue, ScriptHeap};
use std::collections::BTreeMap;
use std::sync::Arc;
use vela_bytecode::compiler::options::{CompilerOptions, HostIndexCapabilityInfo};
use vela_bytecode::compiler::{
    compile_function_source, compile_module_sources, compile_program_source,
    compile_program_source_with_registry,
};
use vela_bytecode::{
    CacheSiteKind, Constant, ConstantId, InstructionOffset, LinkedProgram, Linker, ProgramImage,
    UnlinkedInstruction,
};
use vela_common::{HostMethodId, HostObjectId, HostTypeId, SourceId};
use vela_def::{FieldId, FunctionId, MethodId, TypeId, VariantId};
use vela_hir::module_graph::{ModuleGraph, ModulePath, ModuleSource};
use vela_host::access::HostAccess;
use vela_host::error::HostErrorKind;
use vela_host::mock::MockStateAdapter;
use vela_host::path::{HostPath, HostRef};
use vela_host::proxy::PathProxy;
use vela_host::target::HostTargetPlan;
use vela_host::value::HostValue;
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

mod bytes;
mod consts;
mod control_flow;
mod deferred_literals;
mod execution_core;
mod heap_host;
mod host_fields;
mod host_methods;
mod iteration;
mod linked_execution_core;
mod modules;
mod numeric_conversions;
mod owned_boundary;
mod program_execution;
mod records_enums;
mod reflection_members;
mod reflection_metadata;
mod reflection_modules;
mod reflection_permissions;
mod reflection_values;
mod scalar_numeric_ops;
mod script_methods;
mod standard_array_id_dispatch;
mod standard_callback_id_dispatch;
mod standard_id_dispatch;
mod standard_map_set_id_dispatch;
mod standard_option_result_id_dispatch;
mod standard_string_id_dispatch;
mod type_guards;

fn link_test_program(program: &UnlinkedProgram) -> LinkedProgram {
    Linker::new()
        .link_program(program)
        .expect("test program should link")
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
    vm.native_ids
        .keys()
        .chain(vm.host_native_ids.keys())
        .copied()
        .for_each(|id| linker.add_native_implementation(id));
    let linked = linker
        .link_program(program)
        .expect("test program should link");
    vm.run_linked_program_with_budget(&linked, entry, args, budget)
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
    vm.native_ids
        .keys()
        .chain(vm.host_native_ids.keys())
        .copied()
        .for_each(|id| linker.add_native_implementation(id));
    let linked = linker
        .link_program(program)
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
    vm.native_ids
        .keys()
        .chain(vm.host_native_ids.keys())
        .copied()
        .for_each(|id| linker.add_native_implementation(id));
    let linked = linker
        .link_program(program)
        .expect("test program should link");
    let code = linked
        .functions()
        .find_map(|(_, code)| (linked.debug_name(code.debug_name) == entry).then_some(code))
        .ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownFunction {
                name: entry.to_owned(),
            })
        })?;
    vm.execute_linked_call(
        crate::linked_execution::LinkedExecutionCall {
            code,
            program: &linked,
            captures: &[],
            args,
            check_param_guards: true,
            call_site: None,
            call_site_offset: None,
            inline_caches: None,
        },
        None,
        Some(heap),
        Some(budget),
    )
}

fn run_linked_test_program_runtime_with_host_heap_and_budget(
    vm: &Vm,
    program: &UnlinkedProgram,
    entry: &str,
    args: &[Value],
    host: &mut HostExecution<'_>,
    heap: &mut HeapExecution<'_>,
    budget: &mut ExecutionBudget,
) -> VmResult<Value> {
    let mut linker = Linker::new();
    vm.native_ids
        .keys()
        .chain(vm.host_native_ids.keys())
        .copied()
        .for_each(|id| linker.add_native_implementation(id));
    let linked = linker
        .link_program(program)
        .expect("test program should link");
    let code = linked
        .functions()
        .find_map(|(_, code)| (linked.debug_name(code.debug_name) == entry).then_some(code))
        .ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownFunction {
                name: entry.to_owned(),
            })
        })?;
    vm.execute_linked_call(
        crate::linked_execution::LinkedExecutionCall {
            code,
            program: &linked,
            captures: &[],
            args,
            check_param_guards: true,
            call_site: None,
            call_site_offset: None,
            inline_caches: None,
        },
        Some(host),
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
        .link_program(&program)
        .expect("test code should link");
    vm.run_linked_program(&linked, &entry, &[])
}

fn host_read_program() -> (UnlinkedProgram, HostRef) {
    let host_ref = player_ref(3);
    let mut code = UnlinkedCodeObject::new("main", 2).with_params(vec!["player".into()]);
    let target =
        code.intern_host_target(HostTargetPlan::new(host_ref.type_id).field(level_field()));
    let cache_site = code.push_cache_site(CacheSiteKind::HostPathRead, InstructionOffset(0));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::HostRead {
            dst: Register(1),
            root: Register(0),
            target,
            dynamic_args: Vec::new(),
            cache_site,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(1),
    }));
    let mut program = UnlinkedProgram::new();
    program.insert_function(code);
    (program, host_ref)
}

fn host_adapter(host_ref: HostRef, value: HostValue) -> MockStateAdapter {
    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(level_path(host_ref), value);
    adapter
}

fn reflection_registry() -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    registry.register_trait(
        TraitDesc::new("Damageable")
            .method(TraitMethodDesc::new(MethodId::new(1), "damage").defaulted(true)),
    );
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .host_type(HostTypeId::new(1))
            .docs("A player host object.")
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
                MethodDesc::new(HostMethodId::new(5), "grant_exp")
                    .effects(MethodEffectSet::host_write())
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
        ModulePath::from_qualified("game::reward"),
        r#"
#[doc("Grant reward.")]
#[event("reward")]
pub fn grant(player: Player, amount: i64 = 1) -> bool {
    return true;
}
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
            .host_type(HostTypeId::new(1))
            .method(MethodDesc::new(HostMethodId::new(1), "visible"))
            .method(
                MethodDesc::new(HostMethodId::new(2), "hidden")
                    .access(MethodAccess::new().reflect_callable(false)),
            )
            .method(
                MethodDesc::new(HostMethodId::new(3), "private")
                    .access(MethodAccess::new().public(false).reflect_callable(true)),
            )
            .method(
                MethodDesc::new(HostMethodId::new(4), "admin")
                    .access(MethodAccess::new().require_permission("player.admin")),
            ),
    );
    registry
}

fn policy_field_reflection_registry() -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(600), "Player"))
            .host_type(HostTypeId::new(1))
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

fn player_ref(generation: u32) -> HostRef {
    HostRef::new(HostTypeId::new(1), HostObjectId::new(7), generation)
}

fn level_path(host_ref: HostRef) -> HostPath {
    HostPath::new(host_ref).field(level_field())
}

fn level_field() -> FieldId {
    FieldId::new(2)
}

fn compile_host_program_source(
    source: SourceId,
    text: &str,
    registry: vela_registry::DefinitionRegistry,
) -> vela_bytecode::compiler::error::CompileResult<UnlinkedProgram> {
    compile_program_source_with_registry(source, text, registry.compile_view())
}

fn compile_standard_program_source(
    source: SourceId,
    text: &str,
) -> vela_bytecode::compiler::error::CompileResult<UnlinkedProgram> {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    compile_program_source_with_registry(source, text, registry.compile_view())
}

fn compile_standard_program_source_with_native_functions(
    source: SourceId,
    text: &str,
    natives: &[&str],
) -> vela_bytecode::compiler::error::CompileResult<UnlinkedProgram> {
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
    compile_program_source_with_registry(source, text, registry.compile_view())
}

fn compile_host_program_source_with_options(
    source: SourceId,
    text: &str,
    options: &CompilerOptions,
    registry: vela_registry::DefinitionRegistry,
) -> vela_bytecode::compiler::error::CompileResult<UnlinkedProgram> {
    vela_bytecode::compiler::compile_program_source_with_options_and_registry(
        source,
        text,
        options,
        registry.compile_view(),
    )
}

fn host_definition_registry(
    types: &[(&str, HostTypeId)],
    fields: &[TestHostField<'_>],
    methods: &[TestHostMethod<'_>],
) -> vela_registry::DefinitionRegistry {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let mut type_ids = BTreeMap::new();
    for (name, runtime_id) in types {
        let type_id = registry
            .register_type(
                vela_registry::TypeDef::new(vela_def::DefPath::ty(
                    "host",
                    std::iter::empty::<&str>(),
                    *name,
                ))
                .host_runtime_id(runtime_id.get().into()),
            )
            .expect("test host type should register");
        type_ids.insert(*name, type_id);
    }

    for field in fields {
        let owner = *type_ids
            .get(field.owner)
            .expect("test field owner type should exist");
        let def = vela_registry::FieldDef::new(
            vela_def::DefPath::field("host", std::iter::empty::<&str>(), field.owner, field.name),
            owner,
        )
        .host_runtime_id(field.id.get())
        .writable(field.writable)
        .type_hint(field.type_hint.map(str::to_owned))
        .variant_field(field.variant);
        registry
            .register_field(def)
            .expect("test host field should register");
    }

    for method in methods {
        let owner = *type_ids
            .get(method.owner)
            .expect("test method owner type should exist");
        registry
            .register_method(
                vela_registry::MethodDef::new(
                    vela_def::DefPath::method(
                        "host",
                        std::iter::empty::<&str>(),
                        method.owner,
                        method.name,
                    ),
                    owner,
                    vela_registry::FunctionSignature::new(
                        method
                            .params
                            .iter()
                            .map(|name| vela_registry::ParamDef::new(*name, None::<String>)),
                        None,
                    ),
                )
                .host_runtime_id(method.id.get()),
            )
            .expect("test host method should register");
    }

    registry
}

#[derive(Clone, Copy)]
struct TestHostField<'a> {
    owner: &'a str,
    name: &'a str,
    id: FieldId,
    writable: bool,
    type_hint: Option<&'a str>,
    variant: bool,
}

impl<'a> TestHostField<'a> {
    const fn new(owner: &'a str, name: &'a str, id: FieldId) -> Self {
        Self {
            owner,
            name,
            id,
            writable: true,
            type_hint: None,
            variant: false,
        }
    }

    const fn readonly(mut self) -> Self {
        self.writable = false;
        self
    }

    const fn type_hint(mut self, type_hint: &'a str) -> Self {
        self.type_hint = Some(type_hint);
        self
    }
}

#[derive(Clone, Copy)]
struct TestHostMethod<'a> {
    owner: &'a str,
    name: &'a str,
    id: HostMethodId,
    params: &'a [&'a str],
}

impl<'a> TestHostMethod<'a> {
    const fn new(owner: &'a str, name: &'a str, id: HostMethodId, params: &'a [&'a str]) -> Self {
        Self {
            owner,
            name,
            id,
            params,
        }
    }
}
