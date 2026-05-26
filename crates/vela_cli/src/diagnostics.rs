use std::path::Path;

use vela_bytecode::compiler::{CompileError, CompileErrorKind};
use vela_common::{Diagnostic, DiagnosticRenderer, DiagnosticSource, SourceId};
use vela_engine::{
    EngineHotReloadSourceError, EngineHotReloadSourceErrorKind, EngineSourceError,
    EngineSourceErrorKind,
};
use vela_hot_reload::{HotReloadError, HotReloadErrorKind};
use vela_vm::VmError;

pub(crate) fn render_engine_source_error(path: &Path, error: &EngineSourceError) -> String {
    match &error.kind {
        EngineSourceErrorKind::Compile(error) => render_compile_error(path, error),
        EngineSourceErrorKind::Io { .. }
        | EngineSourceErrorKind::InvalidSourcePath { .. }
        | EngineSourceErrorKind::TooManySources { .. } => error.to_string(),
    }
}

pub(crate) fn render_hot_reload_source_error(
    path: &Path,
    error: &EngineHotReloadSourceError,
) -> String {
    match &error.kind {
        EngineHotReloadSourceErrorKind::Source(error) => render_engine_source_error(path, error),
        EngineHotReloadSourceErrorKind::HotReload(error) => render_hot_reload_error(path, error),
    }
}

pub(crate) fn render_vm_error(path: &Path, error: &VmError) -> String {
    let source = std::fs::read_to_string(path)
        .ok()
        .map(|text| DiagnosticSource::new(SourceId::new(1), path.display().to_string(), text));
    render_diagnostics(&[error.to_diagnostic()], source)
}

fn render_hot_reload_error(path: &Path, error: &HotReloadError) -> String {
    match &error.kind {
        HotReloadErrorKind::Compile(error) => render_compile_error(path, error),
        _ => format!("{error:?}"),
    }
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
