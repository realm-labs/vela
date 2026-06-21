//! Minimal AST-to-bytecode compiler for the M2 VM loop.

mod assignments;
mod body_payloads;
mod cache_sites;
mod call_args;
mod calls;
mod const_eval;
mod constructors;
mod control_flow;
pub mod error;
mod expected_exprs;
mod expression_checks;
mod expression_payload_kinds;
mod expressions;
mod field_slots;
mod host_paths;
mod lambdas;
mod legacy_payloads;
mod map_literals;
mod methods;
mod operators;
pub mod options;
mod param_defaults;
mod paths;
mod patterns;
mod record_reflection_shapes;
mod record_shapes;
mod schema_defaults;
mod script_impls;
mod script_types;
mod semantic;
mod syntax_payloads;
mod value_flow;
mod value_types;

use std::collections::{BTreeMap, BTreeSet, HashMap};

use vela_common::{GlobalSlot, HostMethodId, HostTypeId, SourceId, Span};
use vela_def::{DefPath, FieldId, MethodId, TypeId};
use vela_hir::attributes::derived_traits;
use vela_hir::binding::{BindingMap, BindingResolution, LocalBindingKind};
use vela_hir::ids::{HirDeclId, HirLocalId};
#[cfg(test)]
use vela_hir::module_graph::ModulePath;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph, ModuleSource};
use vela_hir::type_hint::{FunctionSignature, HirTypeHint, ParamHint};
use vela_registry::RegistryCompileView;
#[cfg(test)]
use vela_syntax::ast::FunctionItem;
use vela_syntax::ast::{Argument, Block, Expr, ExprKind, Param};

use crate::{
    Constant, FrameSlotInfo, FrameSlotKind, GuardKind, GuardLocation, InstructionOffset, Register,
    UnlinkedCodeObject, UnlinkedGuardContext, UnlinkedInstruction, UnlinkedInstructionKind,
    UnlinkedProgram, UnlinkedTypeGuard, UnlinkedTypeGuardPlan,
};
use body_payloads::CompilerBodyPayload;
use cache_sites::{attach_cache_site, cache_site_kind};
use control_flow::LoopContext;
use error::{CompileError, CompileErrorKind, CompileResult};
use field_slots::ScriptFieldSlots;
use lambdas::LambdaCapture;
use options::CompilerOptions;
use param_defaults::ParamDefaultValue;
use patterns::enum_variant_path;
use record_shapes::ValueShapeFlow;
use schema_defaults::ScriptSchemaDefaults;
use script_types::{ScriptTypeFlow, type_hint_script_type};
use semantic::{parse_semantic_modules, parse_semantic_source};
use value_types::{RuntimeTypeFact, StandardRuntimeType, ValueTypeFlow, type_hint_value_type};

#[derive(Clone, Debug)]
struct CompilerFacts<'registry> {
    script_function_symbols: BTreeMap<HirDeclId, String>,
    script_function_signatures: BTreeMap<HirDeclId, Vec<ParamHint>>,
    script_method_ids: BTreeMap<(String, String), MethodId>,
    script_method_signatures: BTreeMap<(String, String), Vec<ParamHint>>,
    derived_operator_traits: BTreeMap<String, BTreeSet<String>>,
    script_field_slots: ScriptFieldSlots,
    schema_defaults: ScriptSchemaDefaults,
    type_symbols: BTreeMap<HirDeclId, String>,
    global_symbols: BTreeMap<HirDeclId, String>,
    global_slots: BTreeMap<String, GlobalSlot>,
    global_type_symbols: BTreeMap<HirDeclId, String>,
    const_values: BTreeMap<HirDeclId, Constant>,
    options: CompilerOptions,
    registry: Option<RegistryCompileView<'registry>>,
}

impl CompilerFacts<'_> {
    fn known_type_names(&self) -> Vec<String> {
        let mut names = self.type_symbols.values().cloned().collect::<Vec<_>>();
        if let Some(registry) = self.registry {
            names.extend(
                registry
                    .type_names_for_package("host")
                    .into_iter()
                    .map(str::to_owned),
            );
        }
        names
    }
}

pub fn compile_function_source(
    source: SourceId,
    text: &str,
    function_name: &str,
) -> CompileResult<UnlinkedCodeObject> {
    compile_function_source_with_options(source, text, function_name, &CompilerOptions::default())
}

pub fn compile_function_source_with_registry(
    source: SourceId,
    text: &str,
    function_name: &str,
    registry: RegistryCompileView<'_>,
) -> CompileResult<UnlinkedCodeObject> {
    compile_function_source_with_options_and_registry(
        source,
        text,
        function_name,
        &CompilerOptions::default(),
        registry,
    )
}

pub fn compile_function_source_with_options(
    source: SourceId,
    text: &str,
    function_name: &str,
    options: &CompilerOptions,
) -> CompileResult<UnlinkedCodeObject> {
    compile_function_source_inner(source, text, function_name, options, None)
}

pub fn compile_function_source_with_options_and_registry(
    source: SourceId,
    text: &str,
    function_name: &str,
    options: &CompilerOptions,
    registry: RegistryCompileView<'_>,
) -> CompileResult<UnlinkedCodeObject> {
    compile_function_source_inner(source, text, function_name, options, Some(registry))
}

fn compile_function_source_inner<'registry>(
    source: SourceId,
    text: &str,
    function_name: &str,
    options: &CompilerOptions,
    registry: Option<RegistryCompileView<'registry>>,
) -> CompileResult<UnlinkedCodeObject> {
    let semantic = parse_semantic_source(source, text)?;
    let script_function_symbols = semantic.script_function_symbols();
    let script_function_signatures = semantic.script_function_signatures();
    let type_symbols = semantic.type_symbols();
    let global_symbols = semantic.global_symbols();
    let global_slots = global_slots(&global_symbols);
    let global_type_symbols = semantic.global_type_symbols();
    let script_field_slots = semantic.script_field_slots(&type_symbols);
    let derived_operator_traits =
        derived_operator_traits(&semantic.script_metadata_graph(), &type_symbols);
    let const_values = semantic.const_values()?;
    let schema_defaults = semantic.schema_defaults(&type_symbols, &const_values);
    let facts = CompilerFacts {
        script_function_symbols,
        script_function_signatures,
        script_method_ids: BTreeMap::new(),
        script_method_signatures: BTreeMap::new(),
        derived_operator_traits,
        script_field_slots,
        schema_defaults,
        type_symbols,
        global_symbols,
        global_slots,
        global_type_symbols,
        const_values,
        options: options.clone(),
        registry,
    };
    let (payload, signature, bindings) = semantic.function(function_name).ok_or_else(|| {
        CompileError::new(CompileErrorKind::FunctionNotFound(function_name.to_owned()))
    })?;

    verify_code_object(
        Compiler::new_with_param_defaults(
            payload.function.name.clone(),
            payload.body,
            payload.param_defaults,
            signature,
            bindings,
            facts,
        )?
        .compile()?,
    )
}

