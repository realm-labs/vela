//! Heavy-HIR -> verified MIR -> bytecode production compiler.

mod cache_sites;
mod const_eval;
mod constant_encoding;
pub mod error;
pub(crate) mod mir_backend;
pub mod options;
mod schema_defaults;
mod semantic;
mod semantic_input;
#[cfg(test)]
#[allow(clippy::result_large_err)]
mod test_support;

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Deref;
use std::sync::Arc;

#[cfg(test)]
use vela_common::SourceId;
use vela_common::{Capability, CapabilitySet, Span};
use vela_hir::ids::HirDeclId;
#[cfg(test)]
use vela_hir::module_graph::ModuleSource;
use vela_hir::module_graph::{ModuleGraph, Visibility};
use vela_hir::source_ingestion::{HirSourceFunction, HirSourceSet, HirSourceSetKind};
use vela_mir::{CompileStateStorage, CompileTypeClass, MirTargetTable};
use vela_package::PackageId;
use vela_registry::RegistryCompileView;

#[cfg(test)]
use crate::{Constant, UnlinkedTypeGuardPlan};
use crate::{
    NominalFieldDescriptor, NominalTypeDescriptor, NominalTypeKind, NominalVariantDescriptor,
    StateDescriptor, StateStorage, StateVisibility, UnlinkedCodeObject, UnlinkedProgram,
};
use error::{CompilationRequestError, CompileError, CompileErrorKind, CompileResult};
use options::CompilerOptions;
use semantic::SemanticCompilation;
#[cfg(test)]
pub(crate) use test_support::*;

#[derive(Debug)]
pub struct CompiledProgram {
    bytecode: UnlinkedProgram,
    verified_mir: Arc<vela_mir::OwnedVerifiedMirBundle>,
    binding_schema: Arc<crate::RustBindingSchema>,
    mir_executables: Box<[CompiledMirExecutable]>,
    budget_layouts: Box<[CompiledExecutableBudgetLayout]>,
    package_metadata: Option<crate::PackageCompilationMetadata>,
}

pub(crate) struct CompiledProgramParts {
    pub(crate) bytecode: UnlinkedProgram,
    pub(crate) verified_mir: Arc<vela_mir::OwnedVerifiedMirBundle>,
    pub(crate) binding_schema: Arc<crate::RustBindingSchema>,
    pub(crate) mir_executables: Box<[CompiledMirExecutable]>,
    pub(crate) budget_layouts: Box<[CompiledExecutableBudgetLayout]>,
    pub(crate) package_metadata: Option<crate::PackageCompilationMetadata>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompiledMirExecutable {
    pub(crate) root: vela_def::FunctionId,
    pub(crate) function: vela_mir::MirFunctionId,
}

pub(crate) type CompiledMirExecutableIdentity = CompiledMirExecutable;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CompiledExecutableBudgetLayout {
    pub(crate) sites: Box<[ExecutableBudgetSite]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExecutableBudgetSite {
    pub(crate) site: vela_mir::MirBudgetSite,
    pub(crate) offset: crate::InstructionOffset,
    pub(crate) class: vela_mir::MirBudgetClass,
    pub(crate) units: u32,
    pub(crate) boundary: ExecutableBudgetBoundary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExecutableBudgetBoundary {
    Operation,
    EdgeStub,
}

impl CompiledProgram {
    #[must_use]
    pub const fn bytecode(&self) -> &UnlinkedProgram {
        &self.bytecode
    }

    #[must_use]
    pub fn verified_mir(&self) -> &Arc<vela_mir::OwnedVerifiedMirBundle> {
        &self.verified_mir
    }

    #[must_use]
    pub fn binding_schema(&self) -> &Arc<crate::RustBindingSchema> {
        &self.binding_schema
    }

    #[doc(hidden)]
    pub fn binding_schema_mut(&mut self) -> &mut crate::RustBindingSchema {
        Arc::make_mut(&mut self.binding_schema)
    }

    #[must_use]
    pub const fn package_metadata(&self) -> Option<&crate::PackageCompilationMetadata> {
        self.package_metadata.as_ref()
    }

    #[must_use]
    pub(crate) fn into_linker_parts(self) -> CompiledProgramParts {
        CompiledProgramParts {
            bytecode: self.bytecode,
            verified_mir: self.verified_mir,
            binding_schema: self.binding_schema,
            mir_executables: self.mir_executables,
            budget_layouts: self.budget_layouts,
            package_metadata: self.package_metadata,
        }
    }

    /// Extracts bytecode for verifier-corruption and low-level VM tests.
    #[must_use]
    pub fn into_bytecode(self) -> UnlinkedProgram {
        self.bytecode
    }
}

impl Deref for CompiledProgram {
    type Target = UnlinkedProgram;

    fn deref(&self) -> &Self::Target {
        self.bytecode()
    }
}

pub struct ProgramCompilationRequest<'a> {
    pub sources: &'a HirSourceSet,
    pub options: &'a CompilerOptions,
    pub registry: Option<RegistryCompileView<'a>>,
}

pub struct PackageProgramCompilationRequest<'a> {
    pub sources: &'a HirSourceSet,
    pub options: &'a CompilerOptions,
    pub registry: Option<RegistryCompileView<'a>>,
    pub roots: &'a BTreeSet<PackageId>,
    pub packages: &'a [crate::PackageCompilationInput],
    pub providers: &'a [crate::ProviderCompilationInput],
}

pub struct FunctionCompilationRequest<'a> {
    pub function: HirSourceFunction<'a>,
    pub options: &'a CompilerOptions,
    pub registry: Option<RegistryCompileView<'a>>,
}

