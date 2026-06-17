use vela_syntax::ast::SourceFile;

use crate::callable_context::{
    CallableFacts, callable_facts, member_callable_facts, source_callable_facts,
};
use crate::{
    CursorContext, CursorContextKind, DocumentId, DocumentSnapshot, LanguageServiceDatabases,
    Position, SourceRecord, SourceVersion, TextRange, WorkspaceGeneration, WorkspaceSnapshot,
    cursor_context_at,
};
use vela_analysis::facts::AnalysisFacts;
use vela_analysis::registry::RegistryFacts;
use vela_analysis::type_fact::TypeFact;
use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingMap, BindingResolution, LocalBinding};
use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph, ModulePath};
use vela_hir::type_hint::HirTypeHint;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CallArgumentFacts<'a> {
    callee_range: TextRange,
    callee: &'a str,
    call_open_offset: usize,
    args_prefix: &'a str,
    active_parameter: usize,
    member_receiver: Option<TextRange>,
}

impl<'a> CallArgumentFacts<'a> {
    #[must_use]
    pub const fn callee_range(&self) -> TextRange {
        self.callee_range
    }

    #[must_use]
    pub const fn callee(&self) -> &'a str {
        self.callee
    }

    #[must_use]
    pub const fn call_open_offset(&self) -> usize {
        self.call_open_offset
    }

    #[must_use]
    pub const fn args_prefix(&self) -> &'a str {
        self.args_prefix
    }

    #[must_use]
    pub const fn active_parameter(&self) -> usize {
        self.active_parameter
    }

    #[must_use]
    pub const fn member_receiver(&self) -> Option<TextRange> {
        self.member_receiver
    }
}

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
    pub fn type_fact_for_range(
        &self,
        databases: &LanguageServiceDatabases,
        range: TextRange,
    ) -> Option<TypeFact> {
        let source_id = self.source_id()?;
        type_fact_for_source_range(databases, source_id, range)
    }

    #[must_use]
    pub fn source_callable_facts(
        &self,
        databases: &LanguageServiceDatabases,
        callee: &str,
    ) -> Vec<CallableFacts> {
        source_callable_facts(databases, callee)
    }

    #[must_use]
    pub fn callable_facts(
        &self,
        databases: &LanguageServiceDatabases,
        callee: &str,
    ) -> Vec<CallableFacts> {
        callable_facts(databases, callee)
    }

    #[must_use]
    pub fn member_callable_facts(
        &self,
        databases: &LanguageServiceDatabases,
        receiver_range: TextRange,
        method: &str,
        args_prefix: &str,
    ) -> Vec<CallableFacts> {
        let Some(source_id) = self.source_id() else {
            return Vec::new();
        };
        member_callable_facts(databases, source_id, receiver_range, method, args_prefix)
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
    pub fn call_args_prefix_text(&self) -> Option<&str> {
        let open = self.call_open_offset()?;
        let end = self.cursor.replace_range().end;
        self.text().get(open + 1..end)
    }

    #[must_use]
    pub fn call_active_parameter_index(&self) -> Option<usize> {
        self.call_args_prefix_text()
            .map(active_call_parameter_index)
    }

    #[must_use]
    pub fn call_argument_facts(&self) -> Option<CallArgumentFacts<'_>> {
        if self.cursor.kind() != CursorContextKind::CallArgument {
            return None;
        }
        let callee_range = self.call_callee_range()?;
        let callee = text_range(self.text(), callee_range)?;
        let call_open_offset = self.call_open_offset()?;
        let args_prefix = self
            .text()
            .get(call_open_offset + 1..self.cursor.replace_range().end)?;
        Some(CallArgumentFacts {
            callee_range,
            callee,
            call_open_offset,
            args_prefix,
            active_parameter: active_call_parameter_index(args_prefix),
            member_receiver: self.call_member_receiver_range(),
        })
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

    #[must_use]
    pub const fn lambda_parameters_range(&self) -> Option<TextRange> {
        self.cursor.lambda_parameters()
    }

    #[must_use]
    pub fn lambda_parameters_text(&self) -> Option<&str> {
        text_range(self.text(), self.lambda_parameters_range()?)
    }
}

fn text_range(text: &str, range: TextRange) -> Option<&str> {
    text.get(range.start..range.end)
}

