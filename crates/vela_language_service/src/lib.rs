//! Editor-neutral language-service workspace state.

mod incremental;
mod project;
mod text;
mod workspace;

pub use incremental::{
    AnalysisDb, BackgroundResult, GenerationToken, HirDb, InvalidationReport,
    LanguageServiceDatabases, ParseDb, ProjectDb, SourceDb, SourceRecord,
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