pub fn compile_function(
    request: FunctionCompilationRequest<'_>,
) -> CompileResult<UnlinkedCodeObject> {
    let sources = request.function.sources();
    let graph = sources.graph();
    let function = request.function.declaration();
    reject_invalid_graph(graph)?;
    let semantic = SemanticCompilation::new(sources)?;
    let script_function_symbols = semantic.function_symbols();
    let type_symbols = semantic.type_symbols();
    let state_symbols = semantic.state_symbols();
    let evaluated_constants = semantic.evaluated_constants()?;
    let schema_defaults = semantic.schema_defaults(&type_symbols, &evaluated_constants)?;
    let input = semantic_input::prepare_semantic_input(semantic_input::SemanticInputRequest {
        graph,
        roots: semantic_input::SemanticRoots::Function(function),
        script_function_symbols: &script_function_symbols,
        script_methods: semantic.script_method_catalog(),
        service_impls: semantic.service_impl_catalog(),
        type_symbols: &type_symbols,
        state_symbols: &state_symbols,
        evaluated_constants: &evaluated_constants,
        schema_defaults: &schema_defaults,
        options: request.options,
        registry: request.registry,
    })?;
    let (mut code, _) = compile_mir_roots(&input, graph)?;
    match code.len() {
        1 => Ok(code.pop().expect("one checked MIR root")),
        count => Err(CompileError::new(CompileErrorKind::InvalidMirRootCount {
            count,
        })),
    }
}

pub fn compile_program(request: ProgramCompilationRequest<'_>) -> CompileResult<CompiledProgram> {
    compile_program_inner(request.sources, request.options, request.registry, None)
}

pub fn compile_package_program(
    request: PackageProgramCompilationRequest<'_>,
) -> CompileResult<CompiledProgram> {
    compile_program_inner(
        request.sources,
        request.options,
        request.registry,
        Some((request.roots, request.packages, request.providers)),
    )
}