fn active_call_parameter_index(args_text: &str) -> usize {
    let mut depth = 0usize;
    let mut active = 0usize;
    let mut lambda_params = false;
    for ch in args_text.chars() {
        match ch {
            '|' => lambda_params = !lambda_params,
            '(' | '[' | '{' => depth = depth.saturating_add(1),
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 && !lambda_params => active = active.saturating_add(1),
            _ => {}
        }
    }
    active
}

fn type_fact_for_resolution(
    resolution: &BindingResolution,
    bindings: &BindingMap,
    facts: &AnalysisFacts,
    schema: &RegistryFacts,
) -> Option<TypeFact> {
    match resolution {
        BindingResolution::Local(local) => {
            let binding = bindings.local(*local)?;
            facts
                .local(*local)
                .cloned()
                .filter(|fact| !matches!(fact, TypeFact::Unknown))
                .or_else(|| schema_fact_for_local_hint(binding, schema))
        }
        BindingResolution::Declaration(declaration) => facts.declaration(*declaration).cloned(),
        BindingResolution::Import(_) | BindingResolution::QualifiedPath(_) => None,
    }
}

fn schema_fact_for_local_hint(binding: &LocalBinding, schema: &RegistryFacts) -> Option<TypeFact> {
    schema_fact_for_hint(binding.type_hint.as_ref()?, schema)
}

fn schema_fact_for_hint(hint: &HirTypeHint, schema: &RegistryFacts) -> Option<TypeFact> {
    if !hint.args.is_empty() {
        return None;
    }
    let qualified = hint.path.join("::");
    schema
        .type_fact(&qualified)
        .or_else(|| schema.trait_fact(&qualified))
        .or_else(|| hint.path.last().and_then(|name| schema.type_fact(name)))
        .or_else(|| hint.path.last().and_then(|name| schema.trait_fact(name)))
        .cloned()
}

pub(crate) fn type_fact_for_source_range(
    databases: &LanguageServiceDatabases,
    source_id: SourceId,
    range: TextRange,
) -> Option<TypeFact> {
    let start = u32::try_from(range.start).ok()?;
    let end = u32::try_from(range.end).ok()?;
    let span = Span::new(source_id, start, end);
    let graph = databases.hir_db().graph();
    let facts = AnalysisFacts::from_module_graph(graph);
    binding_maps_at(databases, source_id, start).find_map(|bindings| {
        let resolution = bindings.resolution_at_span(span)?;
        type_fact_for_resolution(resolution, bindings, &facts, databases.schema_db().facts())
    })
}

fn query_bindings<'a>(
    databases: &'a LanguageServiceDatabases,
    source: &SourceRecord,
    offset: usize,
) -> Option<&'a BindingMap> {
    let offset = u32::try_from(offset).ok()?;
    binding_maps_at(databases, source.source_id(), offset).next()
}

fn binding_maps_at<'a>(
    databases: &'a LanguageServiceDatabases,
    source_id: SourceId,
    offset: u32,
) -> impl Iterator<Item = &'a BindingMap> + 'a {
    let graph = databases.hir_db().graph();
    graph.declarations().filter_map(move |declaration| {
        if declaration.span.source != source_id || !declaration.span.contains(offset) {
            return None;
        }
        match declaration.kind {
            DeclarationKind::Function => graph.bindings(declaration.id),
            DeclarationKind::Trait => bindings_for_trait_method(graph, declaration.id, offset),
            DeclarationKind::Impl => bindings_for_impl_method(graph, declaration.id, offset),
            DeclarationKind::Const
            | DeclarationKind::Struct
            | DeclarationKind::Enum
            | DeclarationKind::Global => None,
        }
    })
}

fn bindings_for_trait_method(
    graph: &ModuleGraph,
    declaration: HirDeclId,
    offset: u32,
) -> Option<&BindingMap> {
    graph
        .trait_shape(declaration)?
        .methods
        .iter()
        .find_map(|method| {
            let body_span = method.default_body_span?;
            body_span
                .contains(offset)
                .then(|| {
                    method
                        .default_body_node
                        .and_then(|node| graph.trait_default_method_bindings(node))
                })
                .flatten()
        })
}

