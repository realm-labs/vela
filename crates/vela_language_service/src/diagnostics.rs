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
pub struct DiagnosticCandidate {
    replacement: String,
}

impl DiagnosticCandidate {
    #[must_use]
    pub fn replacement(&self) -> &str {
        &self.replacement
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DiagnosticRepairHint {
    document_id: DocumentId,
    range: DiagnosticRange,
    title: String,
    replacement: String,
}

impl DiagnosticRepairHint {
    #[must_use]
    pub fn document_id(&self) -> &DocumentId {
        &self.document_id
    }

    #[must_use]
    pub const fn range(&self) -> DiagnosticRange {
        self.range
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn replacement(&self) -> &str {
        &self.replacement
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ServiceDiagnostic {
    severity: ServiceDiagnosticSeverity,
    code: Option<String>,
    message: String,
    range: Option<DiagnosticRange>,
    labels: Vec<DiagnosticLabel>,
    candidates: Vec<DiagnosticCandidate>,
    repair_hints: Vec<DiagnosticRepairHint>,
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

    #[must_use]
    pub fn candidates(&self) -> &[DiagnosticCandidate] {
        &self.candidates
    }

    #[must_use]
    pub fn repair_hints(&self) -> &[DiagnosticRepairHint] {
        &self.repair_hints
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DiagnosticStatus {
    Complete,
    Partial,
    Stale,
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WorkspaceDiagnosticsBatch {
    generation: WorkspaceGeneration,
    documents: Vec<DocumentDiagnostics>,
}

impl WorkspaceDiagnosticsBatch {
    #[must_use]
    pub const fn generation(&self) -> WorkspaceGeneration {
        self.generation
    }

    #[must_use]
    pub fn documents(&self) -> &[DocumentDiagnostics] {
        &self.documents
    }
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn diagnostics_for_document(&self, document_id: &DocumentId) -> DocumentDiagnostics {
        self.diagnostics_for_document_at_generation(document_id, self.generation())
    }

    #[must_use]
    pub fn diagnostics_for_document_at_generation(
        &self,
        document_id: &DocumentId,
        generation: WorkspaceGeneration,
    ) -> DocumentDiagnostics {
        if generation != self.generation() {
            return DocumentDiagnostics {
                document_id: document_id.clone(),
                generation,
                status: DiagnosticStatus::Stale,
                diagnostics: Vec::new(),
            };
        }

        let status = self.diagnostic_status(document_id);
        let diagnostics = self.document_diagnostics(document_id);
        DocumentDiagnostics {
            document_id: document_id.clone(),
            generation,
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

    #[must_use]
    pub fn diagnostics_for_workspace_documents(
        &self,
        open_documents: &BTreeSet<DocumentId>,
    ) -> WorkspaceDiagnosticsBatch {
        self.diagnostics_for_workspace_documents_at_generation(open_documents, self.generation())
    }

    #[must_use]
    pub fn diagnostics_for_workspace_documents_at_generation(
        &self,
        open_documents: &BTreeSet<DocumentId>,
        generation: WorkspaceGeneration,
    ) -> WorkspaceDiagnosticsBatch {
        let documents = self
            .workspace_document_ids(open_documents)
            .into_iter()
            .map(|document_id| {
                self.diagnostics_for_document_at_generation(&document_id, generation)
            })
            .collect();
        WorkspaceDiagnosticsBatch {
            generation,
            documents,
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
        self.workspace_document_ids(open_documents)
            .into_iter()
            .filter(|document_id| {
                self.project_db()
                    .module_by_document()
                    .get(document_id)
                    .is_some_and(|module| self.analysis_db().invalidated_modules().contains(module))
            })
            .collect()
    }

    fn workspace_document_ids(&self, open_documents: &BTreeSet<DocumentId>) -> Vec<DocumentId> {
        self.project_db()
            .module_by_document()
            .keys()
            .filter(|document_id| {
                !open_documents.contains(*document_id)
                    && self.source_db().records().contains_key(*document_id)
            })
            .cloned()
            .collect()
    }

    fn document_diagnostics(&self, document_id: &DocumentId) -> Vec<ServiceDiagnostic> {
        let mut diagnostics = self
            .parse_db()
            .parse_diagnostics(document_id)
            .unwrap_or_default()
            .iter()
            .map(|diagnostic| self.convert_diagnostic(diagnostic))
            .collect::<Vec<_>>();

        if let Some(source) = self.parse_db().source_id(document_id) {
            diagnostics.extend(
                self.hir_db()
                    .graph()
                    .diagnostics()
                    .iter()
                    .filter(|diagnostic| is_hir_diagnostic(diagnostic))
                    .filter(|diagnostic| diagnostic_mentions_source(diagnostic, source))
                    .map(|diagnostic| self.convert_diagnostic(diagnostic)),
            );
        }

        if let Some(parsed) = self.parse_db().parsed_source(document_id) {
            let graph = self.hir_db().graph();
            let source_diagnostics = self
                .project_db()
                .module_by_document()
                .get(document_id)
                .and_then(|module_path| graph.module_id(module_path))
                .map_or_else(
                    || {
                        vela_analysis::diagnostics::source_diagnostics(
                            parsed,
                            self.schema_db().facts(),
                        )
                    },
                    |module| {
                        vela_analysis::diagnostics::source_diagnostics_in_module(
                            parsed,
                            graph,
                            module,
                            self.schema_db().facts(),
                        )
                    },
                );
            diagnostics.extend(
                source_diagnostics
                    .iter()
                    .map(|diagnostic| self.convert_diagnostic(diagnostic)),
            );
        }

        diagnostics.extend(self.schema_db().diagnostics().iter().map(schema_diagnostic));

        diagnostics
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
            candidates: diagnostic
                .candidates
                .iter()
                .map(|candidate| DiagnosticCandidate {
                    replacement: candidate.replacement.clone(),
                })
                .collect(),
            repair_hints: diagnostic
                .repairs
                .iter()
                .filter_map(|repair| {
                    let (document_id, range) = self.document_range_for_span(repair.span)?;
                    Some(DiagnosticRepairHint {
                        document_id,
                        range,
                        title: repair.title.clone(),
                        replacement: repair.replacement.clone(),
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

fn schema_diagnostic(diagnostic: &crate::SchemaDiagnostic) -> ServiceDiagnostic {
    ServiceDiagnostic {
        severity: ServiceDiagnosticSeverity::Warning,
        code: Some("schema::unavailable".to_owned()),
        message: diagnostic.message().to_owned(),
        range: None,
        labels: Vec::new(),
        candidates: Vec::new(),
        repair_hints: Vec::new(),
    }
}

fn is_hir_diagnostic(diagnostic: &Diagnostic) -> bool {
    diagnostic
        .code
        .as_deref()
        .is_some_and(|code| code.starts_with("hir::"))
}

fn diagnostic_mentions_source(diagnostic: &Diagnostic, source: SourceId) -> bool {
    diagnostic.span.is_some_and(|span| span.source == source)
        || diagnostic
            .labels
            .iter()
            .any(|label| label.span.source == source)
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

    #[test]
    fn hir_diagnostics_survive_multi_file_workspace() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let helper = DocumentId::from("/workspace/scripts/game/reward.vela");
        let mut db = LanguageServiceDatabases::new();
        db.update(&project(&[
            file(
                document.as_str(),
                "use game::reward::grant_bonus\npub fn main() { return 1 }",
            ),
            file(helper.as_str(), "pub fn grant() { return 1 }"),
        ]));

        let diagnostics = db.diagnostics_for_document(&document);
        let helper_diagnostics = db.diagnostics_for_document(&helper);

        assert!(
            diagnostics.diagnostics().iter().any(|diagnostic| {
                diagnostic.code() == Some("hir::unresolved_import")
                    && diagnostic.range().is_some()
                    && diagnostic
                        .labels()
                        .iter()
                        .any(|label| label.document_id() == &document)
            }),
            "{:?}",
            diagnostics.diagnostics()
        );
        assert!(helper_diagnostics.diagnostics().is_empty());
    }

    #[test]
    fn analysis_diagnostics_map_to_document_ranges() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let mut db = LanguageServiceDatabases::new();
        db.update(&project(&[file(
            document.as_str(),
            "pub fn main(scores: Array<i64>) { return scores.frist() }",
        )]));

        let diagnostics = db.diagnostics_for_document(&document);

        assert!(
            diagnostics.diagnostics().iter().any(|diagnostic| {
                diagnostic.code() == Some("analysis::unknown_method")
                    && diagnostic.range().is_some()
                    && diagnostic
                        .labels()
                        .iter()
                        .any(|label| label.document_id() == &document)
            }),
            "{:?}",
            diagnostics.diagnostics()
        );
    }

    #[test]
    fn syntax_errors_do_not_block_unaffected_module_diagnostics() {
        let broken_document = DocumentId::from("/workspace/scripts/game/broken.vela");
        let healthy_document = DocumentId::from("/workspace/scripts/game/reward.vela");
        let mut db = LanguageServiceDatabases::new();
        db.update(&project(&[
            file(broken_document.as_str(), "pub fn broken( { return 1 }"),
            file(
                healthy_document.as_str(),
                "pub fn reward(scores: Array<i64>) { return scores.frist() }",
            ),
        ]));

        let broken_diagnostics = db.diagnostics_for_document(&broken_document);
        let healthy_diagnostics = db.diagnostics_for_document(&healthy_document);

        assert!(
            broken_diagnostics
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.severity() == ServiceDiagnosticSeverity::Error),
            "{:?}",
            broken_diagnostics.diagnostics()
        );
        assert!(
            healthy_diagnostics
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == Some("analysis::unknown_method")),
            "{:?}",
            healthy_diagnostics.diagnostics()
        );
    }

    #[test]
    fn schema_diagnostics_degrade_to_any() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let mut db = LanguageServiceDatabases::new();
        db.mark_schema_missing("/workspace/target/vela/schema.json");
        db.update(&project(&[file(
            document.as_str(),
            "pub fn main(player: Player, scores: Array<i64>) {
                player.level
                scores.frist()
            }",
        )]));

        let diagnostics = db.diagnostics_for_document(&document);

        assert!(
            diagnostics
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == Some("schema::unavailable")),
            "{:?}",
            diagnostics.diagnostics()
        );
        assert!(
            diagnostics
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == Some("analysis::unknown_method")),
            "{:?}",
            diagnostics.diagnostics()
        );
        assert!(
            diagnostics
                .diagnostics()
                .iter()
                .all(|diagnostic| diagnostic.code() != Some("analysis::unknown_field")),
            "{:?}",
            diagnostics.diagnostics()
        );
    }

    #[test]
    fn structured_diagnostics_preserve_candidates_and_repair_hints() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let mut db = LanguageServiceDatabases::new();
        db.update(&project(&[file(
            document.as_str(),
            "pub fn main() { return levle }",
        )]));
        let source = db.parse_db().source_id(&document).expect("source exists");
        let diagnostic = Diagnostic::error("unknown name `levle`")
            .with_code("hir::unresolved_name")
            .with_span(Span::new(source, 23, 28))
            .with_candidate("level")
            .with_repair("replace with `level`", Span::new(source, 23, 28), "level");

        let converted = db.convert_diagnostic(&diagnostic);

        assert_eq!(converted.candidates().len(), 1);
        assert_eq!(converted.candidates()[0].replacement(), "level");
        assert_eq!(converted.repair_hints().len(), 1);
        assert_eq!(converted.repair_hints()[0].document_id(), &document);
        assert_eq!(converted.repair_hints()[0].title(), "replace with `level`");
        assert_eq!(converted.repair_hints()[0].replacement(), "level");
        assert_eq!(
            converted.repair_hints()[0].range().start(),
            Position::new(0, 23)
        );
        assert_eq!(
            converted.repair_hints()[0].range().end(),
            Position::new(0, 28)
        );
    }

    #[test]
    fn partial_diagnostics_report_stale_generation() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let mut db = LanguageServiceDatabases::new();
        db.update(&project(&[file(
            document.as_str(),
            "pub fn main(scores: Array<i64>) { return scores.frist() }",
        )]));
        let stale_generation = db.generation();

        db.update(&project(&[file(
            document.as_str(),
            "pub fn main(scores: Array<i64>) { return scores.first() }",
        )]));

        let stale = db.diagnostics_for_document_at_generation(&document, stale_generation);
        let current = db.diagnostics_for_document_at_generation(&document, db.generation());

        assert_eq!(stale.document_id(), &document);
        assert_eq!(stale.generation(), stale_generation);
        assert_eq!(stale.status(), DiagnosticStatus::Stale);
        assert!(stale.diagnostics().is_empty());
        assert_eq!(current.status(), DiagnosticStatus::Partial);
        assert_eq!(current.generation(), db.generation());
    }

    #[test]
    fn workspace_diagnostics_include_background_documents() {
        let open_document = DocumentId::from("/workspace/scripts/game/main.vela");
        let workspace_document = DocumentId::from("/workspace/scripts/game/reward.vela");
        let mut db = LanguageServiceDatabases::new();
        db.update(&project(&[
            file(open_document.as_str(), "pub fn main() { return 1 }"),
            file(
                workspace_document.as_str(),
                "pub fn reward(scores: Array<i64>) { return scores.frist() }",
            ),
        ]));
        let open_documents = BTreeSet::from([open_document.clone()]);

        let open_batch = db.diagnostics_for_open_documents(&open_documents);
        let workspace_batch = db.diagnostics_for_workspace_documents(&open_documents);

        assert_eq!(open_batch.documents().len(), 1);
        assert_eq!(open_batch.documents()[0].document_id(), &open_document);
        assert_eq!(workspace_batch.generation(), db.generation());
        assert_eq!(workspace_batch.documents().len(), 1);
        assert_eq!(
            workspace_batch.documents()[0].document_id(),
            &workspace_document
        );
        assert!(
            workspace_batch.documents()[0]
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == Some("analysis::unknown_method"))
        );
    }
}
