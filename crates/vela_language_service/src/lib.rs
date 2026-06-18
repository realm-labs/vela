//! Editor-neutral language-service workspace state.

mod call_hierarchy;
mod callable_context;
mod code_action;
mod completion;
mod cursor_context;
mod definition;
mod diagnostics;
mod display;
mod expression_facts;
mod folding;
mod formatting;
mod hover;
mod incremental;
mod inlay;
mod member_access;
mod path_calls;
mod project;
mod query_context;
mod references;
mod rename;
mod schema;
mod selection;
mod semantic_tokens;
mod signature;
mod symbol_ref;
mod symbol_target;
mod symbols;
mod text;
mod workspace;

pub use call_hierarchy::{CallHierarchyItem, IncomingCall, OutgoingCall};
pub use code_action::{CodeAction, CodeActionKind};
pub use completion::{
    CompletionAnalysis, CompletionAnalysisKind, CompletionCallArgumentContext, CompletionContext,
    CompletionContextKind, CompletionDeclaration, CompletionDeclarationKind,
    CompletionInsertFormat, CompletionItem, CompletionItemMetadata, CompletionKind,
    CompletionLabelDetails, CompletionList, CompletionRelevance, CompletionResolvePayload,
    CompletionSymbol, CompletionTextEdit, DotAccess, PathCompletionCtx, PathCompletionKind,
    PatternContext, RecordFieldContext, StatementContext, TypeLocation,
};
pub use cursor_context::{CursorContext, CursorContextKind, ModulePathRole, cursor_context_at};
pub use definition::Definition;
pub use diagnostics::{
    DiagnosticCandidate, DiagnosticLabel, DiagnosticRange, DiagnosticRepairHint, DiagnosticStatus,
    DocumentDiagnostics, OpenDiagnosticsBatch, ServiceDiagnostic, ServiceDiagnosticSeverity,
    WorkspaceDiagnosticsBatch,
};
pub use display::{DisplayPart, DisplayPartKind, DisplayParts};
pub use folding::{FoldingRange, FoldingRangeKind};
pub use formatting::{FormattingIr, FormattingSegment, FormattingSegmentKind};
pub use hover::{Hover, HoverKind};
pub use incremental::{
    AnalysisDb, BackgroundResult, CancellationHandle, CancellationToken, GenerationToken, HirDb,
    IndexingMetrics, InvalidationReport, LanguageServiceDatabases, ModuleFingerprint, ParseDb,
    ProjectDb, ScheduledModule, SchemaDb, SchemaDiagnostic, SourceDb, SourceRecord, WorkPriority,
};
pub use inlay::{InlayHint, InlayHintKind};
pub use project::{
    ConfigParseResult, ProjectDiagnostic, ProjectMode, ProjectSources, SchemaConfig,
    SourceFileSnapshot, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
    missing_import_diagnostics,
};
pub use query_context::{CallArgumentFacts, QueryContext};
pub use references::{
    DocumentHighlight, DocumentHighlightKind, Reference, ReferenceKind, ReferenceQueryResult,
    ReferenceResolution,
};
pub use rename::{
    DocumentTextEdit, EditPlan, PrepareRename, RenameRisk, RenameRiskKind, TextEdit, WorkspaceEdit,
};
pub use schema::{
    SCHEMA_ARTIFACT_FORMAT_VERSION, SchemaArtifact, SchemaArtifactError, SchemaArtifactFacts,
    SchemaSourceLocations,
};
pub use selection::SelectionRange;
pub use semantic_tokens::{
    SemanticToken, SemanticTokenDelta, SemanticTokenEdit, SemanticTokenModifiers,
    SemanticTokenType, SemanticTokens,
};
pub use signature::{SignatureHelp, SignatureInformation, SignatureParameter};
pub use symbol_ref::{LocalSymbolRef, SymbolRef};
pub use symbols::{DocumentSymbol, DocumentSymbolKind, WorkspaceSymbol, WorkspaceSymbolLocation};
pub use text::{LineIndex, Position, TextRange};
pub use workspace::{
    DocumentId, DocumentSnapshot, SourceVersion, Workspace, WorkspaceGeneration, WorkspaceSnapshot,
};
