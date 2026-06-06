use std::path::Path;

use vela_bytecode::compiler::error::{CompileError, CompileErrorKind};
use vela_common::diagnostic_render::{DiagnosticRenderer, DiagnosticSource};
use vela_common::{Diagnostic, SourceId};
use vela_engine::source::{EngineSourceError, EngineSourceErrorKind};
use vela_vm::error::VmError;

pub(crate) fn render_engine_source_error(path: &Path, error: &EngineSourceError) -> String {
    match &error.kind {
        EngineSourceErrorKind::Compile(error) => render_compile_error(path, error),
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
    let Some(diagnostics) = compile_diagnostics(error) else {
        return format!("{error:?}");
    };

    let source = std::fs::read_to_string(path)
        .ok()
        .map(|text| DiagnosticSource::new(SourceId::new(1), path.display().to_string(), text));
    render_diagnostics(diagnostics, source)
}

fn compile_diagnostics(error: &CompileError) -> Option<&[Diagnostic]> {
    match &error.kind {
        CompileErrorKind::SyntaxDiagnostics(diagnostics)
        | CompileErrorKind::SemanticDiagnostics(diagnostics) => Some(diagnostics),
        CompileErrorKind::FunctionNotFound(_)
        | CompileErrorKind::UnknownLocal(_)
        | CompileErrorKind::InvalidIntLiteral { .. }
        | CompileErrorKind::InvalidFloatLiteral { .. }
        | CompileErrorKind::RegisterOverflow
        | CompileErrorKind::BytecodeVerification(_)
        | CompileErrorKind::UnsupportedSyntax(_) => None,
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
