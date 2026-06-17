use vela_syntax::ast::SourceFile;

use crate::{
    CursorContext, DocumentId, DocumentSnapshot, LanguageServiceDatabases, Position, SourceRecord,
    SourceVersion, TextRange, WorkspaceGeneration, WorkspaceSnapshot, cursor_context_at,
};
use vela_common::SourceId;
use vela_hir::binding::{BindingMap, LocalBinding};
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

    pub fn local_bindings_before_cursor(&self) -> impl Iterator<Item = &LocalBinding> + '_ {
        let offset = u32::try_from(self.cursor.replace_range().end).ok();
        self.bindings.into_iter().flat_map(move |bindings| {
            bindings
                .locals()
                .filter(move |local| offset.is_some_and(|offset| local.span.end <= offset))
        })
    }

    #[must_use]
    pub const fn cursor(&self) -> &CursorContext {
        &self.cursor
    }

    #[must_use]
    pub const fn identifier_range(&self) -> Option<TextRange> {
        self.cursor.identifier_range()
    }

    #[must_use]
    pub fn identifier_text(&self) -> Option<&str> {
        text_range(self.text(), self.identifier_range()?)
    }

    #[must_use]
    pub const fn member_receiver_range(&self) -> Option<TextRange> {
        self.cursor.member_receiver()
    }

    #[must_use]
    pub const fn call_open_offset(&self) -> Option<usize> {
        self.cursor.call_open()
    }

    #[must_use]
    pub fn member_receiver_text(&self) -> Option<&str> {
        text_range(self.text(), self.member_receiver_range()?)
    }

    #[must_use]
    pub const fn call_callee_range(&self) -> Option<TextRange> {
        self.cursor.call_callee()
    }

    #[must_use]
    pub fn call_callee_text(&self) -> Option<&str> {
        text_range(self.text(), self.call_callee_range()?)
    }

    #[must_use]
    pub const fn call_member_receiver_range(&self) -> Option<TextRange> {
        self.cursor.call_member_receiver()
    }

    #[must_use]
    pub fn call_member_receiver_text(&self) -> Option<&str> {
        text_range(self.text(), self.call_member_receiver_range()?)
    }

    #[must_use]
    pub const fn lambda_method_range(&self) -> Option<TextRange> {
        self.cursor.lambda_method()
    }

    #[must_use]
    pub fn lambda_method_text(&self) -> Option<&str> {
        text_range(self.text(), self.lambda_method_range()?)
    }
}

fn text_range(text: &str, range: TextRange) -> Option<&str> {
    text.get(range.start..range.end)
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
        LineIndex, SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot,
        assemble_project_sources,
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
        assert_eq!(context.identifier_range(), Some(TextRange::new(0, 2)));
        assert_eq!(context.identifier_text(), Some("st"));
        assert_eq!(context.source_id(), None);
        assert!(context.module_path().is_none());
        assert!(context.source_record().is_none());
        assert!(context.parsed_source().is_none());
        assert!(context.bindings().is_none());
    }

    #[test]
    fn query_context_from_databases_carries_parsed_source_and_module_facts() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let source =
            "struct Player { level: i64 }\nfn main() { let player = Player { le }; let after = 1 }";
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
        let field_start = source.find("le };").expect("field prefix");
        assert_eq!(
            context.identifier_range(),
            Some(TextRange::new(field_start, field_start + "le".len()))
        );
        assert_eq!(context.identifier_text(), Some("le"));
        assert_eq!(context.source_id(), Some(SourceId::new(1)));
        assert!(context.parsed_source().is_some());
        assert!(
            context
                .bindings()
                .expect("bindings")
                .locals()
                .any(|local| local.name == "player")
        );
        let visible_locals = context
            .local_bindings_before_cursor()
            .map(|local| local.name.as_str())
            .collect::<Vec<_>>();
        assert!(visible_locals.is_empty());
        let local_position =
            LineIndex::new(source).position(source.find("let after").expect("second statement"));
        let local_context = QueryContext::from_databases(&databases, &document, local_position)
            .expect("local query");
        let visible_locals = local_context
            .local_bindings_before_cursor()
            .map(|local| local.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(visible_locals, vec!["player"]);
        assert_eq!(
            context.module_path().expect("module path").segments(),
            &["game".to_owned(), "main".to_owned()]
        );
    }

    #[test]
    fn query_context_exposes_cursor_receiver_and_callee_text() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let source = "pub fn current_player() -> Player { return Player { level: 1 } }\n\
                      pub fn main(player: Player, scores: Array<i64>) { player.level; grant(current_player().level); scores.filter(player); scores.map(|) }";
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let workspace = Workspace::new();
        let files = vec![SourceFileSnapshot::new(document.clone(), source)];
        let project = assemble_project_sources(&config, &files, &workspace.snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        let member_offset = source.find("level;").expect("member access") + "level".len();
        let member_context = QueryContext::from_databases(
            &databases,
            &document,
            LineIndex::new(source).position(member_offset),
        )
        .expect("member query");
        let expected_receiver_start = source.find("player.level").expect("receiver occurrence");
        assert_eq!(
            member_context.member_receiver_range(),
            Some(TextRange::new(
                expected_receiver_start,
                expected_receiver_start + "player".len()
            ))
        );
        assert_eq!(member_context.member_receiver_text(), Some("player"));

        let call_offset = source.find("current_player().level").expect("call arg") + 1;
        let call_context = QueryContext::from_databases(
            &databases,
            &document,
            LineIndex::new(source).position(call_offset),
        )
        .expect("call query");
        let expected_callee_start = source.find("grant(").expect("callee occurrence");
        assert_eq!(
            call_context.call_callee_range(),
            Some(TextRange::new(
                expected_callee_start,
                expected_callee_start + "grant".len()
            ))
        );
        assert_eq!(call_context.call_callee_text(), Some("grant"));
        assert_eq!(
            call_context.call_open_offset(),
            source.find("grant(").map(|index| index + "grant".len())
        );

        let method_call_offset =
            source.find("filter(player").expect("method call") + "filter(".len();
        let method_call_context = QueryContext::from_databases(
            &databases,
            &document,
            LineIndex::new(source).position(method_call_offset),
        )
        .expect("method call query");
        let method_receiver_start = source.find("scores.filter").expect("method receiver");
        assert_eq!(
            method_call_context.call_member_receiver_range(),
            Some(TextRange::new(
                method_receiver_start,
                method_receiver_start + "scores".len()
            ))
        );
        assert_eq!(
            method_call_context.call_member_receiver_text(),
            Some("scores")
        );

        let lambda_offset = source.find("|)").expect("lambda pipe") + "|".len();
        let lambda_context = QueryContext::from_databases(
            &databases,
            &document,
            LineIndex::new(source).position(lambda_offset),
        )
        .expect("lambda query");
        let expected_method_start = source.find(".map").expect("lambda method") + ".".len();
        assert_eq!(
            lambda_context.lambda_method_range(),
            Some(TextRange::new(
                expected_method_start,
                expected_method_start + "map".len()
            ))
        );
        assert_eq!(
            lambda_context.call_open_offset(),
            source.find("map(").map(|index| index + "map".len())
        );
        assert_eq!(lambda_context.lambda_method_text(), Some("map"));
    }
}
