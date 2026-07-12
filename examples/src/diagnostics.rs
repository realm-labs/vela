use vela_bytecode::compiler::error::{CompileError, CompileErrorKind};
use vela_common::diagnostic_render::{DiagnosticRenderer, DiagnosticSource};
use vela_common::{Diagnostic, SourceId};
use vela_engine::source::{EngineSourceError, EngineSourceErrorKind};
use vela_hot_reload::error::{HotReloadError, HotReloadErrorKind};
use vela_hot_reload::report::HotReloadReport;
use vela_vm::error::VmError;

pub fn render_engine_source_error(label: &str, source: &str, error: &EngineSourceError) -> String {
    match &error.kind {
        EngineSourceErrorKind::Frontend(error) => {
            render_source_diagnostics(label, source, error.diagnostics())
        }
        EngineSourceErrorKind::Backend(error) => render_compile_error(label, source, error),
        EngineSourceErrorKind::Io { .. }
        | EngineSourceErrorKind::InvalidSourcePath { .. }
        | EngineSourceErrorKind::TooManySources { .. } => error.to_string(),
    }
}

pub fn render_vm_error(label: &str, source: &str, error: &VmError) -> String {
    let source = DiagnosticSource::new(SourceId::new(1), label.to_owned(), source.to_owned());
    render_diagnostics(&[error.to_diagnostic()], source)
}

pub fn render_hot_reload_report(label: &str, source: &str, report: &HotReloadReport) -> String {
    let source = DiagnosticSource::new(SourceId::new(1), label.to_owned(), source.to_owned());
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

pub fn render_hot_reload_error(label: &str, source: &str, error: &HotReloadError) -> String {
    match &error.kind {
        HotReloadErrorKind::Frontend(error) => {
            render_source_diagnostics(label, source, error.diagnostics())
        }
        HotReloadErrorKind::Compile(error) => render_compile_error(label, source, error),
        _ => format!("{error:?}"),
    }
}

fn render_compile_error(label: &str, source: &str, error: &CompileError) -> String {
    let diagnostics = compile_diagnostics(error);

    let source = DiagnosticSource::new(SourceId::new(1), label.to_owned(), source.to_owned());
    render_diagnostics(&diagnostics, source)
}

fn render_source_diagnostics(label: &str, source: &str, diagnostics: &[Diagnostic]) -> String {
    let source = DiagnosticSource::new(SourceId::new(1), label.to_owned(), source.to_owned());
    render_diagnostics(diagnostics, source)
}

fn compile_diagnostics(error: &CompileError) -> Vec<Diagnostic> {
    match &error.kind {
        CompileErrorKind::InvalidHirGraph(diagnostics)
        | CompileErrorKind::SemanticDiagnostics(diagnostics) => diagnostics.clone(),
        _ => error.to_diagnostic().into_iter().collect(),
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

fn render_diagnostics(diagnostics: &[Diagnostic], source: DiagnosticSource) -> String {
    let renderer = DiagnosticRenderer::new(Some(source));
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