fn compile_program_inner(
    sources: &HirSourceSet,
    options: &CompilerOptions,
    registry: Option<RegistryCompileView<'_>>,
    package_request: Option<(
        &BTreeSet<PackageId>,
        &[crate::PackageCompilationInput],
        &[crate::ProviderCompilationInput],
    )>,
) -> CompileResult<CompiledProgram> {
    let graph = sources.graph();
    reject_invalid_graph(graph)?;
    validate_program_request(sources)?;
    let semantic = SemanticCompilation::new(sources)?;
    let script_function_symbols = semantic.function_symbols();
    let type_symbols = semantic.type_symbols();
    let state_symbols = semantic.state_symbols();
    let evaluated_constants = semantic.evaluated_constants()?;
    let schema_defaults = semantic.schema_defaults(&type_symbols, &evaluated_constants)?;
    let methods = semantic
        .script_method_catalog()
        .methods()
        .cloned()
        .collect::<Vec<_>>();
    let executable_packages = executable_packages(
        graph,
        &script_function_symbols,
        &state_symbols,
        &methods,
        semantic.service_impl_catalog(),
    )?;
    let input = semantic_input::prepare_semantic_input(semantic_input::SemanticInputRequest {
        graph,
        roots: semantic_input::SemanticRoots::Program,
        script_function_symbols: &script_function_symbols,
        script_methods: semantic.script_method_catalog(),
        service_impls: semantic.service_impl_catalog(),
        type_symbols: &type_symbols,
        state_symbols: &state_symbols,
        evaluated_constants: &evaluated_constants,
        schema_defaults: &schema_defaults,
        options,
        registry,
    })?;

    let mut program = UnlinkedProgram::new();
    program.set_states(state_descriptors(graph, &input, &state_symbols)?);
    program.set_nominal_types(nominal_type_descriptors(input.targets().target_table())?);
    let (mut code, verified_mir) = compile_mir_roots(&input, graph)?;
    let binding_schema = Arc::new(
        crate::binding_schema::build_rust_binding_schema(
            graph,
            &script_function_symbols,
            &type_symbols,
            &methods,
            &verified_mir,
        )
        .map_err(|message| CompileError::new(CompileErrorKind::RegistrySnapshot(message)))?,
    );
    validate_state_initializers(&verified_mir, program.states())?;
    let mir_executables = compiled_mir_executables(&verified_mir);
    attach_compiled_mir_identities(&mut code, &mir_executables);
    for code in code {
        program.insert_function(code);
    }
    for method in methods {
        let method_id = method.method_id();
        let function_id =
            vela_def::script_function_id(method.owner().package().as_str(), &method.symbol_seed());
        let target = input
            .script_method_target(method.node(), method_id, function_id)
            .ok_or_else(|| {
                CompileError::new(CompileErrorKind::RegistrySnapshot(format!(
                    "missing resolved executable target for script method `{}`",
                    method.name()
                )))
            })?;
        program.insert_script_method(
            target.owner,
            method.owner().target_type(),
            method.name(),
            method_id,
            target.function,
            method.symbol_seed(),
        );
    }
    program.set_script_metadata(graph.clone());
    let bytecode = verify_program(program)?;
    let observed = observed_capabilities(&verified_mir, &executable_packages)?;
    let package_metadata = package_request
        .map(|(roots, packages, providers)| {
            crate::PackageCompilationMetadata::new(roots, packages, providers, &observed)
        })
        .transpose()
        .map_err(|message| CompileError::new(CompileErrorKind::RegistrySnapshot(message)))?;
    Ok(CompiledProgram {
        mir_executables: compiled_bytecode_layouts(&bytecode),
        budget_layouts: compiled_budget_layouts(&bytecode),
        bytecode,
        verified_mir,
        binding_schema,
        package_metadata,
    })
}

