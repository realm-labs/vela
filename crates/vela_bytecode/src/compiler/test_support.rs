use vela_common::{Diagnostic, SourceId, Span};
use vela_hir::module_graph::{DeclarationKind, ModulePath, ModuleSource};
use vela_hir::source_ingestion::HirSourceBuildErrorKind;
use vela_hir::source_ingestion::{HirSourceBuildError, build_source_set};

use super::*;

#[derive(Debug)]
pub(crate) struct TestCompileError {
    pub(crate) kind: CompileErrorKind,
    pub(crate) span: Option<Span>,
    frontend: Option<HirSourceBuildError>,
}

impl TestCompileError {
    fn from_frontend(error: HirSourceBuildError) -> Self {
        Self {
            kind: CompileErrorKind::InvalidHirGraph(error.diagnostics().to_vec()),
            span: None,
            frontend: Some(error),
        }
    }

    fn function_not_found(name: &str) -> Self {
        Self {
            kind: CompileErrorKind::InvalidHirGraph(vec![Diagnostic::error(format!(
                "function `{name}` was not found in the test source"
            ))]),
            span: None,
            frontend: None,
        }
    }

    pub(crate) fn to_diagnostic(&self) -> Option<Diagnostic> {
        CompileError {
            kind: self.kind.clone(),
            span: self.span,
        }
        .to_diagnostic()
    }

    pub(crate) fn into_syntax_diagnostics(self) -> Vec<Diagnostic> {
        let error = self.frontend.expect("expected front-end diagnostics");
        assert_eq!(error.kind(), HirSourceBuildErrorKind::Syntax);
        error.into_diagnostics()
    }

    pub(crate) fn into_semantic_diagnostics(self) -> Vec<Diagnostic> {
        if let Some(error) = self.frontend {
            assert_eq!(error.kind(), HirSourceBuildErrorKind::Semantic);
            return error.into_diagnostics();
        }
        let CompileErrorKind::SemanticDiagnostics(diagnostics) = self.kind else {
            panic!("expected semantic diagnostics, got {:?}", self.kind);
        };
        diagnostics
    }
}

impl From<CompileError> for TestCompileError {
    fn from(error: CompileError) -> Self {
        Self {
            kind: error.kind,
            span: error.span,
            frontend: None,
        }
    }
}

pub(crate) fn compile_test_function(
    source: SourceId,
    text: &str,
    function_name: &str,
) -> Result<UnlinkedCodeObject, TestCompileError> {
    compile_test_function_with_options(source, text, function_name, &CompilerOptions::default())
}

pub(crate) fn compile_test_function_with_registry(
    source: SourceId,
    text: &str,
    function_name: &str,
    registry: RegistryCompileView<'_>,
) -> Result<UnlinkedCodeObject, TestCompileError> {
    compile_test_function_inner(
        source,
        text,
        function_name,
        &CompilerOptions::default(),
        Some(registry),
    )
}

pub(crate) fn compile_test_function_with_options(
    source: SourceId,
    text: &str,
    function_name: &str,
    options: &CompilerOptions,
) -> Result<UnlinkedCodeObject, TestCompileError> {
    compile_test_function_inner(source, text, function_name, options, None)
}

fn compile_test_function_inner(
    source: SourceId,
    text: &str,
    function_name: &str,
    options: &CompilerOptions,
    registry: Option<RegistryCompileView<'_>>,
) -> Result<UnlinkedCodeObject, TestCompileError> {
    let sources = [single_source(source, text)];
    let built = build_source_set(&sources).map_err(TestCompileError::from_frontend)?;
    let module = built.modules()[0];
    let function = built
        .graph()
        .module(module)
        .and_then(|declarations| declarations.get(function_name))
        .filter(|declaration| {
            built
                .graph()
                .declaration(*declaration)
                .map(|metadata| metadata.kind)
                == Some(DeclarationKind::Function)
        })
        .ok_or_else(|| TestCompileError::function_not_found(function_name))?;
    compile_function(FunctionCompilationRequest {
        graph: built.graph(),
        module,
        function,
        options,
        registry,
    })
    .map_err(TestCompileError::from)
}

pub(crate) fn compile_test_program(
    source: SourceId,
    text: &str,
) -> Result<CompiledProgram, TestCompileError> {
    compile_test_program_with_options(source, text, &CompilerOptions::default())
}

pub(crate) fn compile_test_program_with_registry(
    source: SourceId,
    text: &str,
    registry: RegistryCompileView<'_>,
) -> Result<CompiledProgram, TestCompileError> {
    compile_test_program_inner(source, text, &CompilerOptions::default(), Some(registry))
}

pub(crate) fn compile_test_program_with_options(
    source: SourceId,
    text: &str,
    options: &CompilerOptions,
) -> Result<CompiledProgram, TestCompileError> {
    compile_test_program_inner(source, text, options, None)
}

pub(crate) fn compile_test_program_with_options_and_registry(
    source: SourceId,
    text: &str,
    options: &CompilerOptions,
    registry: RegistryCompileView<'_>,
) -> Result<CompiledProgram, TestCompileError> {
    compile_test_program_inner(source, text, options, Some(registry))
}

fn compile_test_program_inner(
    source: SourceId,
    text: &str,
    options: &CompilerOptions,
    registry: Option<RegistryCompileView<'_>>,
) -> Result<CompiledProgram, TestCompileError> {
    let sources = [single_source(source, text)];
    let built = build_source_set(&sources).map_err(TestCompileError::from_frontend)?;
    let mode = ProgramCompilationMode::SingleSource {
        root: built.modules()[0],
    };
    compile_program(ProgramCompilationRequest {
        graph: built.graph(),
        mode: &mode,
        options,
        registry,
    })
    .map_err(TestCompileError::from)
}

pub(crate) fn compile_test_modules(
    sources: &[ModuleSource],
) -> Result<CompiledProgram, TestCompileError> {
    compile_test_modules_with_options(sources, &CompilerOptions::default())
}

pub(crate) fn compile_test_modules_with_registry(
    sources: &[ModuleSource],
    registry: RegistryCompileView<'_>,
) -> Result<CompiledProgram, TestCompileError> {
    compile_test_modules_inner(sources, &CompilerOptions::default(), Some(registry))
}

pub(crate) fn compile_test_modules_with_options(
    sources: &[ModuleSource],
    options: &CompilerOptions,
) -> Result<CompiledProgram, TestCompileError> {
    compile_test_modules_inner(sources, options, None)
}

fn compile_test_modules_inner(
    sources: &[ModuleSource],
    options: &CompilerOptions,
    registry: Option<RegistryCompileView<'_>>,
) -> Result<CompiledProgram, TestCompileError> {
    let built = build_source_set(sources).map_err(TestCompileError::from_frontend)?;
    let mode = ProgramCompilationMode::ModuleGraph {
        modules: built.modules().into(),
    };
    compile_program(ProgramCompilationRequest {
        graph: built.graph(),
        mode: &mode,
        options,
        registry,
    })
    .map_err(TestCompileError::from)
}

fn single_source(source: SourceId, text: &str) -> ModuleSource {
    ModuleSource::new(source, ModulePath::new(Vec::<String>::new()), text)
}
