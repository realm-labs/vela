use std::sync::Arc;

use vela_common::{Diagnostic, Label, Span};

use crate::error::HotReloadError;
use crate::report_detail::HotReloadDiagnosticDetail;
use crate::report_render::HotReloadReportLine;
use crate::symbol::{FunctionSymbolId, ProgramVersionId};
use crate::version::ProgramVersion;

#[derive(Clone, Debug, PartialEq)]
pub struct HotReloadReport {
    pub accepted: bool,
    pub from_version: ProgramVersionId,
    pub to_version: Option<ProgramVersionId>,
    pub changed_functions: Vec<String>,
    pub changed_modules: Vec<String>,
    pub impacted_modules: Vec<String>,
    pub errors: Vec<HotReloadDiagnostic>,
    version: Option<Arc<ProgramVersion>>,
}

impl HotReloadReport {
    #[must_use]
    pub(crate) fn accepted(
        from_version: ProgramVersionId,
        version: Arc<ProgramVersion>,
        changes: AcceptedHotReloadChanges,
    ) -> Self {
        let changed_functions = sorted_functions(changes.changed_functions);
        let changed_modules = sorted_strings(changes.changed_modules);
        let impacted_modules = sorted_strings(changes.impacted_modules);
        Self {
            accepted: true,
            from_version,
            to_version: Some(version.id),
            changed_functions,
            changed_modules,
            impacted_modules,
            errors: Vec::new(),
            version: Some(version),
        }
    }

    #[must_use]
    pub fn rejected(from_version: ProgramVersionId, error: HotReloadError) -> Self {
        Self {
            accepted: false,
            from_version,
            to_version: None,
            changed_functions: Vec::new(),
            changed_modules: Vec::new(),
            impacted_modules: Vec::new(),
            errors: vec![HotReloadDiagnostic::from_error(error)],
            version: None,
        }
    }

    #[must_use]
    pub fn version(&self) -> Option<Arc<ProgramVersion>> {
        self.version.as_ref().map(Arc::clone)
    }

    #[must_use]
    pub fn render_lines(&self) -> Vec<HotReloadReportLine> {
        crate::report_render::render_lines(self)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct AcceptedHotReloadChanges {
    pub changed_functions: Vec<FunctionSymbolId>,
    pub changed_modules: Vec<String>,
    pub impacted_modules: Vec<String>,
}

impl AcceptedHotReloadChanges {
    #[must_use]
    pub fn new(
        changed_functions: Vec<FunctionSymbolId>,
        changed_modules: Vec<String>,
        impacted_modules: Vec<String>,
    ) -> Self {
        Self {
            changed_functions,
            changed_modules,
            impacted_modules,
        }
    }
}

fn sorted_functions(functions: Vec<FunctionSymbolId>) -> Vec<String> {
    sorted_strings(functions.into_iter().map(|function| function.0))
}

fn sorted_strings(strings: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut strings = strings.into_iter().collect::<Vec<_>>();
    strings.sort();
    strings.dedup();
    strings
}

#[derive(Clone, Debug, PartialEq)]
pub struct HotReloadDiagnostic {
    pub code: &'static str,
    pub target: Option<String>,
    pub detail: Option<HotReloadDiagnosticDetail>,
    pub source_span: Option<Span>,
    pub labels: Vec<Label>,
    pub source_diagnostics: Vec<Diagnostic>,
    pub reason: String,
    pub repair_hint: Option<String>,
    pub error: HotReloadError,
}

impl HotReloadDiagnostic {
    #[must_use]
    pub fn from_error(error: HotReloadError) -> Self {
        let detail = HotReloadDiagnosticDetail::from_error(&error);
        Self {
            code: error.code(),
            target: error.target(),
            detail,
            source_span: error.source_span(),
            labels: error.labels(),
            source_diagnostics: error.source_diagnostics(),
            reason: error.reason(),
            repair_hint: error.repair_hint(),
            error,
        }
    }
}