fn bindings_for_impl_method(
    graph: &ModuleGraph,
    declaration: HirDeclId,
    offset: u32,
) -> Option<&BindingMap> {
    graph
        .impl_metadata(declaration)?
        .methods
        .iter()
        .find_map(|method| {
            method
                .span
                .contains(offset)
                .then(|| graph.impl_method_bindings(method.node))
                .flatten()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::callable_context::CallableOrigin;
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
        assert_eq!(call_context.call_args_prefix_text(), Some("c"));
        assert_eq!(call_context.call_active_parameter_index(), Some(0));
        let call_facts = call_context.call_argument_facts().expect("call facts");
        assert_eq!(
            call_facts.callee_range(),
            call_context.call_callee_range().expect("callee")
        );
        assert_eq!(call_facts.callee(), "grant");
        assert_eq!(
            call_facts.call_open_offset(),
            call_context.call_open_offset().expect("call open")
        );
        assert_eq!(call_facts.args_prefix(), "c");
        assert_eq!(call_facts.active_parameter(), 0);
        assert_eq!(call_facts.member_receiver(), None);

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
        assert_eq!(
            method_call_context
                .type_fact_for_range(
                    &databases,
                    method_call_context
                        .call_member_receiver_range()
                        .expect("call member receiver")
                )
                .map(|fact| fact.display_name()),
            Some("Array(i64)".to_owned())
        );
        assert_eq!(method_call_context.call_args_prefix_text(), Some(""));
        assert_eq!(method_call_context.call_active_parameter_index(), Some(0));
        let method_call_facts = method_call_context
            .call_argument_facts()
            .expect("method call facts");
        assert_eq!(method_call_facts.callee(), "scores.filter");
        assert_eq!(method_call_facts.args_prefix(), "");
        assert_eq!(method_call_facts.active_parameter(), 0);
        assert_eq!(
            method_call_facts.member_receiver(),
            method_call_context.call_member_receiver_range()
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
        assert_eq!(
            lambda_context.lambda_parameters_range(),
            Some(TextRange::new(lambda_offset, lambda_offset))
        );
        assert_eq!(lambda_context.lambda_parameters_text(), Some(""));
        assert_eq!(lambda_context.call_argument_facts(), None);
    }

    #[test]
    fn query_context_exposes_active_call_parameter_index() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let source = "pub fn main(player: Player) { grant(player, current_player().level, map(|left, right| left), final); }";
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let workspace = Workspace::new();
        let files = vec![SourceFileSnapshot::new(document.clone(), source)];
        let project = assemble_project_sources(&config, &files, &workspace.snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        let second_arg_offset = source.find("current_player").expect("second arg") + "c".len();
        let second_arg_context = QueryContext::from_databases(
            &databases,
            &document,
            LineIndex::new(source).position(second_arg_offset),
        )
        .expect("second arg query");
        assert_eq!(
            second_arg_context.call_args_prefix_text(),
            Some("player, c")
        );
        assert_eq!(second_arg_context.call_active_parameter_index(), Some(1));
        let second_arg_facts = second_arg_context
            .call_argument_facts()
            .expect("second arg facts");
        assert_eq!(second_arg_facts.args_prefix(), "player, c");
        assert_eq!(second_arg_facts.active_parameter(), 1);

        let after_lambda_offset = source.find("final").expect("outer final arg") + "f".len();
        let after_lambda_context = QueryContext::from_databases(
            &databases,
            &document,
            LineIndex::new(source).position(after_lambda_offset),
        )
        .expect("after lambda query");
        assert_eq!(
            after_lambda_context.call_args_prefix_text(),
            Some("player, current_player().level, map(|left, right| left), f")
        );
        assert_eq!(after_lambda_context.call_active_parameter_index(), Some(3));
        let after_lambda_facts = after_lambda_context
            .call_argument_facts()
            .expect("after lambda facts");
        assert_eq!(
            after_lambda_facts.args_prefix(),
            "player, current_player().level, map(|left, right| left), f"
        );
        assert_eq!(after_lambda_facts.active_parameter(), 3);
    }

    #[test]
    fn query_context_exposes_source_callable_facts() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let source = "enum QuestState { Finished(quest_id: String) }\nfn grant(player: Player, amount: i64) -> bool { return true }\nfn main(player: Player) { grant(player, ) }";
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let workspace = Workspace::new();
        let files = vec![SourceFileSnapshot::new(document.clone(), source)];
        let project = assemble_project_sources(&config, &files, &workspace.snapshot());
        let mut databases = LanguageServiceDatabases::new();
        let mut schema = vela_analysis::registry::RegistryFacts::default();
        schema.insert_type("Player", TypeFact::host("Player"));
        schema.insert_function(
            "host::spawn",
            TypeFact::function(vec![TypeFact::STRING], TypeFact::host("Player")),
        );
        schema.insert_type(
            "QuestProgress",
            TypeFact::enum_type("QuestProgress", None::<String>),
        );
        schema.insert_variant(
            "QuestProgress",
            "Active",
            TypeFact::enum_type("QuestProgress", Some("Active")),
        );
        schema.insert_field("QuestProgress::Active", "0", TypeFact::STRING);
        schema.insert_field("QuestProgress::Active", "1", TypeFact::I64);
        schema.insert_variant(
            "QuestProgress",
            "Named",
            TypeFact::enum_type("QuestProgress", Some("Named")),
        );
        schema.insert_field("QuestProgress::Named", "quest_id", TypeFact::STRING);
        databases.set_schema_facts(schema);
        databases.update(&project);
        let position =
            LineIndex::new(source).position(source.find(", )").expect("argument hole") + 2);
        let context = QueryContext::from_databases(&databases, &document, position)
            .expect("database document exists");

        let callables = context.source_callable_facts(&databases, "grant");

        assert_eq!(callables.len(), 1);
        assert_eq!(callables[0].name(), "grant");
        assert_eq!(callables[0].returns().display_name(), "bool");
        assert_eq!(callables[0].params()[0].name(), "player");
        assert_eq!(
            callables[0].params()[0].type_fact().display_name(),
            "Player"
        );
        assert_eq!(callables[0].params()[1].name(), "amount");
        assert_eq!(callables[0].params()[1].type_fact().display_name(), "i64");
        assert!(!callables[0].params()[1].defaulted());
        assert_eq!(callables[0].origin(), CallableOrigin::Source);

        let schema_callables = context.callable_facts(&databases, "spawn");
        let schema_callable = schema_callables
            .iter()
            .find(|callable| callable.origin() == CallableOrigin::Schema)
            .expect("schema function callable facts");
        assert_eq!(schema_callable.name(), "host::spawn");
        assert_eq!(schema_callable.returns().display_name(), "Player");
        assert_eq!(schema_callable.params()[0].name(), "arg0");
        assert_eq!(
            schema_callable.params()[0].type_fact().display_name(),
            "String"
        );

        let variant_callables = context.callable_facts(&databases, "Finished");
        let variant_callable = variant_callables
            .iter()
            .find(|callable| callable.origin() == CallableOrigin::SourceVariant)
            .expect("source enum variant callable facts");
        assert_eq!(variant_callable.name(), "game::main::QuestState::Finished");
        assert_eq!(
            variant_callable.returns(),
            &TypeFact::enum_type("game::main::QuestState", Some("Finished"))
        );
        assert_eq!(variant_callable.params()[0].name(), "quest_id");
        assert_eq!(
            variant_callable.params()[0].type_fact().display_name(),
            "String"
        );

        let schema_variant_callables = context.callable_facts(&databases, "Active");
        let schema_variant_callable = schema_variant_callables
            .iter()
            .find(|callable| callable.origin() == CallableOrigin::SchemaVariant)
            .expect("schema enum variant callable facts");
        assert_eq!(schema_variant_callable.name(), "QuestProgress::Active");
        assert_eq!(
            schema_variant_callable.returns(),
            &TypeFact::enum_type("QuestProgress", Some("Active"))
        );
        assert_eq!(schema_variant_callable.params()[0].name(), "arg0");
        assert_eq!(
            schema_variant_callable.params()[0]
                .type_fact()
                .display_name(),
            "String"
        );
        assert_eq!(schema_variant_callable.params()[1].name(), "arg1");
        assert!(
            context
                .callable_facts(&databases, "Named")
                .iter()
                .all(|callable| callable.origin() != CallableOrigin::SchemaVariant)
        );

        let stdlib_callables = context.callable_facts(&databases, "max");
        let stdlib_callable = stdlib_callables
            .iter()
            .find(|callable| callable.origin() == CallableOrigin::Stdlib)
            .expect("stdlib function callable facts");
        assert_eq!(stdlib_callable.name(), "math::max");
        assert_eq!(stdlib_callable.params()[0].name(), "arg0");
        assert_eq!(
            stdlib_callable.params()[0].type_fact().display_name(),
            "i64 | f64"
        );
    }

    #[test]
    fn query_context_exposes_member_callable_facts() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let source = "struct Player { level: i64 }\n\
                      impl Player { fn grant(self, amount: i64, bonus: i64) -> i64 { return amount + bonus } }\n\
                      fn main(player: Player, enemy: Enemy, scores: Array<i64>) { player.grant(1, ); enemy.preview(1, ); scores.filter(|score| score > 0) }";
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let workspace = Workspace::new();
        let files = vec![SourceFileSnapshot::new(document.clone(), source)];
        let project = assemble_project_sources(&config, &files, &workspace.snapshot());
        let mut databases = LanguageServiceDatabases::new();
        let mut schema = vela_analysis::registry::RegistryFacts::default();
        schema.insert_type("Enemy", TypeFact::host("Enemy"));
        schema.insert_method(
            "Enemy",
            "preview",
            TypeFact::function(vec![TypeFact::I64, TypeFact::I64], TypeFact::BOOL),
        );
        databases.set_schema_facts(schema);
        databases.update(&project);
        let line_index = LineIndex::new(source);

        let source_position =
            line_index.position(source.find("grant(1, )").expect("source method call") + 9);
        let source_context = QueryContext::from_databases(&databases, &document, source_position)
            .expect("source method query");
        let source_call = source_context
            .call_argument_facts()
            .expect("source method call facts");
        let (_, source_method) = source_call
            .callee()
            .rsplit_once('.')
            .expect("source method callee");
        let source_callables = source_context.member_callable_facts(
            &databases,
            source_call
                .member_receiver()
                .expect("source method receiver"),
            source_method,
            source_call.args_prefix(),
        );
        let source_callable = source_callables
            .iter()
            .find(|callable| callable.origin() == CallableOrigin::SourceMethod)
            .expect("source method callable facts");
        assert_eq!(source_callable.name(), "Player.grant");
        assert_eq!(source_callable.returns().display_name(), "i64");
        assert_eq!(source_callable.params()[0].name(), "amount");
        assert_eq!(
            source_callable.params()[0].type_fact().display_name(),
            "i64"
        );
        assert_eq!(source_callable.params()[1].name(), "bonus");

        let schema_position =
            line_index.position(source.find("preview(1, )").expect("schema method call") + 11);
        let schema_context = QueryContext::from_databases(&databases, &document, schema_position)
            .expect("schema method query");
        let schema_call = schema_context
            .call_argument_facts()
            .expect("schema method call facts");
        let (_, schema_method) = schema_call
            .callee()
            .rsplit_once('.')
            .expect("schema method callee");
        let schema_callables = schema_context.member_callable_facts(
            &databases,
            schema_call
                .member_receiver()
                .expect("schema method receiver"),
            schema_method,
            schema_call.args_prefix(),
        );
        let schema_callable = schema_callables
            .iter()
            .find(|callable| callable.origin() == CallableOrigin::SchemaMethod)
            .expect("schema method callable facts");
        assert_eq!(schema_callable.name(), "Enemy.preview");
        assert_eq!(schema_callable.returns().display_name(), "bool");
        assert_eq!(schema_callable.params()[1].name(), "arg1");

        let stdlib_position =
            line_index.position(source.find("score >").expect("stdlib lambda body"));
        let stdlib_context = QueryContext::from_databases(&databases, &document, stdlib_position)
            .expect("stdlib method query");
        let stdlib_call = stdlib_context
            .call_argument_facts()
            .expect("stdlib method call facts");
        let (_, stdlib_method) = stdlib_call
            .callee()
            .rsplit_once('.')
            .expect("stdlib method callee");
        let stdlib_callables = stdlib_context.member_callable_facts(
            &databases,
            stdlib_call
                .member_receiver()
                .expect("stdlib method receiver"),
            stdlib_method,
            stdlib_call.args_prefix(),
        );
        let stdlib_callable = stdlib_callables
            .iter()
            .find(|callable| callable.origin() == CallableOrigin::StdlibMethod)
            .expect("stdlib method callable facts");
        assert_eq!(stdlib_callable.name(), "Array(i64).filter");
        assert_eq!(stdlib_callable.params()[0].name(), "callback");
        assert_eq!(
            stdlib_callable.params()[0].type_fact().display_name(),
            "Function(i64) -> bool"
        );
        assert_eq!(stdlib_callable.returns().display_name(), "Array(i64)");
    }
}