pub fn compile_program_source(source: SourceId, text: &str) -> CompileResult<UnlinkedProgram> {
    compile_program_source_with_options(source, text, &CompilerOptions::default())
}

pub fn compile_program_source_with_registry(
    source: SourceId,
    text: &str,
    registry: RegistryCompileView<'_>,
) -> CompileResult<UnlinkedProgram> {
    compile_program_source_with_options_and_registry(
        source,
        text,
        &CompilerOptions::default(),
        registry,
    )
}

pub fn compile_program_source_with_options(
    source: SourceId,
    text: &str,
    options: &CompilerOptions,
) -> CompileResult<UnlinkedProgram> {
    compile_program_source_inner(source, text, options, None)
}

pub fn compile_program_source_with_options_and_registry(
    source: SourceId,
    text: &str,
    options: &CompilerOptions,
    registry: RegistryCompileView<'_>,
) -> CompileResult<UnlinkedProgram> {
    compile_program_source_inner(source, text, options, Some(registry))
}

fn compile_program_source_inner<'registry>(
    source: SourceId,
    text: &str,
    options: &CompilerOptions,
    registry: Option<RegistryCompileView<'registry>>,
) -> CompileResult<UnlinkedProgram> {
    let semantic = parse_semantic_source(source, text)?;
    let script_functions = semantic.script_function_names();
    let script_function_symbols = semantic.script_function_symbols();
    let script_function_signatures = semantic.script_function_signatures();
    let script_impl_methods = semantic.script_impl_methods();
    let script_method_ids = script_method_ids(&script_impl_methods);
    let script_method_signatures = script_method_signatures(&script_impl_methods);
    let type_symbols = semantic.type_symbols();
    let global_symbols = semantic.global_symbols();
    let global_slots = global_slots(&global_symbols);
    let global_type_symbols = semantic.global_type_symbols();
    let script_field_slots = semantic.script_field_slots(&type_symbols);
    let derived_operator_traits =
        derived_operator_traits(&semantic.script_metadata_graph(), &type_symbols);
    let const_values = semantic.const_values()?;
    let schema_defaults = semantic.schema_defaults(&type_symbols, &const_values);
    let facts = CompilerFacts {
        script_function_symbols,
        script_function_signatures,
        script_method_ids,
        script_method_signatures,
        derived_operator_traits,
        script_field_slots,
        schema_defaults,
        type_symbols,
        global_symbols,
        global_slots,
        global_type_symbols,
        const_values,
        options: options.clone(),
        registry,
    };
    let mut program = UnlinkedProgram::new();
    program.set_global_layout(global_names(&facts.global_symbols));

    for name in &script_functions {
        let (payload, signature, bindings) = semantic
            .function(name)
            .expect("HIR function declarations come from parsed function items");
        program.insert_function(
            Compiler::new_with_param_defaults(
                payload.function.name.clone(),
                payload.body,
                payload.param_defaults,
                signature,
                bindings,
                facts.clone(),
            )?
            .compile()?,
        );
    }
    insert_script_impl_methods(&mut program, script_impl_methods, &facts)?;
    program.set_script_metadata(semantic.script_metadata_graph());

    verify_program(program)
}

pub fn compile_module_sources(sources: &[ModuleSource]) -> CompileResult<UnlinkedProgram> {
    compile_module_sources_with_options(sources, &CompilerOptions::default())
}

pub fn compile_module_sources_with_registry(
    sources: &[ModuleSource],
    registry: RegistryCompileView<'_>,
) -> CompileResult<UnlinkedProgram> {
    compile_module_sources_with_options_and_registry(sources, &CompilerOptions::default(), registry)
}

pub fn compile_module_sources_with_options(
    sources: &[ModuleSource],
    options: &CompilerOptions,
) -> CompileResult<UnlinkedProgram> {
    compile_module_sources_inner(sources, options, None)
}

pub fn compile_module_sources_with_options_and_registry(
    sources: &[ModuleSource],
    options: &CompilerOptions,
    registry: RegistryCompileView<'_>,
) -> CompileResult<UnlinkedProgram> {
    compile_module_sources_inner(sources, options, Some(registry))
}

fn compile_module_sources_inner<'registry>(
    sources: &[ModuleSource],
    options: &CompilerOptions,
    registry: Option<RegistryCompileView<'registry>>,
) -> CompileResult<UnlinkedProgram> {
    let semantic = parse_semantic_modules(sources)?;
    let script_functions = semantic.script_function_declarations();
    let script_function_symbols = semantic.script_function_symbols();
    let script_function_signatures = semantic.script_function_signatures();
    let script_impl_methods = semantic.script_impl_methods();
    let script_method_ids = script_method_ids(&script_impl_methods);
    let script_method_signatures = script_method_signatures(&script_impl_methods);
    let type_symbols = semantic.type_symbols();
    let global_symbols = semantic.global_symbols();
    let global_slots = global_slots(&global_symbols);
    let global_type_symbols = semantic.global_type_symbols();
    let script_field_slots = semantic.script_field_slots(&type_symbols);
    let derived_operator_traits =
        derived_operator_traits(&semantic.script_metadata_graph(), &type_symbols);
    let const_values = semantic.const_values()?;
    let schema_defaults = semantic.schema_defaults(&type_symbols, &const_values);
    let facts = CompilerFacts {
        script_function_symbols,
        script_function_signatures,
        script_method_ids,
        script_method_signatures,
        derived_operator_traits,
        script_field_slots,
        schema_defaults,
        type_symbols,
        global_symbols,
        global_slots,
        global_type_symbols,
        const_values,
        options: options.clone(),
        registry,
    };
    let mut program = UnlinkedProgram::new();
    program.set_global_layout(global_names(&facts.global_symbols));

    for declaration in script_functions {
        let (payload, signature, bindings) = semantic
            .function(declaration)
            .expect("HIR function declaration comes from parsed function item");
        let code_name = facts
            .script_function_symbols
            .get(&declaration)
            .expect("script function symbol exists for declaration")
            .clone();
        program.insert_function(
            Compiler::new_with_param_defaults(
                code_name,
                payload.body,
                payload.param_defaults,
                signature,
                bindings,
                facts.clone(),
            )?
            .compile()?,
        );
    }
    insert_script_impl_methods(&mut program, script_impl_methods, &facts)?;
    program.set_script_metadata(semantic.script_metadata_graph());

    verify_program(program)
}

