//! Heavy-HIR -> verified MIR -> bytecode production compiler.

mod cache_sites;
mod const_eval;
mod constant_encoding;
pub mod error;
pub(crate) mod mir_backend;
pub mod options;
mod schema_defaults;
mod semantic;
#[path = "semantic_input/mod.rs"]
mod semantic_input;

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Deref;
use std::sync::Arc;

use vela_common::SourceId;
#[cfg(test)]
use vela_common::Span;
use vela_hir::ids::HirDeclId;
#[cfg(test)]
use vela_hir::module_graph::ModulePath;
use vela_hir::module_graph::{ModuleGraph, ModuleSource};
use vela_hir::script_methods::ScriptMethodCatalog;
use vela_mir::MirEvaluatedConstant;
use vela_registry::RegistryCompileView;

#[cfg(test)]
use crate::{Constant, UnlinkedTypeGuardPlan};
use crate::{UnlinkedCodeObject, UnlinkedProgram};
use error::{CompileError, CompileErrorKind, CompileResult};
use options::CompilerOptions;
use semantic::{parse_semantic_modules, parse_semantic_source};

#[derive(Clone, Debug)]
pub struct CompiledProgram {
    bytecode: UnlinkedProgram,
    verified_mir: Arc<vela_mir::OwnedVerifiedMirBundle>,
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
    pub fn into_parts(self) -> (UnlinkedProgram, Arc<vela_mir::OwnedVerifiedMirBundle>) {
        (self.bytecode, self.verified_mir)
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

fn compile_function_source_inner(
    source: SourceId,
    text: &str,
    function_name: &str,
    options: &CompilerOptions,
    registry: Option<RegistryCompileView<'_>>,
) -> CompileResult<UnlinkedCodeObject> {
    let semantic = parse_semantic_source(source, text)?;
    let declaration = semantic
        .function_declaration(function_name)
        .ok_or_else(|| {
            CompileError::new(CompileErrorKind::FunctionNotFound(function_name.to_owned()))
        })?;
    let script_function_symbols = semantic.script_function_symbols();
    let type_symbols = semantic.type_symbols();
    let global_symbols = semantic.global_symbols();
    let evaluated_constants = semantic.evaluated_constants()?;
    let schema_defaults = semantic.schema_defaults(&type_symbols, &evaluated_constants)?;
    let input = semantic_input::prepare_semantic_input(semantic_input::SemanticInputRequest {
        graph: semantic.script_metadata_graph(),
        roots: semantic_input::SemanticRoots::Function(declaration),
        script_function_symbols: &script_function_symbols,
        script_methods: semantic.script_method_catalog(),
        type_symbols: &type_symbols,
        global_symbols: &global_symbols,
        evaluated_constants: &evaluated_constants,
        schema_defaults: &schema_defaults,
        options,
        registry,
    })?;
    let (mut code, _) = compile_mir_roots(&input, semantic.script_metadata_graph())?;
    match code.len() {
        1 => Ok(code.pop().expect("one checked MIR root")),
        count => Err(CompileError::new(CompileErrorKind::InvalidMirRootCount {
            count,
        })),
    }
}

pub fn compile_program_source(source: SourceId, text: &str) -> CompileResult<CompiledProgram> {
    compile_program_source_with_options(source, text, &CompilerOptions::default())
}

pub fn compile_program_source_with_registry(
    source: SourceId,
    text: &str,
    registry: RegistryCompileView<'_>,
) -> CompileResult<CompiledProgram> {
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
) -> CompileResult<CompiledProgram> {
    compile_program_source_inner(source, text, options, None)
}

pub fn compile_program_source_with_options_and_registry(
    source: SourceId,
    text: &str,
    options: &CompilerOptions,
    registry: RegistryCompileView<'_>,
) -> CompileResult<CompiledProgram> {
    compile_program_source_inner(source, text, options, Some(registry))
}

fn compile_program_source_inner(
    source: SourceId,
    text: &str,
    options: &CompilerOptions,
    registry: Option<RegistryCompileView<'_>>,
) -> CompileResult<CompiledProgram> {
    let semantic = parse_semantic_source(source, text)?;
    compile_semantic_program(&semantic, options, registry)
}

pub fn compile_module_sources(sources: &[ModuleSource]) -> CompileResult<CompiledProgram> {
    compile_module_sources_with_options(sources, &CompilerOptions::default())
}

pub fn compile_module_sources_with_registry(
    sources: &[ModuleSource],
    registry: RegistryCompileView<'_>,
) -> CompileResult<CompiledProgram> {
    compile_module_sources_with_options_and_registry(sources, &CompilerOptions::default(), registry)
}

pub fn compile_module_sources_with_options(
    sources: &[ModuleSource],
    options: &CompilerOptions,
) -> CompileResult<CompiledProgram> {
    compile_module_sources_inner(sources, options, None)
}

pub fn compile_module_sources_with_options_and_registry(
    sources: &[ModuleSource],
    options: &CompilerOptions,
    registry: RegistryCompileView<'_>,
) -> CompileResult<CompiledProgram> {
    compile_module_sources_inner(sources, options, Some(registry))
}

fn compile_module_sources_inner(
    sources: &[ModuleSource],
    options: &CompilerOptions,
    registry: Option<RegistryCompileView<'_>>,
) -> CompileResult<CompiledProgram> {
    let semantic = parse_semantic_modules(sources)?;
    compile_semantic_program(&semantic, options, registry)
}

trait SemanticProgram {
    fn graph(&self) -> &ModuleGraph;
    fn function_symbols(&self) -> BTreeMap<HirDeclId, String>;
    fn type_symbols(&self) -> BTreeMap<HirDeclId, String>;
    fn global_symbols(&self) -> BTreeMap<HirDeclId, String>;
    fn evaluated_constants(&self) -> CompileResult<BTreeMap<HirDeclId, MirEvaluatedConstant>>;
    fn schema_defaults(
        &self,
        types: &BTreeMap<HirDeclId, String>,
        constants: &BTreeMap<HirDeclId, MirEvaluatedConstant>,
    ) -> CompileResult<schema_defaults::EvaluatedSchemaDefaults>;
    fn methods(&self) -> &ScriptMethodCatalog;
}

impl SemanticProgram for semantic::SemanticSource {
    fn graph(&self) -> &ModuleGraph {
        self.script_metadata_graph()
    }
    fn function_symbols(&self) -> BTreeMap<HirDeclId, String> {
        self.script_function_symbols()
    }
    fn type_symbols(&self) -> BTreeMap<HirDeclId, String> {
        self.type_symbols()
    }
    fn global_symbols(&self) -> BTreeMap<HirDeclId, String> {
        self.global_symbols()
    }
    fn evaluated_constants(&self) -> CompileResult<BTreeMap<HirDeclId, MirEvaluatedConstant>> {
        self.evaluated_constants()
    }
    fn schema_defaults(
        &self,
        types: &BTreeMap<HirDeclId, String>,
        constants: &BTreeMap<HirDeclId, MirEvaluatedConstant>,
    ) -> CompileResult<schema_defaults::EvaluatedSchemaDefaults> {
        self.schema_defaults(types, constants)
    }
    fn methods(&self) -> &ScriptMethodCatalog {
        self.script_method_catalog()
    }
}

impl SemanticProgram for semantic::SemanticModules {
    fn graph(&self) -> &ModuleGraph {
        self.script_metadata_graph()
    }
    fn function_symbols(&self) -> BTreeMap<HirDeclId, String> {
        self.script_function_symbols()
    }
    fn type_symbols(&self) -> BTreeMap<HirDeclId, String> {
        self.type_symbols()
    }
    fn global_symbols(&self) -> BTreeMap<HirDeclId, String> {
        self.global_symbols()
    }
    fn evaluated_constants(&self) -> CompileResult<BTreeMap<HirDeclId, MirEvaluatedConstant>> {
        self.evaluated_constants()
    }
    fn schema_defaults(
        &self,
        types: &BTreeMap<HirDeclId, String>,
        constants: &BTreeMap<HirDeclId, MirEvaluatedConstant>,
    ) -> CompileResult<schema_defaults::EvaluatedSchemaDefaults> {
        self.schema_defaults(types, constants)
    }
    fn methods(&self) -> &ScriptMethodCatalog {
        self.script_method_catalog()
    }
}

fn compile_semantic_program(
    semantic: &impl SemanticProgram,
    options: &CompilerOptions,
    registry: Option<RegistryCompileView<'_>>,
) -> CompileResult<CompiledProgram> {
    let script_function_symbols = semantic.function_symbols();
    let type_symbols = semantic.type_symbols();
    let global_symbols = semantic.global_symbols();
    let evaluated_constants = semantic.evaluated_constants()?;
    let schema_defaults = semantic.schema_defaults(&type_symbols, &evaluated_constants)?;
    let methods = semantic
        .methods()
        .methods()
        .map(|method| {
            (
                method.owner().target_type().to_owned(),
                method.name().to_owned(),
                method.method_id(),
                method.symbol_seed(),
            )
        })
        .collect::<Vec<_>>();
    let input = semantic_input::prepare_semantic_input(semantic_input::SemanticInputRequest {
        graph: semantic.graph(),
        roots: semantic_input::SemanticRoots::Program,
        script_function_symbols: &script_function_symbols,
        script_methods: semantic.methods(),
        type_symbols: &type_symbols,
        global_symbols: &global_symbols,
        evaluated_constants: &evaluated_constants,
        schema_defaults: &schema_defaults,
        options,
        registry,
    })?;

    let mut program = UnlinkedProgram::new();
    program.set_global_layout(global_names(&global_symbols));
    let (code, verified_mir) = compile_mir_roots(&input, semantic.graph())?;
    for code in code {
        program.insert_function(code);
    }
    for (owner, name, method, symbol) in methods {
        program.insert_script_method(owner, name, method, symbol);
    }
    program.set_script_metadata(semantic.graph().clone());
    Ok(CompiledProgram {
        bytecode: verify_program(program)?,
        verified_mir,
    })
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

fn global_names(global_symbols: &BTreeMap<HirDeclId, String>) -> Vec<String> {
    global_symbols
        .values()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
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