fn executable_packages(
    graph: &ModuleGraph,
    function_symbols: &BTreeMap<HirDeclId, String>,
    state_symbols: &BTreeMap<HirDeclId, String>,
    methods: &[vela_hir::script_methods::ScriptMethod],
    service_impls: &vela_hir::service_impl::ServiceImplCatalog,
) -> CompileResult<BTreeMap<vela_def::FunctionId, PackageId>> {
    let mut packages = BTreeMap::new();
    for (declaration, symbol) in function_symbols {
        let metadata = graph.declaration(*declaration).ok_or_else(|| {
            CompileError::new(CompileErrorKind::RegistrySnapshot(
                "script function has no declaration metadata".to_owned(),
            ))
        })?;
        let package = graph.module_package(metadata.module).ok_or_else(|| {
            CompileError::new(CompileErrorKind::RegistrySnapshot(
                "script function has no package owner".to_owned(),
            ))
        })?;
        packages.insert(
            vela_def::script_function_id(package.as_str(), symbol),
            package.clone(),
        );
    }
    for (declaration, symbol) in state_symbols {
        if graph.state_initializer_body(*declaration).is_none() {
            continue;
        }
        let metadata = graph.declaration(*declaration).ok_or_else(|| {
            CompileError::new(CompileErrorKind::RegistrySnapshot(
                "script state has no declaration metadata".to_owned(),
            ))
        })?;
        let package = graph.module_package(metadata.module).ok_or_else(|| {
            CompileError::new(CompileErrorKind::RegistrySnapshot(
                "script state has no package owner".to_owned(),
            ))
        })?;
        packages.insert(
            vela_def::script_state_initializer_id(package.as_str(), symbol),
            package.clone(),
        );
    }
    for method in methods {
        packages.insert(
            vela_def::script_function_id(method.owner().package().as_str(), &method.symbol_seed()),
            method.owner().package().clone(),
        );
    }
    for implementation in service_impls.implementations() {
        let package = graph
            .module_package(implementation.module())
            .ok_or_else(|| {
                CompileError::new(CompileErrorKind::RegistrySnapshot(
                    "service implementation has no package owner".to_owned(),
                ))
            })?;
        for method in implementation.methods() {
            packages.insert(
                vela_def::script_function_id(
                    package.as_str(),
                    &implementation.method_symbol(method),
                ),
                package.clone(),
            );
        }
    }
    Ok(packages)
}

fn observed_capabilities(
    bundle: &vela_mir::OwnedVerifiedMirBundle,
    executable_packages: &BTreeMap<vela_def::FunctionId, PackageId>,
) -> CompileResult<BTreeMap<PackageId, CapabilitySet>> {
    let mut observed = BTreeMap::<PackageId, CapabilitySet>::new();
    for (root, owner) in bundle.roots() {
        let package = executable_packages.get(&root).ok_or_else(|| {
            CompileError::new(CompileErrorKind::RegistrySnapshot(format!(
                "verified MIR root {root:?} has no package owner"
            )))
        })?;
        let mut effect = vela_mir::MirEffect::PURE;
        for (_, function) in owner.program().functions() {
            for (_, statement) in function.statements() {
                effect = effect.union(statement.effect);
            }
            for (_, block) in function.blocks() {
                if let Some(terminator) = block.terminator() {
                    effect = effect.union(terminator.effect);
                }
            }
        }
        let capabilities = observed.entry(package.clone()).or_default();
        for (present, capability) in [
            (effect.host_read, Capability::HostRead),
            (effect.host_write, Capability::HostWrite),
            (effect.emits_event, Capability::EventEmit),
            (effect.reads_time, Capability::Time),
            (effect.uses_random, Capability::Random),
            (effect.reads_io, Capability::IoRead),
            (effect.writes_io, Capability::IoWrite),
            (effect.reflection_read, Capability::ReflectionRead),
            (effect.reflection_write, Capability::ReflectionWrite),
            (effect.reflection_call, Capability::ReflectionCall),
        ] {
            if present {
                capabilities.insert(capability);
            }
        }
    }
    Ok(observed)
}

fn validate_state_initializers(
    bundle: &vela_mir::OwnedVerifiedMirBundle,
    states: &[StateDescriptor],
) -> CompileResult<()> {
    let programs = bundle
        .roots()
        .map(|(function, owner)| (function, owner.as_ref()))
        .collect::<BTreeMap<_, _>>();
    for state in states
        .iter()
        .filter(|state| state.storage == StateStorage::Vm)
    {
        let Some(initializer) = state.initializer else {
            return Err(invalid_state_initializer(
                state,
                state.source_span,
                "the VM state has no compiled initializer",
            ));
        };
        let mut visiting = BTreeSet::new();
        let mut validated = BTreeSet::new();
        validate_initializer_root(state, initializer, &programs, &mut visiting, &mut validated)?;
    }
    Ok(())
}

