use std::path::Path;

use vela_bytecode::compiler::error::{CompileError, CompileErrorKind};
use vela_common::diagnostic_render::{DiagnosticRenderer, DiagnosticSource};
use vela_common::{Diagnostic, SourceId};
use vela_engine::source::{EngineSourceError, EngineSourceErrorKind};
use vela_vm::error::VmError;

pub(crate) fn render_engine_source_error(path: &Path, error: &EngineSourceError) -> String {
    match &error.kind {
        EngineSourceErrorKind::Frontend(error) => {
            render_source_diagnostics(path, error.diagnostics())
        }
        EngineSourceErrorKind::Backend(error) => render_compile_error(path, error),
        EngineSourceErrorKind::Io { .. }
        | EngineSourceErrorKind::InvalidSourcePath { .. }
        | EngineSourceErrorKind::TooManySources { .. } => error.to_string(),
    }
}

pub(crate) fn render_vm_error(path: &Path, error: &VmError) -> String {
    let source = std::fs::read_to_string(path)
        .ok()
        .map(|text| DiagnosticSource::new(SourceId::new(1), path.display().to_string(), text));
    render_diagnostics(&[error.to_diagnostic()], source)
}

fn render_compile_error(path: &Path, error: &CompileError) -> String {
    let diagnostics = compile_diagnostics(error);
    if diagnostics.is_empty() {
        return "compilation failed without a projected diagnostic".to_owned();
    }

    let source = std::fs::read_to_string(path)
        .ok()
        .map(|text| DiagnosticSource::new(SourceId::new(1), path.display().to_string(), text));
    render_diagnostics(&diagnostics, source)
}

fn render_source_diagnostics(path: &Path, diagnostics: &[Diagnostic]) -> String {
    let source = std::fs::read_to_string(path)
        .ok()
        .map(|text| DiagnosticSource::new(SourceId::new(1), path.display().to_string(), text));
    render_diagnostics(diagnostics, source)
}

fn compile_diagnostics(error: &CompileError) -> Vec<Diagnostic> {
    match &error.kind {
        CompileErrorKind::InvalidHirGraph(diagnostics)
        | CompileErrorKind::SemanticDiagnostics(diagnostics) => diagnostics.clone(),
        CompileErrorKind::UnknownLocal(_) | CompileErrorKind::UnsupportedSyntax(_) => Vec::new(),
        _ => error.to_diagnostic().into_iter().collect(),
    }
}

fn render_diagnostics(diagnostics: &[Diagnostic], source: Option<DiagnosticSource>) -> String {
    let renderer = DiagnosticRenderer::new(source);
    diagnostics
        .iter()
        .enumerate()
        .flat_map(|(index, diagnostic)| {
            let mut lines = Vec::new();
            if index > 0 {
                lines.push(String::new());
            }
            lines.extend(renderer.render(diagnostic));
            lines
        })
        .collect::<Vec<_>>()
        .join("\n")
}
