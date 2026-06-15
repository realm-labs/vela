//! Editor-neutral language-service workspace state.

mod call_hierarchy;
mod completion;
mod definition;
mod diagnostics;
mod folding;
mod hover;
mod incremental;
mod project;
mod references;
mod schema;
mod selection;
mod semantic_tokens;
mod signature;
mod symbols;
mod text;
mod workspace;

pub use call_hierarchy::{CallHierarchyItem, IncomingCall, OutgoingCall};
pub use completion::{
    CompletionContext, CompletionContextKind, CompletionItem, CompletionKind, CompletionList,
};
pub use definition::Definition;
pub use diagnostics::{
    DiagnosticCandidate, DiagnosticLabel, DiagnosticRange, DiagnosticRepairHint, DiagnosticStatus,
    DocumentDiagnostics, OpenDiagnosticsBatch, ServiceDiagnostic, ServiceDiagnosticSeverity,
    WorkspaceDiagnosticsBatch,
};
pub use folding::{FoldingRange, FoldingRangeKind};
pub use hover::{Hover, HoverKind};
pub use incremental::{
    AnalysisDb, BackgroundResult, CancellationHandle, CancellationToken, GenerationToken, HirDb,
    IndexingMetrics, InvalidationReport, LanguageServiceDatabases, ParseDb, ProjectDb,
    ScheduledModule, SchemaDb, SchemaDiagnostic, SourceDb, SourceRecord, WorkPriority,
};
pub use project::{
    ConfigParseResult, ProjectDiagnostic, ProjectMode, ProjectSources, SchemaConfig,
    SourceFileSnapshot, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
    missing_import_diagnostics,
};
pub use references::{DocumentHighlight, DocumentHighlightKind, Reference, ReferenceKind};
pub use schema::{
    SCHEMA_ARTIFACT_FORMAT_VERSION, SchemaArtifact, SchemaArtifactError, SchemaArtifactFacts,
};
pub use selection::SelectionRange;
pub use semantic_tokens::{
    SemanticToken, SemanticTokenModifiers, SemanticTokenType, SemanticTokens,
};
pub use signature::{SignatureHelp, SignatureInformation, SignatureParameter};
pub use symbols::{DocumentSymbol, DocumentSymbolKind, WorkspaceSymbol, WorkspaceSymbolLocation};
pub use text::{LineIndex, Position, TextRange};
pub use workspace::{
    DocumentId, DocumentSnapshot, SourceVersion, Workspace, WorkspaceGeneration, WorkspaceSnapshot,
};
