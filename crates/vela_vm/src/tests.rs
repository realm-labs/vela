use super::*;
use crate::budget::ExecutionBudgetKind;
use crate::heap::{GcBudget, HeapValue, ScriptHeap};
use std::collections::BTreeMap;
use std::sync::Arc;
use vela_bytecode::compiler::options::CompilerOptions;
use vela_bytecode::compiler::{
    compile_function_source, compile_module_sources, compile_program_source,
    compile_program_source_with_options,
};
use vela_bytecode::{
    CacheSiteKind, Constant, ConstantId, Instruction, InstructionOffset, ProgramImage,
};
use vela_common::{
    FieldId, FunctionId, HostMethodId, HostObjectId, HostTypeId, MethodId, SourceId, TypeId,
    VariantId,
};
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

mod consts;
mod control_flow;
mod execution_core;
mod heap_host;
mod host_fields;
mod host_methods;
mod iteration;
mod modules;
mod owned_boundary;
mod program_execution;
mod records_enums;
mod reflection_members;
mod reflection_metadata;
mod reflection_modules;
mod reflection_permissions;
mod reflection_values;
mod script_methods;

fn host_read_program() -> (Program, HostRef) {
    let host_ref = player_ref(3);
    let mut code = CodeObject::new("main", 2).with_params(vec!["player".into()]);
    let target =
        code.intern_host_target(HostTargetPlan::new(host_ref.type_id).field(level_field()));
    let cache_site = code.push_cache_site(CacheSiteKind::HostPathRead, InstructionOffset(0));
    code.push_instruction(Instruction::new(InstructionKind::HostRead {
        dst: Register(1),
        root: Register(0),
        target,
        dynamic_args: Vec::new(),
        cache_site,
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(1),
    }));
    let mut program = Program::new();
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
                    .type_hint("int")
                    .docs("Current player level.")
                    .attr("unit", "level"),
            )
            .method(
                MethodDesc::new(HostMethodId::new(5), "grant_exp")
                    .effects(MethodEffectSet::host_write())
                    .param(MethodParamDesc::new("amount").type_hint("int"))
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
pub fn grant(player: Player, amount: int = 1) -> bool {
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
