use std::path::Path;

use vela_bytecode::compiler::error::{CompileError, CompileErrorKind};
use vela_common::diagnostic_render::{DiagnosticRenderer, DiagnosticSource};
use vela_common::{Diagnostic, SourceId};
use vela_engine::reload::{EngineHotReloadSourceError, EngineHotReloadSourceErrorKind};
use vela_engine::source::{EngineSourceError, EngineSourceErrorKind};
use vela_hot_reload::error::{HotReloadError, HotReloadErrorKind};
use vela_hot_reload::report::HotReloadReport;
use vela_vm::error::VmError;

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

pub(crate) fn render_hot_reload_report(path: &Path, report: &HotReloadReport) -> String {
    let source = std::fs::read_to_string(path)
        .ok()
        .map(|text| DiagnosticSource::new(SourceId::new(1), path.display().to_string(), text));
    let mut lines = report
        .render_lines()
        .into_iter()
        .map(|line| line.text)
        .collect::<Vec<_>>();
    let source_diagnostics = report_source_diagnostics(report);
    if !source_diagnostics.is_empty() {
        lines.push(String::new());
        lines.push(render_diagnostics(&source_diagnostics, source));
    }
    lines.join("\n")
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
        | CompileErrorKind::BytecodeVerification(_)
        | CompileErrorKind::UnsupportedSyntax(_) => None,
    }
}

fn report_source_diagnostics(report: &HotReloadReport) -> Vec<Diagnostic> {
    report
        .errors
        .iter()
        .filter_map(|error| {
            let span = error.source_span?;
            Some(
                Diagnostic::error(error.reason.clone())
                    .with_code(error.code)
                    .with_span(span),
            )
        })
        .collect()
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
