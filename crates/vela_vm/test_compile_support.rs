use vela_bytecode::UnlinkedCodeObject;
use vela_bytecode::compiler::error::{CompileError, CompileErrorKind, CompileResult};
use vela_bytecode::compiler::options::CompilerOptions;
use vela_bytecode::compiler::{
    CompiledProgram, FunctionCompilationRequest, ProgramCompilationRequest, compile_function,
    compile_program,
};
use vela_common::{Diagnostic, SourceId};
use vela_hir::module_graph::{ModulePath, ModuleSource};
use vela_hir::source_ingestion::{
    HirSourceBuildError, build_module_source_set, build_single_source,
};
use vela_registry::RegistryCompileView;

pub(crate) fn compile_test_function(
    source: SourceId,
    text: &str,
    function_name: &str,
) -> CompileResult<UnlinkedCodeObject> {
    compile_test_function_inner(
        source,
        text,
        function_name,
        &CompilerOptions::default(),
        None,
    )
}

pub(crate) fn compile_test_function_with_registry(
    source: SourceId,
    text: &str,
    function_name: &str,
    registry: RegistryCompileView<'_>,
) -> CompileResult<UnlinkedCodeObject> {
    compile_test_function_inner(
        source,
        text,
        function_name,
        &CompilerOptions::default(),
        Some(registry),
    )
}

fn compile_test_function_inner(
    source: SourceId,
    text: &str,
    function_name: &str,
    options: &CompilerOptions,
    registry: Option<RegistryCompileView<'_>>,
) -> CompileResult<UnlinkedCodeObject> {
    let built = build_single_source(source, text).map_err(frontend_error)?;
    let function = built
        .function(&ModulePath::root(), function_name)
        .ok_or_else(|| function_not_found(function_name))?;
    compile_function(FunctionCompilationRequest {
        function,
        options,
        registry,
    })
}

pub(crate) fn compile_test_program(source: SourceId, text: &str) -> CompileResult<CompiledProgram> {
    compile_test_program_inner(source, text, &CompilerOptions::default(), None)
}

pub(crate) fn compile_test_program_with_registry(
    source: SourceId,
    text: &str,
    registry: RegistryCompileView<'_>,
) -> CompileResult<CompiledProgram> {
    compile_test_program_inner(source, text, &CompilerOptions::default(), Some(registry))
}

pub(crate) fn compile_test_program_with_options_and_registry(
    source: SourceId,
    text: &str,
    options: &CompilerOptions,
    registry: RegistryCompileView<'_>,
) -> CompileResult<CompiledProgram> {
    compile_test_program_inner(source, text, options, Some(registry))
}

fn compile_test_program_inner(
    source: SourceId,
    text: &str,
    options: &CompilerOptions,
    registry: Option<RegistryCompileView<'_>>,
) -> CompileResult<CompiledProgram> {
    let built = build_single_source(source, text).map_err(frontend_error)?;
    compile_program(ProgramCompilationRequest {
        sources: &built,
        options,
        registry,
    })
}

pub(crate) fn compile_test_modules(sources: &[ModuleSource]) -> CompileResult<CompiledProgram> {
    compile_test_modules_inner(sources, None)
}

pub(crate) fn compile_test_modules_with_registry(
    sources: &[ModuleSource],
    registry: RegistryCompileView<'_>,
) -> CompileResult<CompiledProgram> {
    compile_test_modules_inner(sources, Some(registry))
}

fn compile_test_modules_inner(
    sources: &[ModuleSource],
    registry: Option<RegistryCompileView<'_>>,
) -> CompileResult<CompiledProgram> {
    let built = build_module_source_set(sources).map_err(frontend_error)?;
    compile_program(ProgramCompilationRequest {
        sources: &built,
        options: &CompilerOptions::default(),
        registry,
    })
}

fn frontend_error(error: HirSourceBuildError) -> CompileError {
    CompileError {
        kind: CompileErrorKind::InvalidHirGraph(error.into_diagnostics()),
        span: None,
    }
}

fn function_not_found(name: &str) -> CompileError {
    CompileError {
        kind: CompileErrorKind::InvalidHirGraph(vec![Diagnostic::error(format!(
            "function `{name}` was not found in the test source"
        ))]),
        span: None,
    }
}
