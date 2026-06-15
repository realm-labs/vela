use std::collections::BTreeMap;

use vela_analysis::completion::{
    CompletionItem as AnalysisCompletionItem, CompletionKind as AnalysisCompletionKind,
    declaration_completions, global_completions, member_completions, module_completions,
};
use vela_analysis::facts::AnalysisFacts;
use vela_analysis::type_fact::TypeFact;
use vela_common::Span;
use vela_hir::binding::{BindingMap, BindingResolution, LocalBinding};

use crate::{DocumentId, LanguageServiceDatabases, LineIndex, Position, TextRange};

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub enum CompletionKind {
    Binding,
    Const,
    Field,
    Method,
    Module,
    Variant,
    Function,
    Type,
    Trait,
}

impl From<AnalysisCompletionKind> for CompletionKind {
    fn from(value: AnalysisCompletionKind) -> Self {
        match value {
            AnalysisCompletionKind::Binding => Self::Binding,
            AnalysisCompletionKind::Const => Self::Const,
            AnalysisCompletionKind::Field => Self::Field,
            AnalysisCompletionKind::Method => Self::Method,
            AnalysisCompletionKind::Module => Self::Module,
            AnalysisCompletionKind::Variant => Self::Variant,
            AnalysisCompletionKind::Function => Self::Function,
            AnalysisCompletionKind::Type => Self::Type,
            AnalysisCompletionKind::Trait => Self::Trait,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CompletionContextKind {
    Global,
    ModulePath,
    Member,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CompletionItem {
    label: String,
    kind: CompletionKind,
    detail: String,
}

impl CompletionItem {
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn kind(&self) -> CompletionKind {
        self.kind
    }

    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CompletionContext {
    kind: CompletionContextKind,
    prefix: String,
    replace_range: TextRange,
    module_base: Option<String>,
    member_receiver: Option<MemberReceiver>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct MemberReceiver {
    range: TextRange,
}

impl CompletionContext {
    #[must_use]
    pub const fn kind(&self) -> CompletionContextKind {
        self.kind
    }

    #[must_use]
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    #[must_use]
    pub const fn replace_range(&self) -> TextRange {
        self.replace_range
    }

    #[must_use]
    pub fn module_base(&self) -> Option<&str> {
        self.module_base.as_deref()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CompletionList {
    context: CompletionContext,
    items: Vec<CompletionItem>,
}

impl CompletionList {
    #[must_use]
    pub fn context(&self) -> &CompletionContext {
        &self.context
    }

    #[must_use]
    pub fn items(&self) -> &[CompletionItem] {
        &self.items
    }
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn completion_items(&self, document_id: &DocumentId, position: Position) -> CompletionList {
        let Some(source) = self.source_db().records().get(document_id) else {
            return empty_completion_list(CompletionContext::global(0, ""));
        };
        let context = completion_context(source.text(), position);
        let items = match context.kind {
            CompletionContextKind::Global => self.global_completion_items(&context),
            CompletionContextKind::ModulePath => self.module_path_completion_items(&context),
            CompletionContextKind::Member => self.member_completion_items(document_id, &context),
        };
        CompletionList { context, items }
    }

    fn global_completion_items(&self, context: &CompletionContext) -> Vec<CompletionItem> {
        let graph = self.hir_db().graph();
        let facts = AnalysisFacts::from_module_graph(graph);
        let mut items = global_completions(self.schema_db().facts());
        items.extend(declaration_completions(graph, &facts));
        items.extend(module_completions(graph));
        dedupe_and_filter_items(items, |item| {
            label_segment_matches(&item.label, context.prefix())
        })
    }

    fn module_path_completion_items(&self, context: &CompletionContext) -> Vec<CompletionItem> {
        let graph = self.hir_db().graph();
        let facts = AnalysisFacts::from_module_graph(graph);
        let Some(base) = context.module_base() else {
            return Vec::new();
        };
        let mut items = declaration_completions(graph, &facts);
        items.extend(module_completions(graph));
        dedupe_and_filter_items(items, |item| {
            item.label
                .strip_prefix(base)
                .and_then(|suffix| suffix.strip_prefix("::"))
                .is_some_and(|suffix| suffix.starts_with(context.prefix()))
        })
    }

    fn member_completion_items(
        &self,
        document_id: &DocumentId,
        context: &CompletionContext,
    ) -> Vec<CompletionItem> {
        let Some(receiver) = context.member_receiver.as_ref() else {
            return Vec::new();
        };
        let Some(receiver_fact) = self.member_receiver_fact(document_id, receiver) else {
            return Vec::new();
        };
        dedupe_and_filter_items(
            member_completions(self.schema_db().facts(), &receiver_fact),
            |item| label_segment_matches(&item.label, context.prefix()),
        )
    }

    fn member_receiver_fact(
        &self,
        document_id: &DocumentId,
        receiver: &MemberReceiver,
    ) -> Option<TypeFact> {
        let source = self.source_db().records().get(document_id)?;
        let source_id = source.source_id();
        let start = u32::try_from(receiver.range.start).ok()?;
        let end = u32::try_from(receiver.range.end).ok()?;
        let receiver_span = Span::new(source_id, start, end);
        let graph = self.hir_db().graph();
        let facts = AnalysisFacts::from_module_graph(graph);

        graph.declarations().find_map(|declaration| {
            if declaration.span.source != source_id || !declaration.span.contains(start) {
                return None;
            }
            let bindings = graph.bindings(declaration.id)?;
            let resolution = bindings.resolution_at_span(receiver_span)?;
            type_fact_for_resolution(resolution, bindings, &facts, self.schema_db().facts())
        })
    }
}

impl CompletionContext {
    fn global(prefix_start: usize, prefix: &str) -> Self {
        Self {
            kind: CompletionContextKind::Global,
            prefix: prefix.to_owned(),
            replace_range: TextRange::new(prefix_start, prefix_start + prefix.len()),
            module_base: None,
            member_receiver: None,
        }
    }
}

fn completion_context(text: &str, position: Position) -> CompletionContext {
    let offset = LineIndex::new(text).offset(position);
    let prefix_start = identifier_prefix_start(text, offset);
    let prefix = &text[prefix_start..offset];
    let before_prefix = &text[..prefix_start];

    if before_prefix.ends_with('.') {
        let member_receiver = member_receiver_before_dot(before_prefix);
        return CompletionContext {
            kind: CompletionContextKind::Member,
            prefix: prefix.to_owned(),
            replace_range: TextRange::new(prefix_start, offset),
            module_base: None,
            member_receiver,
        };
    }

    if let Some(module_base) = module_base_before_colons(before_prefix) {
        return CompletionContext {
            kind: CompletionContextKind::ModulePath,
            prefix: prefix.to_owned(),
            replace_range: TextRange::new(prefix_start, offset),
            module_base: Some(module_base),
            member_receiver: None,
        };
    }

    CompletionContext::global(prefix_start, prefix)
}

fn identifier_prefix_start(text: &str, offset: usize) -> usize {
    text[..offset]
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0)
}

fn module_base_before_colons(before_prefix: &str) -> Option<String> {
    let before_colons = before_prefix.strip_suffix("::")?;
    let start = before_colons
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_module_path_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    let module_base = before_colons[start..].trim_matches(':');
    (!module_base.is_empty()).then(|| module_base.to_owned())
}

fn member_receiver_before_dot(before_prefix: &str) -> Option<MemberReceiver> {
    let before_dot = before_prefix.strip_suffix('.')?;
    let end = before_dot.len();
    let start = before_dot
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    (start < end).then(|| MemberReceiver {
        range: TextRange::new(start, end),
    })
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn is_module_path_continue(ch: char) -> bool {
    is_identifier_continue(ch) || ch == ':'
}

fn dedupe_and_filter_items(
    items: Vec<AnalysisCompletionItem>,
    matches_context: impl Fn(&AnalysisCompletionItem) -> bool,
) -> Vec<CompletionItem> {
    let mut deduped = BTreeMap::new();
    for item in items.into_iter().filter(matches_context) {
        deduped
            .entry((item.label.clone(), CompletionKind::from(item.kind)))
            .or_insert_with(|| CompletionItem {
                label: item.label,
                kind: item.kind.into(),
                detail: item.fact.display_name(),
            });
    }
    deduped.into_values().collect()
}

fn label_segment_matches(label: &str, prefix: &str) -> bool {
    prefix.is_empty()
        || label.starts_with(prefix)
        || label
            .rsplit("::")
            .next()
            .is_some_and(|segment| segment.starts_with(prefix))
}

fn type_fact_for_resolution(
    resolution: &BindingResolution,
    bindings: &BindingMap,
    facts: &AnalysisFacts,
    schema: &vela_analysis::registry::RegistryFacts,
) -> Option<TypeFact> {
    match resolution {
        BindingResolution::Local(local) => {
            let binding = bindings.local(*local)?;
            let fact = facts.local(*local).cloned();
            fact.filter(|fact| !matches!(fact, TypeFact::Unknown))
                .or_else(|| schema_fact_for_local_hint(binding, schema))
        }
        BindingResolution::Declaration(declaration) => facts.declaration(*declaration).cloned(),
        BindingResolution::Import(_) | BindingResolution::QualifiedPath(_) => None,
    }
}

fn schema_fact_for_local_hint(
    binding: &LocalBinding,
    schema: &vela_analysis::registry::RegistryFacts,
) -> Option<TypeFact> {
    let hint = binding.type_hint.as_ref()?;
    if hint.args.is_empty() {
        let qualified = hint.path.join("::");
        schema
            .type_fact(&qualified)
            .or_else(|| hint.path.last().and_then(|name| schema.type_fact(name)))
            .cloned()
    } else {
        None
    }
}

fn empty_completion_list(context: CompletionContext) -> CompletionList {
    CompletionList {
        context,
        items: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use vela_analysis::registry::RegistryFacts;
    use vela_analysis::type_fact::TypeFact;

    use super::*;
    use crate::{
        SourceFileSnapshot, SourceVersion, Workspace, WorkspaceConfig, WorkspaceRoot,
        assemble_project_sources,
    };

    #[test]
    fn completion_uses_open_overlay_facts() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let files = vec![SourceFileSnapshot::new(
            document.clone(),
            "pub fn disk_only() { return 1 }",
        )];
        let mut workspace = Workspace::new();
        workspace.open_document(
            document.clone(),
            "pub fn overlay_only() { return 2 }",
            SourceVersion::new(2),
        );
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &workspace.snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        let completions = databases.completion_items(&document, Position::new(0, 7));

        assert_completion(
            &completions,
            "game::main::overlay_only",
            CompletionKind::Function,
        );
        assert_no_completion(&completions, "game::main::disk_only");
    }

    #[test]
    fn global_completion_uses_schema_facts() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let files = vec![SourceFileSnapshot::new(
            document.clone(),
            "pub fn main() { Pla }",
        )];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        let mut schema = RegistryFacts::default();
        schema.insert_type("Player", TypeFact::host("Player"));
        schema.insert_function(
            "spawn_player",
            TypeFact::function(vec![TypeFact::STRING], TypeFact::host("Player")),
        );
        databases.set_schema_facts(schema);
        databases.update(&project);

        let completions = databases.completion_items(&document, Position::new(0, 18));

        assert_completion(&completions, "Player", CompletionKind::Type);
        assert_no_completion(&completions, "spawn_player");
    }

    #[test]
    fn member_completion_uses_host_schema_facts() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub fn main(player: Player) { player.le }";
        let files = vec![SourceFileSnapshot::new(document.clone(), text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        let mut schema = RegistryFacts::default();
        schema.insert_type("Player", TypeFact::host("Player"));
        schema.insert_field("Player", "level", TypeFact::I64);
        schema.insert_method(
            "Player",
            "level_up",
            TypeFact::function(vec![TypeFact::I64], TypeFact::BOOL),
        );
        databases.set_schema_facts(schema);
        databases.update(&project);

        let completions = databases.completion_items(
            &document,
            Position::new(0, text.find("le }").expect("member prefix") + "le".len()),
        );

        assert_eq!(completions.context().kind(), CompletionContextKind::Member);
        assert_completion(&completions, "level", CompletionKind::Field);
        assert_completion(&completions, "level_up", CompletionKind::Method);
    }

    #[test]
    fn module_completion_follows_import_context() {
        let main = DocumentId::from("/workspace/scripts/game/main.vela");
        let reward = DocumentId::from("/workspace/scripts/game/reward.vela");
        let files = vec![
            SourceFileSnapshot::new(main.clone(), "use game::r"),
            SourceFileSnapshot::new(reward, "pub fn grant() { return 1 }"),
        ];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        let completions = databases.completion_items(&main, Position::new(0, "use game::r".len()));

        assert_eq!(
            completions.context().kind(),
            CompletionContextKind::ModulePath
        );
        assert_eq!(completions.context().module_base(), Some("game"));
        assert_completion(&completions, "game::reward", CompletionKind::Module);
        assert_no_completion(&completions, "game::main");
    }

    #[test]
    fn member_context_is_detected_without_global_fallback() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let files = vec![SourceFileSnapshot::new(
            document.clone(),
            "pub fn main(player) { player.le }",
        )];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        let completions = databases.completion_items(&document, Position::new(0, 31));

        assert_eq!(completions.context().kind(), CompletionContextKind::Member);
        assert!(completions.items().is_empty(), "{completions:?}");
    }

    fn assert_completion(list: &CompletionList, label: &str, kind: CompletionKind) {
        assert!(
            list.items()
                .iter()
                .any(|item| item.label() == label && item.kind() == kind),
            "{list:?}"
        );
    }

    fn assert_no_completion(list: &CompletionList, label: &str) {
        assert!(
            list.items().iter().all(|item| item.label() != label),
            "{list:?}"
        );
    }
}
