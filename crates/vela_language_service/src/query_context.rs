use vela_syntax::ast::SourceFile;

use crate::{
    CursorContext, DocumentId, DocumentSnapshot, LanguageServiceDatabases, Position, SourceRecord,
    SourceVersion, WorkspaceGeneration, WorkspaceSnapshot, cursor_context_at,
};
use vela_common::SourceId;
use vela_hir::binding::BindingMap;
use vela_hir::module_graph::ModulePath;

#[derive(Debug, Clone)]
enum QuerySource<'a> {
    Snapshot(DocumentSnapshot),
    Database(&'a SourceRecord),
}

impl QuerySource<'_> {
    fn text(&self) -> &str {
        match self {
            Self::Snapshot(document) => document.text(),
            Self::Database(source) => source.text(),
        }
    }

    const fn version(&self) -> SourceVersion {
        match self {
            Self::Snapshot(document) => document.version(),
            Self::Database(source) => source.version(),
        }
    }

    const fn source_record(&self) -> Option<&SourceRecord> {
        match self {
            Self::Snapshot(_) => None,
            Self::Database(source) => Some(source),
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueryContext<'a> {
    document_id: DocumentId,
    position: Position,
    generation: WorkspaceGeneration,
    source: QuerySource<'a>,
    parsed: Option<&'a SourceFile>,
    bindings: Option<&'a BindingMap>,
    cursor: CursorContext,
}

impl<'a> QueryContext<'a> {
    #[must_use]
    pub fn from_workspace_snapshot(
        snapshot: &WorkspaceSnapshot,
        document_id: &DocumentId,
        position: Position,
    ) -> Option<Self> {
        let document = snapshot.document(document_id)?;
        let cursor = cursor_context_at(document.text(), position, None);
        Some(Self {
            document_id: document_id.clone(),
            position,
            generation: snapshot.generation(),
            source: QuerySource::Snapshot(document),
            parsed: None,
            bindings: None,
            cursor,
        })
    }

    #[must_use]
    pub(crate) fn from_databases(
        databases: &'a LanguageServiceDatabases,
        document_id: &DocumentId,
        position: Position,
    ) -> Option<Self> {
        let source = databases.source_db().records().get(document_id)?;
        let parsed = databases.parse_db().parsed_source(document_id);
        let cursor = cursor_context_at(source.text(), position, parsed);
        let bindings = query_bindings(databases, source, cursor.replace_range().end);
        Some(Self {
            document_id: document_id.clone(),
            position,
            generation: databases.generation(),
            source: QuerySource::Database(source),
            parsed,
            bindings,
            cursor,
        })
    }

    #[must_use]
    pub fn document_id(&self) -> &DocumentId {
        &self.document_id
    }

    #[must_use]
    pub const fn position(&self) -> Position {
        self.position
    }

    #[must_use]
    pub const fn generation(&self) -> WorkspaceGeneration {
        self.generation
    }

    #[must_use]
    pub fn text(&self) -> &str {
        self.source.text()
    }

    #[must_use]
    pub const fn version(&self) -> SourceVersion {
        self.source.version()
    }

    #[must_use]
    pub const fn source_record(&self) -> Option<&SourceRecord> {
        self.source.source_record()
    }

    #[must_use]
    pub const fn source_id(&self) -> Option<SourceId> {
        match self.source_record() {
            Some(source) => Some(source.source_id()),
            None => None,
        }
    }

    #[must_use]
    pub fn module_path(&self) -> Option<&ModulePath> {
        self.source_record().map(SourceRecord::module_path)
    }

    #[must_use]
    pub const fn parsed_source(&self) -> Option<&SourceFile> {
        self.parsed
    }

    #[must_use]
    pub const fn bindings(&self) -> Option<&BindingMap> {
        self.bindings
    }

    #[must_use]
    pub const fn cursor(&self) -> &CursorContext {
        &self.cursor
    }
}

fn query_bindings<'a>(
    databases: &'a LanguageServiceDatabases,
    source: &SourceRecord,
    offset: usize,
) -> Option<&'a BindingMap> {
    let offset = u32::try_from(offset).ok()?;
    let source_id = source.source_id();
    let graph = databases.hir_db().graph();
    graph.declarations().find_map(|declaration| {
        (declaration.span.source == source_id && declaration.span.contains(offset))
            .then(|| graph.bindings(declaration.id))
            .flatten()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
    };

    #[test]
    fn query_context_uses_workspace_snapshot_generation_and_overlay_text() {
        let document = DocumentId::from("/workspace/scripts/main.vela");
        let mut workspace = Workspace::new();
        workspace.set_disk_snapshot(
            document.clone(),
            "fn disk() -> i64 { return 1 }",
            SourceVersion::new(1),
        );
        workspace.open_document(document.clone(), "st", SourceVersion::new(2));
        let snapshot = workspace.snapshot();

        workspace.change_document(document.clone(), "fn live() {}", SourceVersion::new(3));

        let context =
            QueryContext::from_workspace_snapshot(&snapshot, &document, Position::new(0, 2))
                .expect("snapshot document exists");
        assert_eq!(context.document_id(), &document);
        assert_eq!(context.generation(), snapshot.generation());
        assert_eq!(context.version(), SourceVersion::new(2));
        assert_eq!(context.text(), "st");
        assert_eq!(context.cursor().prefix(), "st");
        assert_eq!(context.source_id(), None);
        assert!(context.module_path().is_none());
        assert!(context.source_record().is_none());
        assert!(context.parsed_source().is_none());
        assert!(context.bindings().is_none());
    }

    #[test]
    fn query_context_from_databases_carries_parsed_source_and_module_facts() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let source = "struct Player { level: i64 }\nfn main() { let player = Player { le } }";
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let workspace = Workspace::new();
        let files = vec![SourceFileSnapshot::new(document.clone(), source)];
        let project = assemble_project_sources(&config, &files, &workspace.snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        let line = source.lines().nth(1).expect("fixture has function line");
        let position = Position::new(
            1,
            line.find("le }").expect("fixture has field prefix") + "le".len(),
        );

        let context = QueryContext::from_databases(&databases, &document, position)
            .expect("database document exists");

        assert_eq!(context.document_id(), &document);
        assert_eq!(context.generation(), databases.generation());
        assert_eq!(context.text(), source);
        assert_eq!(context.cursor().prefix(), "le");
        assert_eq!(context.source_id(), Some(SourceId::new(1)));
        assert!(context.parsed_source().is_some());
        assert!(
            context
                .bindings()
                .expect("bindings")
                .locals()
                .any(|local| local.name == "player")
        );
        assert_eq!(
            context.module_path().expect("module path").segments(),
            &["game".to_owned(), "main".to_owned()]
        );
    }
}