fn verify_program(program: UnlinkedProgram) -> CompileResult<UnlinkedProgram> {
    program
        .verify()
        .map_err(|error| CompileError::new(CompileErrorKind::BytecodeVerification(error)))?;
    Ok(program)
}

fn verify_code_object(code: UnlinkedCodeObject) -> CompileResult<UnlinkedCodeObject> {
    code.verify()
        .map_err(|error| CompileError::new(CompileErrorKind::BytecodeVerification(error)))?;
    Ok(code)
}

fn global_names(global_symbols: &BTreeMap<HirDeclId, String>) -> Vec<String> {
    global_symbols
        .values()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn global_slots(global_symbols: &BTreeMap<HirDeclId, String>) -> BTreeMap<String, GlobalSlot> {
    global_names(global_symbols)
        .into_iter()
        .enumerate()
        .map(|(slot, name)| (name, GlobalSlot::new(slot)))
        .collect()
}

fn derived_operator_traits(
    graph: &ModuleGraph,
    type_symbols: &BTreeMap<HirDeclId, String>,
) -> BTreeMap<String, BTreeSet<String>> {
    type_symbols
        .iter()
        .filter_map(|(declaration, type_name)| {
            let metadata = graph.declaration(*declaration)?;
            if metadata.kind != DeclarationKind::Struct {
                return None;
            }
            let traits = derived_traits(graph.declaration_attrs(*declaration))
                .into_iter()
                .filter(|trait_name| {
                    matches!(
                        trait_name.as_str(),
                        "PartialEq" | "Eq" | "PartialOrd" | "Ord"
                    )
                })
                .collect::<BTreeSet<_>>();
            (!traits.is_empty()).then(|| (type_name.clone(), traits))
        })
        .collect()
}

fn insert_script_impl_methods(
    program: &mut UnlinkedProgram,
    methods: Vec<script_impls::ScriptImplMethod<'_>>,
    facts: &CompilerFacts<'_>,
) -> CompileResult<()> {
    for method in methods {
        program.insert_script_method(
            method.target_type.clone(),
            method.method_name.clone(),
            method.method_id,
            method.symbol.clone(),
        );
        program.insert_function(
            Compiler::new_script_method_body(
                method.symbol,
                method.default_values.clone(),
                method.signature,
                method.body,
                method.bindings,
                &method.target_type,
                facts.clone(),
            )?
            .compile()?,
        );
    }
    Ok(())
}

fn script_method_ids(
    methods: &[script_impls::ScriptImplMethod<'_>],
) -> BTreeMap<(String, String), MethodId> {
    methods
        .iter()
        .map(|method| {
            (
                (method.target_type.clone(), method.method_name.clone()),
                method.method_id,
            )
        })
        .collect()
}

fn script_method_signatures(
    methods: &[script_impls::ScriptImplMethod<'_>],
) -> BTreeMap<(String, String), Vec<ParamHint>> {
    methods
        .iter()
        .map(|method| {
            (
                (method.target_type.clone(), method.method_name.clone()),
                method.signature.params.clone(),
            )
        })
        .collect()
}

fn type_guard_for_hint(
    hint: &HirTypeHint,
    location: GuardLocation,
    debug_name: impl Into<String>,
    facts: &CompilerFacts<'_>,
) -> Option<UnlinkedTypeGuard> {
    let plan = type_guard_plan_for_hint_inner(hint, facts)?;
    Some(UnlinkedTypeGuard::new(
        plan,
        UnlinkedGuardContext::new(GuardKind::Contract, location, debug_name),
    ))
}

fn type_guard_plan_for_hint_inner(
    hint: &HirTypeHint,
    facts: &CompilerFacts<'_>,
) -> Option<UnlinkedTypeGuardPlan> {
    let [name] = hint.path.as_slice() else {
        return None;
    };
    match name.as_str() {
        "Any" => None,
        "null" => Some(UnlinkedTypeGuardPlan::Primitive(
            vela_common::PrimitiveTag::Null,
        )),
        "bool" => Some(UnlinkedTypeGuardPlan::Primitive(
            vela_common::PrimitiveTag::Bool,
        )),
        "char" => Some(UnlinkedTypeGuardPlan::Primitive(
            vela_common::PrimitiveTag::Char,
        )),
        "i8" => Some(UnlinkedTypeGuardPlan::Primitive(
            vela_common::PrimitiveTag::I8,
        )),
        "i16" => Some(UnlinkedTypeGuardPlan::Primitive(
            vela_common::PrimitiveTag::I16,
        )),
        "i32" => Some(UnlinkedTypeGuardPlan::Primitive(
            vela_common::PrimitiveTag::I32,
        )),
        "i64" => Some(UnlinkedTypeGuardPlan::Primitive(
            vela_common::PrimitiveTag::I64,
        )),
        "u8" => Some(UnlinkedTypeGuardPlan::Primitive(
            vela_common::PrimitiveTag::U8,
        )),
        "u16" => Some(UnlinkedTypeGuardPlan::Primitive(
            vela_common::PrimitiveTag::U16,
        )),
        "u32" => Some(UnlinkedTypeGuardPlan::Primitive(
            vela_common::PrimitiveTag::U32,
        )),
        "u64" => Some(UnlinkedTypeGuardPlan::Primitive(
            vela_common::PrimitiveTag::U64,
        )),
        "f32" => Some(UnlinkedTypeGuardPlan::Primitive(
            vela_common::PrimitiveTag::F32,
        )),
        "f64" => Some(UnlinkedTypeGuardPlan::Primitive(
            vela_common::PrimitiveTag::F64,
        )),
        "String" => Some(UnlinkedTypeGuardPlan::Primitive(
            vela_common::PrimitiveTag::String,
        )),
        "Bytes" => Some(UnlinkedTypeGuardPlan::Primitive(
            vela_common::PrimitiveTag::Bytes,
        )),
        "Array" if hint.args.len() == 1 => Some(UnlinkedTypeGuardPlan::Array {
            element: type_guard_plan_for_hint_inner(&hint.args[0], facts).map(Box::new),
        }),
        "Array" => Some(UnlinkedTypeGuardPlan::Standard(
            crate::StandardTypeGuard::Array,
        )),
        "Map" if hint.args.len() == 2 => Some(UnlinkedTypeGuardPlan::Map {
            key: type_guard_plan_for_hint_inner(&hint.args[0], facts).map(Box::new),
            value: type_guard_plan_for_hint_inner(&hint.args[1], facts).map(Box::new),
        }),
        "Map" => Some(UnlinkedTypeGuardPlan::Standard(
            crate::StandardTypeGuard::Map,
        )),
        "Set" if hint.args.len() == 1 => Some(UnlinkedTypeGuardPlan::Set {
            element: type_guard_plan_for_hint_inner(&hint.args[0], facts).map(Box::new),
        }),
        "Set" => Some(UnlinkedTypeGuardPlan::Standard(
            crate::StandardTypeGuard::Set,
        )),
        "Range" => Some(UnlinkedTypeGuardPlan::Standard(
            crate::StandardTypeGuard::Range,
        )),
        "Function" => Some(UnlinkedTypeGuardPlan::Standard(
            crate::StandardTypeGuard::Function,
        )),
        "Closure" => Some(UnlinkedTypeGuardPlan::Standard(
            crate::StandardTypeGuard::Closure,
        )),
        "Iterator" if hint.args.len() == 1 => Some(UnlinkedTypeGuardPlan::Iterator {
            item: type_guard_plan_for_hint_inner(&hint.args[0], facts).map(Box::new),
        }),
        "Iterator" => Some(UnlinkedTypeGuardPlan::Standard(
            crate::StandardTypeGuard::Iterator,
        )),
        "Option" if hint.args.len() == 1 => Some(UnlinkedTypeGuardPlan::Option {
            some: type_guard_plan_for_hint_inner(&hint.args[0], facts).map(Box::new),
        }),
        "Option" => Some(UnlinkedTypeGuardPlan::Standard(
            crate::StandardTypeGuard::Option,
        )),
        "Result" if hint.args.len() == 2 => Some(UnlinkedTypeGuardPlan::Result {
            ok: type_guard_plan_for_hint_inner(&hint.args[0], facts).map(Box::new),
            err: type_guard_plan_for_hint_inner(&hint.args[1], facts).map(Box::new),
        }),
        "Result" => Some(UnlinkedTypeGuardPlan::Standard(
            crate::StandardTypeGuard::Result,
        )),
        _ => script_record_shape_guard_plan(name, facts)
            .or_else(|| host_type_guard_plan(name, facts.registry))
            .or_else(|| Some(UnlinkedTypeGuardPlan::Type(hint.display()))),
    }
}

fn script_record_shape_guard_plan(
    type_name: &str,
    facts: &CompilerFacts<'_>,
) -> Option<UnlinkedTypeGuardPlan> {
    let (type_name, shape_id) = facts.script_field_slots.record_shape_id(type_name)?;
    Some(UnlinkedTypeGuardPlan::Shape {
        type_name,
        shape_id,
    })
}

fn host_type_guard_plan(
    type_name: &str,
    registry: Option<vela_registry::RegistryCompileView<'_>>,
) -> Option<UnlinkedTypeGuardPlan> {
    let registry = registry?;
    let type_id =
        registry.resolve_type(&DefPath::ty("host", std::iter::empty::<&str>(), type_name))?;
    let runtime_id = registry.type_host_runtime_id(type_id)?;
    let runtime_id = u64::try_from(runtime_id).ok()?;
    Some(UnlinkedTypeGuardPlan::HostType {
        type_name: type_name.to_owned(),
        host_type_id: HostTypeId::new(runtime_id),
    })
}

fn type_guard_plan_for_runtime_type(ty: &RuntimeTypeFact) -> Option<UnlinkedTypeGuardPlan> {
    match ty {
        RuntimeTypeFact::Primitive(tag) => Some(UnlinkedTypeGuardPlan::Primitive(*tag)),
        RuntimeTypeFact::Standard(StandardRuntimeType::Array) => Some(
            UnlinkedTypeGuardPlan::Standard(crate::StandardTypeGuard::Array),
        ),
        RuntimeTypeFact::Standard(StandardRuntimeType::Map) => Some(
            UnlinkedTypeGuardPlan::Standard(crate::StandardTypeGuard::Map),
        ),
        RuntimeTypeFact::Standard(StandardRuntimeType::Set) => Some(
            UnlinkedTypeGuardPlan::Standard(crate::StandardTypeGuard::Set),
        ),
        RuntimeTypeFact::Standard(StandardRuntimeType::Range) => Some(
            UnlinkedTypeGuardPlan::Standard(crate::StandardTypeGuard::Range),
        ),
        RuntimeTypeFact::Standard(StandardRuntimeType::Function) => Some(
            UnlinkedTypeGuardPlan::Standard(crate::StandardTypeGuard::Function),
        ),
        RuntimeTypeFact::Standard(StandardRuntimeType::Closure) => Some(
            UnlinkedTypeGuardPlan::Standard(crate::StandardTypeGuard::Closure),
        ),
        RuntimeTypeFact::Standard(StandardRuntimeType::Iterator) => Some(
            UnlinkedTypeGuardPlan::Standard(crate::StandardTypeGuard::Iterator),
        ),
        RuntimeTypeFact::Standard(StandardRuntimeType::Option) => Some(
            UnlinkedTypeGuardPlan::Standard(crate::StandardTypeGuard::Option),
        ),
        RuntimeTypeFact::Standard(StandardRuntimeType::Result) => Some(
            UnlinkedTypeGuardPlan::Standard(crate::StandardTypeGuard::Result),
        ),
        RuntimeTypeFact::Array(element) => Some(UnlinkedTypeGuardPlan::Array {
            element: type_guard_plan_for_runtime_type(element).map(Box::new),
        }),
        RuntimeTypeFact::Map { key, value } => Some(UnlinkedTypeGuardPlan::Map {
            key: type_guard_plan_for_runtime_type(key).map(Box::new),
            value: type_guard_plan_for_runtime_type(value).map(Box::new),
        }),
        RuntimeTypeFact::Set(element) => Some(UnlinkedTypeGuardPlan::Set {
            element: type_guard_plan_for_runtime_type(element).map(Box::new),
        }),
        RuntimeTypeFact::Iterator(item) => Some(UnlinkedTypeGuardPlan::Iterator {
            item: type_guard_plan_for_runtime_type(item).map(Box::new),
        }),
        RuntimeTypeFact::Option(payload) => Some(UnlinkedTypeGuardPlan::Option {
            some: type_guard_plan_for_runtime_type(payload).map(Box::new),
        }),
        RuntimeTypeFact::Result { ok, err } => Some(UnlinkedTypeGuardPlan::Result {
            ok: type_guard_plan_for_runtime_type(ok).map(Box::new),
            err: type_guard_plan_for_runtime_type(err).map(Box::new),
        }),
    }
}

struct Compiler<'ast, 'registry> {
    code: UnlinkedCodeObject,
    locals: HashMap<String, Register>,
    hir_locals: HashMap<HirLocalId, Register>,
    script_types: ScriptTypeFlow,
    value_types: ValueTypeFlow,
    value_shapes: ValueShapeFlow,
    bindings: &'ast BindingMap,
    next_register: u16,
    param_defaults: Vec<Option<ParamDefaultValue>>,
    return_type: Option<RuntimeTypeFact>,
    body: CompilerBodyPayload<'ast>,
    facts: CompilerFacts<'registry>,
    loop_stack: Vec<LoopContext>,
}

impl<'ast, 'registry> Compiler<'ast, 'registry> {
    #[cfg(test)]
    fn new(
        code_name: String,
        function: &'ast FunctionItem,
        signature: &FunctionSignature,
        bindings: &'ast BindingMap,
        facts: CompilerFacts<'registry>,
    ) -> CompileResult<Self> {
        let param_defaults = (0..signature.params.len())
            .map(|index| {
                function
                    .params
                    .get(index)
                    .and_then(|param| param.default_value.clone())
                    .map(ParamDefaultValue::Legacy)
            })
            .collect();
        Self::new_body(
            code_name,
            param_defaults,
            signature,
            CompilerBodyPayload::legacy(&function.body),
            bindings,
            facts,
        )
    }

    fn new_with_param_defaults(
        code_name: String,
        body: CompilerBodyPayload<'ast>,
        param_defaults: Vec<Option<ParamDefaultValue>>,
        signature: &FunctionSignature,
        bindings: &'ast BindingMap,
        facts: CompilerFacts<'registry>,
    ) -> CompileResult<Self> {
        Self::new_body(code_name, param_defaults, signature, body, bindings, facts)
    }

    fn new_body(
        code_name: String,
        param_defaults: Vec<Option<ParamDefaultValue>>,
        signature: &FunctionSignature,
        body: CompilerBodyPayload<'ast>,
        bindings: &'ast BindingMap,
        facts: CompilerFacts<'registry>,
    ) -> CompileResult<Self> {
        let param_count = u16::try_from(signature.params.len())
            .map_err(|_| CompileError::new(CompileErrorKind::RegisterOverflow))?;
        let param_names = signature
            .params
            .iter()
            .map(|param| param.name.clone())
            .collect::<Vec<_>>();
        let param_default_flags = param_default_flags(signature);
        let return_type = signature
            .return_type
            .as_ref()
            .and_then(type_hint_value_type);
        let mut code = UnlinkedCodeObject::new(code_name, 0)
            .with_params(param_names)
            .with_param_defaults(param_default_flags);
        if let Some(return_type) = &signature.return_type
            && let Some(guard) =
                type_guard_for_hint(return_type, GuardLocation::Return, "return", &facts)
        {
            code.set_return_guard(guard);
        }
        let mut locals = HashMap::new();
        let mut hir_locals = HashMap::new();
        let mut script_types = ScriptTypeFlow::default();
        let mut value_types = ValueTypeFlow::default();
        let value_shapes = ValueShapeFlow::default();
        let parameter_locals = bindings
            .locals()
            .filter(|local| local.kind == LocalBindingKind::Parameter)
            .map(|local| local.id)
            .collect::<Vec<_>>();
        let known_type_names = facts.known_type_names();
        for (index, param) in signature.params.iter().enumerate() {
            let register = u16::try_from(index)
                .map_err(|_| CompileError::new(CompileErrorKind::RegisterOverflow))?;
            if let Some(type_hint) = &param.type_hint
                && let Some(guard) = type_guard_for_hint(
                    type_hint,
                    GuardLocation::Parameter { index: register },
                    param.name.clone(),
                    &facts,
                )
            {
                code.push_param_guard(register, guard);
            }
            locals.insert(param.name.clone(), Register(register));
            let script_type = param
                .type_hint
                .as_ref()
                .and_then(|hint| type_hint_script_type(hint, known_type_names.iter()));
            let value_type = param.type_hint.as_ref().and_then(type_hint_value_type);
            if let Some(local) = parameter_locals.get(index).copied() {
                hir_locals.insert(local, Register(register));
                script_types.set_local(local, &param.name, script_type);
                value_types.set_local(local, &param.name, value_type);
                code.frame.push_slot(FrameSlotInfo::new(
                    param.name.clone(),
                    Register(register),
                    FrameSlotKind::Parameter,
                    Some(local),
                    Some(param.span),
                ));
            } else {
                script_types.set_name(&param.name, script_type);
                value_types.set_name(&param.name, value_type);
                code.frame.push_slot(FrameSlotInfo::new(
                    param.name.clone(),
                    Register(register),
                    FrameSlotKind::Parameter,
                    None,
                    Some(param.span),
                ));
            }
        }

        Ok(Self {
            code,
            locals,
            hir_locals,
            script_types,
            value_types,
            value_shapes,
            bindings,
            next_register: param_count,
            param_defaults,
            return_type,
            body,
            facts,
            loop_stack: Vec::new(),
        })
    }

    fn new_script_method_body(
        code_name: String,
        param_defaults: Vec<Option<ParamDefaultValue>>,
        signature: &FunctionSignature,
        body: CompilerBodyPayload<'ast>,
        bindings: &'ast BindingMap,
        receiver_type: &str,
        facts: CompilerFacts<'registry>,
    ) -> CompileResult<Self> {
        let mut compiler =
            Self::new_body(code_name, param_defaults, signature, body, bindings, facts)?;
        compiler
            .script_types
            .set_name("self", Some(receiver_type.to_owned()));
        Ok(compiler)
    }

    fn new_lambda(
        name: String,
        _lambda_span: Span,
        params: &[Param],
        fallback_body: &'ast Block,
        captures: &[LambdaCapture],
        bindings: &'ast BindingMap,
        facts: CompilerFacts<'registry>,
    ) -> CompileResult<Self> {
        let capture_count = u16::try_from(captures.len())
            .map_err(|_| CompileError::new(CompileErrorKind::RegisterOverflow))?;
        let param_count = u16::try_from(params.len())
            .map_err(|_| CompileError::new(CompileErrorKind::RegisterOverflow))?;
        let param_names = params
            .iter()
            .map(|param| param.name.clone())
            .collect::<Vec<_>>();
        let param_default_flags = vec![false; params.len()];
        let mut code = UnlinkedCodeObject::new(name, 0)
            .with_params(param_names)
            .with_param_defaults(param_default_flags)
            .with_capture_count(capture_count);
        let mut locals = HashMap::new();
        let mut hir_locals = HashMap::new();
        let mut script_types = ScriptTypeFlow::default();
        let mut value_types = ValueTypeFlow::default();
        let value_shapes = ValueShapeFlow::default();

        for (index, capture) in captures.iter().enumerate() {
            let register = Register(
                u16::try_from(index)
                    .map_err(|_| CompileError::new(CompileErrorKind::RegisterOverflow))?,
            );
            locals.insert(capture.name.clone(), register);
            hir_locals.insert(capture.local, register);
            let span = bindings.local(capture.local).map(|local| local.span);
            code.frame.push_slot(FrameSlotInfo::new(
                capture.name.clone(),
                register,
                FrameSlotKind::Capture,
                Some(capture.local),
                span,
            ));
        }
        let known_type_names = facts.known_type_names();
        for (index, param) in params.iter().enumerate() {
            let local_binding = bindings
                .local_named_at(&param.name, LocalBindingKind::LambdaParameter, param.span)
                .and_then(|local| {
                    bindings
                        .local(local)
                        .map(|binding| (local, binding.type_hint.clone()))
                });
            let hir_type_hint = local_binding.as_ref().and_then(|(_, hint)| hint.as_ref());
            let register = Register(
                capture_count
                    .checked_add(
                        u16::try_from(index)
                            .map_err(|_| CompileError::new(CompileErrorKind::RegisterOverflow))?,
                    )
                    .ok_or_else(|| CompileError::new(CompileErrorKind::RegisterOverflow))?,
            );
            if let Some(hint) = hir_type_hint
                && let Some(guard) = type_guard_for_hint(
                    hint,
                    GuardLocation::Parameter {
                        index: u16::try_from(index)
                            .map_err(|_| CompileError::new(CompileErrorKind::RegisterOverflow))?,
                    },
                    param.name.clone(),
                    &facts,
                )
            {
                code.push_param_guard(
                    u16::try_from(index)
                        .map_err(|_| CompileError::new(CompileErrorKind::RegisterOverflow))?,
                    guard,
                );
            }
            locals.insert(param.name.clone(), register);
            let script_type =
                hir_type_hint.and_then(|hint| type_hint_script_type(hint, known_type_names.iter()));
            let value_type = hir_type_hint.and_then(type_hint_value_type);
            if let Some((local, _)) = local_binding {
                hir_locals.insert(local, register);
                script_types.set_local(local, &param.name, script_type);
                value_types.set_local(local, &param.name, value_type);
                code.frame.push_slot(FrameSlotInfo::new(
                    param.name.clone(),
                    register,
                    FrameSlotKind::LambdaParameter,
                    Some(local),
                    Some(param.span),
                ));
            } else {
                script_types.set_name(&param.name, script_type);
                value_types.set_name(&param.name, value_type);
                code.frame.push_slot(FrameSlotInfo::new(
                    param.name.clone(),
                    register,
                    FrameSlotKind::LambdaParameter,
                    None,
                    Some(param.span),
                ));
            }
        }

        Ok(Self {
            code,
            locals,
            hir_locals,
            script_types,
            value_types,
            value_shapes,
            bindings,
            next_register: capture_count
                .checked_add(param_count)
                .ok_or_else(|| CompileError::new(CompileErrorKind::RegisterOverflow))?,
            param_defaults: vec![None; params.len()],
            return_type: None,
            body: CompilerBodyPayload::legacy(fallback_body),
            facts,
            loop_stack: Vec::new(),
        })
    }

    fn compile(mut self) -> CompileResult<UnlinkedCodeObject> {
        self.compile_param_defaults()?;
        let statements = self.body.statement_payloads();
        let returned = self.compile_statement_payloads(&statements)?;
        if !returned {
            let null = self.emit_constant(Constant::Null)?;
            self.emit(UnlinkedInstructionKind::Return { src: null });
        }
        self.code.register_count = self.next_register;
        Ok(self.code)
    }

    fn compile_param_defaults(&mut self) -> CompileResult<()> {
        for index in 0..self.param_defaults.len() {
            let Some(default_value) = self.param_defaults[index].clone() else {
                continue;
            };
            let param = Register(
                self.code
                    .capture_count
                    .checked_add(
                        u16::try_from(index)
                            .map_err(|_| CompileError::new(CompileErrorKind::RegisterOverflow))?,
                    )
                    .ok_or_else(|| CompileError::new(CompileErrorKind::RegisterOverflow))?,
            );
            let skip_default = self.emit_jump_if_not_missing(param);
            let default_payload = default_value.compiler_payload();
            let value =
                self.compile_expr_with_payload(default_value.fallback(), default_payload.as_ref())?;
            self.emit(UnlinkedInstructionKind::Move {
                dst: param,
                src: value,
            });
            self.patch_jump(skip_default, self.current_offset())?;
        }
        Ok(())
    }

    fn tuple_enum_constructor_call(&self, callee: &Expr) -> Option<(String, String)> {
        let ExprKind::Path(path) = &callee.kind else {
            return None;
        };
        let (_, variant) = enum_variant_path(path)?;
        let enum_name = self.type_symbol_at_span(callee.span)?;
        Some((enum_name, variant))
    }

    fn script_function_call(&self, callee: &Expr) -> Option<(HirDeclId, String)> {
        let Some(BindingResolution::Declaration(declaration)) =
            self.bindings.resolution_at_span(callee.span)
        else {
            return None;
        };
        self.facts
            .script_function_symbols
            .get(declaration)
            .cloned()
            .map(|name| (*declaration, name))
    }

    fn local_callee(&self, callee: &Expr) -> Option<HirLocalId> {
        let Some(BindingResolution::Local(local)) = self.bindings.resolution_at_span(callee.span)
        else {
            return None;
        };
        Some(*local)
    }

    fn type_symbol_at_span(&self, span: Span) -> Option<String> {
        let Some(BindingResolution::Declaration(declaration)) =
            self.bindings.resolution_at_span(span)
        else {
            return None;
        };
        self.facts.type_symbols.get(declaration).cloned()
    }

    fn global_type_at_span(&self, span: Span) -> Option<String> {
        let Some(BindingResolution::Declaration(declaration)) =
            self.bindings.resolution_at_span(span)
        else {
            return None;
        };
        self.facts.global_type_symbols.get(declaration).cloned()
    }

    fn global_symbol_named(&self, name: &str) -> Option<String> {
        unique_symbol_with_short_name(self.facts.global_symbols.values(), name)
    }

    fn global_type_named(&self, name: &str) -> Option<String> {
        let global = self.global_symbol_named(name)?;
        self.facts
            .global_symbols
            .iter()
            .find_map(|(declaration, symbol)| {
                (symbol == &global)
                    .then(|| self.facts.global_type_symbols.get(declaration).cloned())
                    .flatten()
            })
    }

    fn host_method_receiver_type(&self, callee: &Expr) -> Option<String> {
        match &callee.kind {
            ExprKind::Field { base, .. } => self.script_type_for_expr(base),
            ExprKind::Path(path) => {
                let [receiver, _method] = path.as_slice() else {
                    return None;
                };
                self.script_types
                    .name(receiver)
                    .or_else(|| self.global_type_at_span(callee.span))
            }
            _ => None,
        }
    }

    fn script_record_field_slot_for_type(&self, type_name: &str, field: &str) -> Option<usize> {
        self.facts.script_field_slots.record(type_name, field)
    }

    fn script_type_for_receiver_path(&self, receiver_path: &[String]) -> Option<String> {
        let [receiver] = receiver_path else {
            return None;
        };
        self.script_types.name(receiver)
    }

    fn value_type_for_receiver_path(&self, receiver_path: &[String]) -> Option<RuntimeTypeFact> {
        let [receiver] = receiver_path else {
            let (field, prefix) = receiver_path.split_last()?;
            let root = prefix.first()?;
            let mut shape = self.value_shapes.name(root)?.as_record().cloned()?;
            for segment in prefix.iter().skip(1) {
                shape = shape.field_record_shape(segment)?.clone();
            }
            return shape.field_value_type(field);
        };
        self.value_types.name(receiver)
    }

    fn script_method_id_for_type(&self, type_name: &str, method: &str) -> Option<MethodId> {
        self.facts
            .script_method_ids
            .get(&(type_name.to_owned(), method.to_owned()))
            .copied()
    }

    fn script_method_params(&self, type_name: &str, method: &str) -> Option<Vec<ParamHint>> {
        self.facts
            .script_method_signatures
            .get(&(type_name.to_owned(), method.to_owned()))
            .cloned()
    }

    fn host_type_id_for_name(&self, type_name: &str) -> Option<TypeId> {
        let registry = self.facts.registry?;
        registry.resolve_type(&DefPath::ty("host", std::iter::empty::<&str>(), type_name))
    }

    pub(super) fn is_native_module_root(&self, root: &str) -> bool {
        self.facts.options.is_native_module_root(root)
    }

    pub(super) fn host_runtime_type_id(&self, type_name: &str) -> Option<HostTypeId> {
        if let Some(registry) = self.facts.registry
            && let Some(type_id) = self.host_type_id_for_name(type_name)
            && let Some(runtime_id) = registry.type_host_runtime_id(type_id)
            && let Ok(runtime_id) = u64::try_from(runtime_id)
        {
            return Some(HostTypeId::new(runtime_id));
        }
        None
    }

    pub(super) fn host_field_info(
        &self,
        receiver_type: Option<&str>,
        name: &str,
    ) -> Option<HostFieldLookup> {
        if let Some(registry) = self.facts.registry
            && let Some(receiver_type) = receiver_type
            && let Some(owner) = self.host_type_id_for_name(receiver_type)
            && let Some(id) = registry.resolve_host_field(owner, name)
        {
            let runtime_id = registry
                .field_host_runtime_id(id)
                .map(FieldId::new)
                .unwrap_or(id);
            return Some(HostFieldLookup {
                id: runtime_id,
                writable: registry.field_writable(id).unwrap_or(true),
                type_hint: registry.field_type_hint(id).map(|hint| hint.display()),
                variant_field: registry.field_is_variant_field(id).unwrap_or(false),
            });
        }
        None
    }

    pub(super) fn host_method_id(
        &self,
        receiver_type: Option<&str>,
        name: &str,
    ) -> Option<HostMethodId> {
        if let Some(registry) = self.facts.registry
            && let Some(receiver_type) = receiver_type
            && let Some(owner) = self.host_type_id_for_name(receiver_type)
            && let Some(method_id) = registry.resolve_host_method(owner, name)
            && let Some(runtime_id) = registry.host_method_runtime_id(method_id)
        {
            return Some(HostMethodId::new(runtime_id));
        }
        None
    }

    fn emit_constant(&mut self, constant: Constant) -> CompileResult<Register> {
        let dst = self.alloc_register()?;
        let constant = self.code.push_constant(constant);
        self.emit(UnlinkedInstructionKind::LoadConst { dst, constant });
        Ok(dst)
    }

    fn emit_bool_constant_to(&mut self, dst: Register, value: bool) {
        self.emit_constant_to(dst, Constant::Bool(value));
    }

    fn emit_constant_to(&mut self, dst: Register, value: Constant) {
        let constant = self.code.push_constant(value);
        self.emit(UnlinkedInstructionKind::LoadConst { dst, constant });
    }

    fn alloc_register(&mut self) -> CompileResult<Register> {
        let register = self.next_register;
        self.next_register = self
            .next_register
            .checked_add(1)
            .ok_or_else(|| CompileError::new(CompileErrorKind::RegisterOverflow))?;
        Ok(Register(register))
    }

    fn emit(&mut self, kind: UnlinkedInstructionKind) {
        let offset = InstructionOffset(self.current_offset());
        let kind = if let Some(cache_kind) = cache_site_kind(&kind) {
            let cache_site = self.code.push_cache_site(cache_kind, offset);
            attach_cache_site(kind, cache_site)
        } else {
            kind
        };
        self.code.push_instruction(UnlinkedInstruction::new(kind));
    }

    fn emit_spanned(&mut self, kind: UnlinkedInstructionKind, span: Span) {
        let offset = InstructionOffset(self.current_offset());
        let kind = if let Some(cache_kind) = cache_site_kind(&kind) {
            let cache_site = self.code.push_cache_site(cache_kind, offset);
            attach_cache_site(kind, cache_site)
        } else {
            kind
        };
        self.code
            .push_instruction(UnlinkedInstruction::new(kind).with_span(span));
    }

    fn emit_jump_if_false(&mut self, condition: Register) -> usize {
        let offset = self.current_offset();
        self.emit(UnlinkedInstructionKind::JumpIfFalse {
            condition,
            target: InstructionOffset(usize::MAX),
        });
        offset
    }

    fn emit_jump_if_not_missing(&mut self, value: Register) -> usize {
        let offset = self.current_offset();
        self.emit(UnlinkedInstructionKind::JumpIfNotMissing {
            value,
            target: InstructionOffset(usize::MAX),
        });
        offset
    }

    fn emit_jump(&mut self) -> usize {
        let offset = self.current_offset();
        self.emit(UnlinkedInstructionKind::Jump {
            target: InstructionOffset(usize::MAX),
        });
        offset
    }

    fn emit_iter_next(&mut self, iterator: Register, dst: Register) -> usize {
        let offset = self.current_offset();
        self.emit(UnlinkedInstructionKind::IterNext {
            iterator,
            dst,
            jump_if_done: InstructionOffset(usize::MAX),
        });
        offset
    }

    fn emit_range_next(
        &mut self,
        cursor: Register,
        end: Register,
        done: Register,
        inclusive: bool,
        dst: Register,
    ) -> usize {
        let offset = self.current_offset();
        self.emit(UnlinkedInstructionKind::I64RangeNext {
            cursor,
            end,
            done,
            inclusive,
            dst,
            jump_if_done: InstructionOffset(usize::MAX),
        });
        offset
    }

    fn patch_jump(&mut self, offset: usize, target: usize) -> CompileResult<()> {
        let instruction =
            self.code.instructions.get_mut(offset).ok_or_else(|| {
                CompileError::new(CompileErrorKind::UnsupportedSyntax("jump patch"))
            })?;
        match &mut instruction.kind {
            UnlinkedInstructionKind::JumpIfFalse {
                target: jump_target,
                ..
            }
            | UnlinkedInstructionKind::JumpIfNotMissing {
                target: jump_target,
                ..
            }
            | UnlinkedInstructionKind::Jump {
                target: jump_target,
            }
            | UnlinkedInstructionKind::I64CmpImmJumpIfFalse {
                target: jump_target,
                ..
            }
            | UnlinkedInstructionKind::IterNext {
                jump_if_done: jump_target,
                ..
            }
            | UnlinkedInstructionKind::RangeNext {
                jump_if_done: jump_target,
                ..
            }
            | UnlinkedInstructionKind::I64RangeNext {
                jump_if_done: jump_target,
                ..
            } => {
                *jump_target = InstructionOffset(target);
                Ok(())
            }
            _ => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "jump patch",
            ))),
        }
    }

    fn current_offset(&self) -> usize {
        self.code.instructions.len()
    }

    fn record_frame_slot(
        &mut self,
        name: impl Into<String>,
        register: Register,
        kind: FrameSlotKind,
        local: Option<HirLocalId>,
        span: Option<Span>,
    ) {
        self.code
            .frame
            .push_slot(FrameSlotInfo::new(name, register, kind, local, span));
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct HostFieldLookup {
    pub(super) id: FieldId,
    pub(super) writable: bool,
    pub(super) type_hint: Option<String>,
    pub(super) variant_field: bool,
}

fn unique_symbol_with_short_name<'a>(
    symbols: impl IntoIterator<Item = &'a String>,
    name: &str,
) -> Option<String> {
    let mut matched = None;
    for symbol in symbols {
        if symbol.rsplit("::").next() == Some(name) {
            if matched.is_some() {
                return None;
            }
            matched = Some(symbol.clone());
        }
    }
    matched
}

fn frame_slot_kind(kind: LocalBindingKind) -> FrameSlotKind {
    match kind {
        LocalBindingKind::Parameter => FrameSlotKind::Parameter,
        LocalBindingKind::Let => FrameSlotKind::Local,
        LocalBindingKind::For => FrameSlotKind::ForBinding,
        LocalBindingKind::LambdaParameter => FrameSlotKind::LambdaParameter,
        LocalBindingKind::Pattern => FrameSlotKind::PatternBinding,
    }
}

fn param_default_flags(signature: &FunctionSignature) -> Vec<bool> {
    signature
        .params
        .iter()
        .map(|param| param.default_value_span.is_some())
        .collect()
}

fn reject_named_args(args: &[Argument], context: &'static str) -> CompileResult<()> {
    if args.iter().any(|arg| arg.name.is_some()) {
        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            context,
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
