//! Editor-neutral language-service workspace state.

mod diagnostics;
mod incremental;
mod project;
mod text;
mod workspace;

pub use diagnostics::{
    DiagnosticLabel, DiagnosticRange, DiagnosticStatus, DocumentDiagnostics, OpenDiagnosticsBatch,
    ServiceDiagnostic, ServiceDiagnosticSeverity, WorkspaceDiagnosticsBatch,
};
pub use incremental::{
    AnalysisDb, BackgroundResult, CancellationHandle, CancellationToken, GenerationToken, HirDb,
    IndexingMetrics, InvalidationReport, LanguageServiceDatabases, ParseDb, ProjectDb,
    ScheduledModule, SourceDb, SourceRecord, WorkPriority,
};
pub use project::{
    ConfigParseResult, ProjectDiagnostic, ProjectMode, ProjectSources, SchemaConfig,
    SourceFileSnapshot, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
    missing_import_diagnostics,
};
pub use text::{LineIndex, Position, TextRange};
pub use workspace::{
    DocumentId, DocumentSnapshot, SourceVersion, Workspace, WorkspaceGeneration, WorkspaceSnapshot,
};