fn validate_initializer_root(
    state: &StateDescriptor,
    function: vela_def::FunctionId,
    programs: &BTreeMap<vela_def::FunctionId, &vela_mir::OwnedVerifiedMirProgram>,
    visiting: &mut BTreeSet<vela_def::FunctionId>,
    validated: &mut BTreeSet<vela_def::FunctionId>,
) -> CompileResult<()> {
    if validated.contains(&function) || !visiting.insert(function) {
        return Ok(());
    }
    let owner = programs.get(&function).copied().ok_or_else(|| {
        invalid_state_initializer(
            state,
            state.source_span,
            "a called script function is missing from the verified program",
        )
    })?;
    let mut callees = Vec::new();
    for (_, body) in owner.program().functions() {
        for (_, statement) in body.statements() {
            reject_initializer_effect(state, statement.effect, statement.origin.span)?;
            if let vela_mir::MirStatementKind::Call(call) = &statement.kind {
                match call {
                    vela_mir::MirCall::ScriptFunction {
                        function,
                        signature,
                        ..
                    } if !signature.asyncness.is_async() => callees.push(*function),
                    vela_mir::MirCall::ScriptFunction { .. }
                    | vela_mir::MirCall::ScriptMethod { .. } => {
                        return Err(invalid_state_initializer(
                            state,
                            Some(statement.origin.span),
                            "async or method calls are not allowed",
                        ));
                    }
                    vela_mir::MirCall::ValueMethod {
                        owner, debug_name, ..
                    } if initializer_collection_method_allowed(*owner, debug_name) => {}
                    _ => {
                        return Err(invalid_state_initializer(
                            state,
                            Some(statement.origin.span),
                            "native, standard-library, host, provider, reflective, and dynamic calls are not allowed",
                        ));
                    }
                }
            }
        }
        for (_, block) in body.blocks() {
            let Some(terminator) = block.terminator() else {
                continue;
            };
            reject_initializer_effect(state, terminator.effect, terminator.origin.span)?;
            if matches!(
                terminator.kind,
                vela_mir::MirTerminatorKind::AwaitCall { .. }
            ) {
                return Err(invalid_state_initializer(
                    state,
                    Some(terminator.origin.span),
                    "await is not allowed",
                ));
            }
        }
    }
    for callee in callees {
        validate_initializer_root(state, callee, programs, visiting, validated)?;
    }
    visiting.remove(&function);
    validated.insert(function);
    Ok(())
}

fn initializer_collection_method_allowed(owner: vela_def::TypeId, debug_name: &str) -> bool {
    let method = debug_name.rsplit("::").next().unwrap_or(debug_name);
    [
        ("Array", &["push", "pop", "insert", "extend", "clear"][..]),
        ("Map", &["set", "remove", "extend", "clear"][..]),
        ("Set", &["add", "remove", "extend", "clear"][..]),
    ]
    .into_iter()
    .any(|(type_name, methods)| {
        vela_stdlib::std_type_id(type_name) == Some(owner) && methods.contains(&method)
    })
}

fn reject_initializer_effect(
    state: &StateDescriptor,
    effect: vela_mir::MirEffect,
    span: Span,
) -> CompileResult<()> {
    let reason = [
        (effect.dynamic_call, "dynamic dispatch"),
        (effect.state_read || effect.state_write, "state access"),
        (
            effect.host_read || effect.host_write || effect.host_call,
            "host access",
        ),
        (
            effect.reflection_read || effect.reflection_write || effect.reflection_call,
            "reflection",
        ),
        (effect.emits_event, "event emission"),
        (effect.reads_time, "time access"),
        (effect.uses_random, "random access"),
        (effect.reads_io || effect.writes_io, "IO access"),
    ]
    .into_iter()
    .find_map(|(present, reason)| present.then_some(reason));
    match reason {
        Some(reason) => Err(invalid_state_initializer(state, Some(span), reason)),
        None => Ok(()),
    }
}

fn invalid_state_initializer(
    state: &StateDescriptor,
    span: Option<Span>,
    reason: impl Into<String>,
) -> CompileError {
    let error = CompileError::new(CompileErrorKind::InvalidStateInitializer {
        state: state.qualified_name.clone(),
        reason: reason.into(),
    });
    span.map_or(error.clone(), |span| error.with_span(span))
}

fn validate_program_request(sources: &HirSourceSet) -> CompileResult<()> {
    match sources.kind() {
        HirSourceSetKind::ModuleGraph if sources.modules().is_empty() => Err(
            invalid_compilation_request(CompilationRequestError::EmptyModuleGraph),
        ),
        HirSourceSetKind::SingleSource | HirSourceSetKind::ModuleGraph => Ok(()),
    }
}

