use std::collections::BTreeSet;

use vela_common::{Diagnostic, Severity, SourceId, Span};

use crate::{DocumentId, LanguageServiceDatabases, LineIndex, Position, WorkspaceGeneration};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ServiceDiagnosticSeverity {
    Error,
    Warning,
    Note,
    Help,
}

impl From<Severity> for ServiceDiagnosticSeverity {
    fn from(value: Severity) -> Self {
        match value {
            Severity::Error => Self::Error,
            Severity::Warning => Self::Warning,
            Severity::Note => Self::Note,
            Severity::Help => Self::Help,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct DiagnosticRange {
    start: Position,
    end: Position,
}

impl DiagnosticRange {
    #[must_use]
    pub const fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    #[must_use]
    pub const fn start(self) -> Position {
        self.start
    }

    #[must_use]
    pub const fn end(self) -> Position {
        self.end
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DiagnosticLabel {
    document_id: DocumentId,
    range: DiagnosticRange,
    message: String,
}

impl DiagnosticLabel {
    #[must_use]
    pub fn document_id(&self) -> &DocumentId {
        &self.document_id
    }

    #[must_use]
    pub const fn range(&self) -> DiagnosticRange {
        self.range
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ServiceDiagnostic {
    severity: ServiceDiagnosticSeverity,
    code: Option<String>,
    message: String,
    range: Option<DiagnosticRange>,
    labels: Vec<DiagnosticLabel>,
}

impl ServiceDiagnostic {
    #[must_use]
    pub const fn severity(&self) -> ServiceDiagnosticSeverity {
        self.severity
    }

    #[must_use]
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub const fn range(&self) -> Option<DiagnosticRange> {
        self.range
    }

    #[must_use]
    pub fn labels(&self) -> &[DiagnosticLabel] {
        &self.labels
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DiagnosticStatus {
    Complete,
    Partial,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DocumentDiagnostics {
    document_id: DocumentId,
    generation: WorkspaceGeneration,
    status: DiagnosticStatus,
    diagnostics: Vec<ServiceDiagnostic>,
}

impl DocumentDiagnostics {
    #[must_use]
    pub fn document_id(&self) -> &DocumentId {
        &self.document_id
    }

    #[must_use]
    pub const fn generation(&self) -> WorkspaceGeneration {
        self.generation
    }

    #[must_use]
    pub const fn status(&self) -> DiagnosticStatus {
        self.status
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[ServiceDiagnostic] {
        &self.diagnostics
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct OpenDiagnosticsBatch {
    generation: WorkspaceGeneration,
    documents: Vec<DocumentDiagnostics>,
    pending_workspace_documents: Vec<DocumentId>,
}

impl OpenDiagnosticsBatch {
    #[must_use]
    pub const fn generation(&self) -> WorkspaceGeneration {
        self.generation
    }

    #[must_use]
    pub fn documents(&self) -> &[DocumentDiagnostics] {
        &self.documents
    }

    #[must_use]
    pub fn pending_workspace_documents(&self) -> &[DocumentId] {
        &self.pending_workspace_documents
    }
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn diagnostics_for_document(&self, document_id: &DocumentId) -> DocumentDiagnostics {
        let status = self.diagnostic_status(document_id);
        let diagnostics = self
            .parse_db()
            .parse_diagnostics(document_id)
            .unwrap_or_default()
            .iter()
            .map(|diagnostic| self.convert_diagnostic(diagnostic))
            .collect();
        DocumentDiagnostics {
            document_id: document_id.clone(),
            generation: self.generation(),
            status,
            diagnostics,
        }
    }

    #[must_use]
    pub fn diagnostics_for_open_documents(
        &self,
        open_documents: &BTreeSet<DocumentId>,
    ) -> OpenDiagnosticsBatch {
        let documents = open_documents
            .iter()
            .filter(|document_id| self.source_db().records().contains_key(*document_id))
            .map(|document_id| self.diagnostics_for_document(document_id))
            .collect();
        let pending_workspace_documents = self.pending_workspace_diagnostics(open_documents);
        OpenDiagnosticsBatch {
            generation: self.generation(),
            documents,
            pending_workspace_documents,
        }
    }

    fn diagnostic_status(&self, document_id: &DocumentId) -> DiagnosticStatus {
        self.project_db()
            .module_by_document()
            .get(document_id)
            .filter(|module| self.analysis_db().invalidated_modules().contains(*module))
            .map_or(DiagnosticStatus::Complete, |_| DiagnosticStatus::Partial)
    }

    fn pending_workspace_diagnostics(
        &self,
        open_documents: &BTreeSet<DocumentId>,
    ) -> Vec<DocumentId> {
        self.project_db()
            .module_by_document()
            .iter()
            .filter(|(document_id, module)| {
                !open_documents.contains(*document_id)
                    && self.analysis_db().invalidated_modules().contains(*module)
            })
            .map(|(document_id, _)| document_id.clone())
            .collect()
    }

    fn convert_diagnostic(&self, diagnostic: &Diagnostic) -> ServiceDiagnostic {
        ServiceDiagnostic {
            severity: diagnostic.severity.into(),
            code: diagnostic.code.clone(),
            message: diagnostic.message.clone(),
            range: diagnostic.span.and_then(|span| self.range_for_span(span)),
            labels: diagnostic
                .labels
                .iter()
                .filter_map(|label| {
                    let (document_id, range) = self.document_range_for_span(label.span)?;
                    Some(DiagnosticLabel {
                        document_id,
                        range,
                        message: label.message.clone(),
                    })
                })
                .collect(),
        }
    }

    fn range_for_span(&self, span: Span) -> Option<DiagnosticRange> {
        self.document_range_for_span(span).map(|(_, range)| range)
    }

    fn document_range_for_span(&self, span: Span) -> Option<(DocumentId, DiagnosticRange)> {
        let (document_id, text) = self.text_for_source(span.source)?;
        let line_index = LineIndex::new(text);
        let start = line_index.position(span.start as usize);
        let end = line_index.position(span.end as usize);
        Some((document_id, DiagnosticRange::new(start, end)))
    }

    fn text_for_source(&self, source: SourceId) -> Option<(DocumentId, &str)> {
        self.source_db()
            .records()
            .values()
            .find(|record| record.source_id() == source)
            .map(|record| (record.document_id().clone(), record.text()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
    };

    fn file(path: &str, text: &str) -> SourceFileSnapshot {
        SourceFileSnapshot::new(path, text)
    }

    fn project(files: &[SourceFileSnapshot]) -> crate::ProjectSources {
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        assemble_project_sources(&config, files, &Workspace::new().snapshot())
    }

    #[test]
    fn syntax_diagnostics_map_to_document_ranges() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let mut db = LanguageServiceDatabases::new();
        db.update(&project(&[file(
            document.as_str(),
            "pub fn main( { return 1 }",
        )]));

        let diagnostics = db.diagnostics_for_document(&document);

        assert_eq!(diagnostics.document_id(), &document);
        assert_eq!(diagnostics.status(), DiagnosticStatus::Partial);
        assert!(!diagnostics.diagnostics().is_empty());
        assert!(
            diagnostics
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.range().is_some()
                    && diagnostic.severity() == ServiceDiagnosticSeverity::Error)
        );
    }

    #[test]
    fn open_file_diagnostics_are_prioritized() {
        let mut db = LanguageServiceDatabases::new();
        db.update(&project(&[
            file(
                "/workspace/scripts/game/main.vela",
                "use game::reward::grant\npub fn main() { return grant() }",
            ),
            file(
                "/workspace/scripts/game/wrapper.vela",
                "use game::main::main\npub fn wrapped() { return main() }",
            ),
            file(
                "/workspace/scripts/game/reward.vela",
                "pub fn grant() { return 1 }",
            ),
        ]));
        let open_document = DocumentId::from("/workspace/scripts/game/wrapper.vela");
        let open_documents = BTreeSet::from([open_document.clone()]);
        db.update_with_open_documents(
            &project(&[
                file(
                    "/workspace/scripts/game/main.vela",
                    "use game::reward::grant\npub fn main() { return grant() }",
                ),
                file(
                    "/workspace/scripts/game/wrapper.vela",
                    "use game::main::main\npub fn wrapped() { return main() }",
                ),
                file(
                    "/workspace/scripts/game/reward.vela",
                    "pub fn grant_bonus() { return 1 }",
                ),
            ]),
            &open_documents,
        );

        let batch = db.diagnostics_for_open_documents(&open_documents);

        assert_eq!(batch.documents().len(), 1);
        assert_eq!(batch.documents()[0].document_id(), &open_document);
        assert_eq!(batch.documents()[0].status(), DiagnosticStatus::Partial);
        assert!(
            batch
                .pending_workspace_documents()
                .iter()
                .any(|document| document.as_str() == "/workspace/scripts/game/reward.vela")
        );
    }
}