fn invalid_compilation_request(error: CompilationRequestError) -> CompileError {
    CompileError::new(CompileErrorKind::InvalidCompilationRequest(error))
}

fn reject_invalid_graph(graph: &ModuleGraph) -> CompileResult<()> {
    if graph.diagnostics().is_empty() {
        Ok(())
    } else {
        Err(CompileError::new(CompileErrorKind::InvalidHirGraph(
            graph.diagnostics().to_vec(),
        )))
    }
}

fn compiled_budget_layouts(program: &UnlinkedProgram) -> Box<[CompiledExecutableBudgetLayout]> {
    fn layout(code: &UnlinkedCodeObject) -> CompiledExecutableBudgetLayout {
        let sites = code
            .instructions
            .iter()
            .enumerate()
            .flat_map(|(offset, instruction)| {
                instruction
                    .mir_budget_charges
                    .iter()
                    .map(move |charge| ExecutableBudgetSite {
                        site: charge.site,
                        offset: crate::InstructionOffset(offset),
                        class: charge.class,
                        units: charge.units,
                        boundary: if matches!(charge.site, vela_mir::MirBudgetSite::Edge { .. }) {
                            ExecutableBudgetBoundary::EdgeStub
                        } else {
                            ExecutableBudgetBoundary::Operation
                        },
                    })
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        CompiledExecutableBudgetLayout { sites }
    }

    fn append(code: &UnlinkedCodeObject, layouts: &mut Vec<CompiledExecutableBudgetLayout>) {
        for nested in &code.nested_functions {
            append(nested, layouts);
            layouts.push(layout(nested));
        }
    }

    let mut layouts = program.functions().map(layout).collect::<Vec<_>>();
    for code in program.functions() {
        append(code, &mut layouts);
    }
    layouts.into_boxed_slice()
}

fn compiled_bytecode_layouts(program: &UnlinkedProgram) -> Box<[CompiledMirExecutable]> {
    fn append(code: &UnlinkedCodeObject, layouts: &mut Vec<CompiledMirExecutable>) {
        for nested in &code.nested_functions {
            append(nested, layouts);
            layouts.push(
                nested
                    .compiled_mir
                    .expect("production bytecode retains nested MIR identity"),
            );
        }
    }

    let mut layouts = program
        .functions()
        .map(|code| {
            code.compiled_mir
                .expect("production bytecode retains root MIR identity")
        })
        .collect::<Vec<_>>();
    for code in program.functions() {
        append(code, &mut layouts);
    }
    layouts.into_boxed_slice()
}

fn attach_compiled_mir_identities(
    roots: &mut [UnlinkedCodeObject],
    layouts: &[CompiledMirExecutable],
) {
    fn attach_nested(
        code: &mut UnlinkedCodeObject,
        layouts: &[CompiledMirExecutable],
        next: &mut usize,
    ) {
        for nested in &mut code.nested_functions {
            nested.compiled_mir = layouts.get(*next).copied();
            *next += 1;
            attach_nested(nested, layouts, next);
        }
    }

    for (code, layout) in roots.iter_mut().zip(layouts) {
        code.compiled_mir = Some(*layout);
    }
    let mut next = roots.len();
    for root in roots {
        attach_nested(root, layouts, &mut next);
    }
}

fn compiled_mir_executables(
    bundle: &vela_mir::OwnedVerifiedMirBundle,
) -> Box<[CompiledMirExecutable]> {
    fn append_children(
        layouts: &mut Vec<CompiledMirExecutable>,
        root: vela_def::FunctionId,
        program: &vela_mir::MirProgram,
        parent: vela_mir::MirFunctionId,
    ) {
        let children = program
            .functions()
            .filter_map(|(id, function)| {
                matches!(
                    function.owner(),
                    vela_mir::MirFunctionOwner::Lambda { parent: owner, .. } if *owner == parent
                )
                .then_some(id)
            })
            .collect::<Vec<_>>();
        for function in children {
            layouts.push(CompiledMirExecutable { root, function });
            append_children(layouts, root, program, function);
        }
    }

    let roots = bundle
        .roots()
        .map(|(root, owned)| {
            let function = owned
                .program()
                .functions()
                .find_map(|(id, function)| {
                    (!matches!(function.owner(), vela_mir::MirFunctionOwner::Lambda { .. }))
                        .then_some(id)
                })
                .expect("verified MIR root retains its root executable");
            (root, function, owned.as_ref())
        })
        .collect::<Vec<_>>();
    let mut layouts = roots
        .iter()
        .map(|(root, function, _)| CompiledMirExecutable {
            root: *root,
            function: *function,
        })
        .collect::<Vec<_>>();
    for (root, function, owned) in roots {
        append_children(&mut layouts, root, owned.program(), function);
    }
    layouts.into_boxed_slice()
}

fn compile_mir_roots(
    input: &semantic_input::PreparedSemanticInput,
    graph: &ModuleGraph,
) -> CompileResult<(
    Vec<UnlinkedCodeObject>,
    Arc<vela_mir::OwnedVerifiedMirBundle>,
)> {
    let programs = input
        .lowering_inputs(graph, vela_mir::MirLoweringConfig::default())?
        .into_iter()
        .map(|input| {
            let program = vela_mir::build_mir(input)
                .map_err(|error| CompileError::new(CompileErrorKind::MirInput(Box::new(error))))?;
            let verified = vela_mir::verify_owned_mir(program).map_err(|error| {
                CompileError::new(CompileErrorKind::MirVerification(Box::new(error)))
            })?;
            Ok(verified)
        })
        .collect::<CompileResult<Vec<_>>>()?;
    let bundle = Arc::new(vela_mir::OwnedVerifiedMirBundle::new(programs));
    let code = bundle
        .roots()
        .map(|(_, verified)| {
            let handoff = verified
                .backend_handoff()
                .map_err(|error| CompileError::new(CompileErrorKind::MirBackendHandoff(error)))?;
            let code = mir_backend::compile(handoff)
                .map_err(|error| mir_backend_compile_error(verified, error))?;
            verify_code_object(code)
        })
        .collect::<CompileResult<Vec<_>>>()?;
    Ok((code, bundle))
}

fn mir_backend_compile_error(
    verified: &vela_mir::OwnedVerifiedMirProgram,
    error: mir_backend::MirBackendError,
) -> CompileError {
    let (root, root_body) = verified
        .program()
        .functions()
        .next()
        .expect("verified MIR backend input has a root function");
    if matches!(error, mir_backend::MirBackendError::RegisterOverflow) {
        return CompileError::new(CompileErrorKind::RegisterOverflow)
            .with_span(root_body.origin().span);
    }
    let function = match error {
        mir_backend::MirBackendError::MissingMirFunction(function) => function,
        _ => root,
    };
    let origin = verified
        .program()
        .function(function)
        .map_or(root_body.origin(), vela_mir::MirFunction::origin);
    let kind = match error {
        mir_backend::MirBackendError::MissingRoot => error::MirBackendFailureKind::MissingRoot,
        mir_backend::MirBackendError::MissingMirFunction(function) => {
            error::MirBackendFailureKind::MissingFunction(function)
        }
        mir_backend::MirBackendError::MissingBlock(block) => {
            error::MirBackendFailureKind::MissingBlock(block)
        }
        mir_backend::MirBackendError::MissingStatement => {
            error::MirBackendFailureKind::MissingStatement
        }
        mir_backend::MirBackendError::MissingDestination => {
            error::MirBackendFailureKind::MissingDestination
        }
        mir_backend::MirBackendError::MissingTarget(target) => {
            error::MirBackendFailureKind::MissingTarget(target)
        }
        mir_backend::MirBackendError::DynamicHostArgumentOverflow => {
            error::MirBackendFailureKind::DynamicHostArgumentOverflow
        }
        mir_backend::MirBackendError::RegisterOverflow => unreachable!("handled above"),
    };
    CompileError::new(CompileErrorKind::MirBackend(Box::new(
        error::MirBackendFailure {
            function,
            origin,
            kind,
        },
    )))
    .with_span(origin.span)
}

fn state_descriptors(
    graph: &ModuleGraph,
    input: &semantic_input::PreparedSemanticInput,
    state_symbols: &BTreeMap<HirDeclId, String>,
) -> CompileResult<Vec<StateDescriptor>> {
    let mut descriptors = state_symbols
        .iter()
        .map(|(declaration, symbol)| {
            let metadata = graph.declaration(*declaration).ok_or_else(|| {
                CompileError::new(CompileErrorKind::RegistrySnapshot(format!(
                    "missing HIR declaration for state `{symbol}`"
                )))
            })?;
            let target = input.targets().state(*declaration).ok_or_else(|| {
                CompileError::new(CompileErrorKind::RegistrySnapshot(format!(
                    "missing state target for `{symbol}`"
                )))
            })?;
            Ok(StateDescriptor {
                id: target.id,
                qualified_name: symbol.clone(),
                visibility: match metadata.visibility {
                    Visibility::Private => StateVisibility::Private,
                    Visibility::Public => StateVisibility::Public,
                },
                storage: match target.storage {
                    CompileStateStorage::Vm => StateStorage::Vm,
                    CompileStateStorage::Extern => StateStorage::Extern,
                },
                type_contract: target.contract.clone(),
                initializer: target.initializer,
                source_span: Some(metadata.span),
            })
        })
        .collect::<CompileResult<Vec<_>>>()?;
    descriptors.sort_by(|left, right| left.qualified_name.cmp(&right.qualified_name));
    Ok(descriptors)
}

fn nominal_type_descriptors(targets: &MirTargetTable) -> CompileResult<Vec<NominalTypeDescriptor>> {
    targets
        .types()
        .filter(|(_, ty)| {
            matches!(
                ty.class,
                CompileTypeClass::ScriptRecord | CompileTypeClass::ScriptEnum
            ) || matches!(ty.class, CompileTypeClass::Registry)
                && (ty.shape.is_some() || !ty.variants.is_empty())
        })
        .map(|(_, ty)| {
            let fields = ty
                .fields
                .iter()
                .map(|field| nominal_field_descriptor(targets, *field))
                .collect::<CompileResult<Vec<_>>>()?;
            let variants = ty
                .variants
                .iter()
                .map(|variant| {
                    let descriptor = targets.variant(*variant).ok_or_else(|| {
                        missing_nominal_metadata(format!(
                            "missing variant target {variant:?} for `{}`",
                            ty.runtime_name
                        ))
                    })?;
                    let fields = descriptor
                        .fields
                        .iter()
                        .map(|field| nominal_field_descriptor(targets, *field))
                        .collect::<CompileResult<Vec<_>>>()?;
                    Ok(NominalVariantDescriptor {
                        id: descriptor.id,
                        name: descriptor.name.clone(),
                        fields,
                    })
                })
                .collect::<CompileResult<Vec<_>>>()?;
            Ok(NominalTypeDescriptor {
                id: ty.id,
                canonical_name: ty.canonical_name.clone(),
                runtime_name: ty.runtime_name.clone(),
                kind: match ty.class {
                    CompileTypeClass::ScriptRecord => NominalTypeKind::Record,
                    CompileTypeClass::ScriptEnum => NominalTypeKind::Enum,
                    CompileTypeClass::Registry if ty.shape.is_some() => NominalTypeKind::Record,
                    CompileTypeClass::Registry if !ty.variants.is_empty() => NominalTypeKind::Enum,
                    _ => unreachable!("filtered to script nominal types"),
                },
                shape: ty.shape,
                fields,
                variants,
            })
        })
        .collect()
}

fn nominal_field_descriptor(
    targets: &MirTargetTable,
    field: vela_def::FieldId,
) -> CompileResult<NominalFieldDescriptor> {
    let descriptor = targets.field(field).ok_or_else(|| {
        missing_nominal_metadata(format!("missing field target {field:?} for nominal type"))
    })?;
    Ok(NominalFieldDescriptor {
        id: descriptor.id,
        name: descriptor.name.clone(),
        contract: descriptor.contract.clone(),
    })
}

fn missing_nominal_metadata(message: String) -> CompileError {
    CompileError::new(CompileErrorKind::RegistrySnapshot(message))
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

#[cfg(test)]
mod tests;
