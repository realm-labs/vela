use vela_analysis::{
    completion::{
        CompletionItem as AnalysisCompletionItem, CompletionKind as AnalysisCompletionKind,
        declaration_completions, module_completions, type_completions,
    },
    facts::AnalysisFacts,
    registry::RegistryFacts,
    type_fact::TypeFact,
};
use vela_hir::module_graph::ModuleGraph;

use super::{CompletionInsertFormat, CompletionItem, CompletionKind, label_segment_matches};

pub(super) fn type_hint_completion_context(text: &str, prefix_start: usize) -> bool {
    let Some(before_prefix) = text.get(..prefix_start) else {
        return false;
    };
    let trimmed = before_prefix.trim_end();
    if trimmed.ends_with("::") {
        return false;
    }
    if trimmed.ends_with("->") {
        return true;
    }
    if trimmed.ends_with(':') && !trimmed.ends_with("::") {
        return type_annotation_left_side_is_plausible(trimmed);
    }
    inside_builtin_type_args(trimmed)
}

pub(super) fn type_hint_module_path_context(text: &str, prefix_start: usize) -> Option<String> {
    let before_prefix = text.get(..prefix_start)?;
    let trimmed = before_prefix.trim_end();
    let before_colons = trimmed.strip_suffix("::")?;
    let path_start = module_path_start(before_colons);
    let module_base = before_colons[path_start..].trim_matches(':');
    if module_base.is_empty() || !type_hint_completion_context(text, path_start) {
        return None;
    }
    Some(module_base.to_owned())
}

pub(super) fn type_hint_completion_items(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    prefix: &str,
    module_base: Option<&str>,
) -> Vec<CompletionItem> {
    let facts = AnalysisFacts::from_module_graph(graph);
    if let Some(module_base) = module_base {
        return qualified_type_hint_completion_items(graph, schema, &facts, prefix, module_base);
    }
    let mut items = builtin_type_hint_completions();
    items.extend(
        type_completions(schema)
            .into_iter()
            .map(service_item_from_analysis),
    );
    items.extend(
        declaration_completions(graph, &facts)
            .into_iter()
            .filter(|item| {
                matches!(
                    item.kind,
                    AnalysisCompletionKind::Type | AnalysisCompletionKind::Trait
                )
            })
            .map(service_item_from_analysis),
    );
    items.extend(
        module_completions(graph)
            .into_iter()
            .map(service_item_from_analysis),
    );
    super::dedupe_and_filter_service_items(items, |item| {
        label_segment_matches(item.label(), prefix)
    })
}

fn qualified_type_hint_completion_items(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    facts: &AnalysisFacts,
    prefix: &str,
    module_base: &str,
) -> Vec<CompletionItem> {
    let mut items = type_completions(schema);
    items.extend(
        declaration_completions(graph, facts)
            .into_iter()
            .filter(is_type_position_analysis_item),
    );
    items.extend(module_completions(graph));
    super::dedupe_and_filter_service_items(
        items
            .into_iter()
            .filter_map(|item| service_item_for_qualified_type_path(item, module_base, prefix))
            .collect(),
        |item| label_segment_matches(item.label(), prefix),
    )
}

fn service_item_for_qualified_type_path(
    item: AnalysisCompletionItem,
    module_base: &str,
    prefix: &str,
) -> Option<CompletionItem> {
    if !is_type_position_analysis_item(&item) {
        return None;
    }
    let suffix = item
        .label
        .strip_prefix(module_base)
        .and_then(|suffix| suffix.strip_prefix("::"))?;
    if !suffix.starts_with(prefix) {
        return None;
    }
    let label = suffix
        .split_once("::")
        .map_or(suffix, |(segment, _)| segment)
        .to_owned();
    Some(CompletionItem {
        label,
        kind: CompletionKind::from(item.kind),
        detail: item.fact.display_name(),
        insert_text: None,
        insert_format: CompletionInsertFormat::PlainText,
        sort_text: None,
    })
}

fn is_type_position_analysis_item(item: &AnalysisCompletionItem) -> bool {
    matches!(
        item.kind,
        AnalysisCompletionKind::Type
            | AnalysisCompletionKind::Trait
            | AnalysisCompletionKind::Module
    )
}

fn builtin_type_hint_completions() -> Vec<CompletionItem> {
    [
        ("bool", TypeFact::BOOL),
        ("char", TypeFact::CHAR),
        ("i8", TypeFact::I8),
        ("i16", TypeFact::I16),
        ("i32", TypeFact::I32),
        ("i64", TypeFact::I64),
        ("u8", TypeFact::U8),
        ("u16", TypeFact::U16),
        ("u32", TypeFact::U32),
        ("u64", TypeFact::U64),
        ("f32", TypeFact::F32),
        ("f64", TypeFact::F64),
        ("String", TypeFact::STRING),
        ("Bytes", TypeFact::BYTES),
        ("Array", TypeFact::array(TypeFact::Unknown)),
        ("Map", TypeFact::map(TypeFact::Unknown, TypeFact::Unknown)),
        ("Set", TypeFact::set(TypeFact::Unknown)),
        ("Iterator", TypeFact::iterator(TypeFact::Unknown)),
        ("Option", TypeFact::option(TypeFact::Unknown)),
        (
            "Result",
            TypeFact::result(TypeFact::Unknown, TypeFact::Unknown),
        ),
    ]
    .into_iter()
    .map(|(label, fact)| CompletionItem {
        label: label.to_owned(),
        kind: CompletionKind::Type,
        detail: fact.display_name(),
        insert_text: None,
        insert_format: CompletionInsertFormat::PlainText,
        sort_text: None,
    })
    .collect()
}

fn service_item_from_analysis(item: AnalysisCompletionItem) -> CompletionItem {
    CompletionItem {
        label: item.label,
        kind: CompletionKind::from(item.kind),
        detail: item.fact.display_name(),
        insert_text: None,
        insert_format: CompletionInsertFormat::PlainText,
        sort_text: None,
    }
}

fn type_annotation_left_side_is_plausible(trimmed: &str) -> bool {
    let Some(colon) = trimmed.rfind(':') else {
        return false;
    };
    let start = trimmed[..colon]
        .char_indices()
        .rev()
        .find_map(|(index, ch)| {
            matches!(ch, '\n' | '(' | '{' | ',' | ';').then_some(index + ch.len_utf8())
        })
        .unwrap_or(0);
    trimmed[start..colon]
        .split_whitespace()
        .last()
        .is_some_and(is_identifier)
}

fn inside_builtin_type_args(trimmed: &str) -> bool {
    let Some(open) = unmatched_type_arg_open(trimmed) else {
        return false;
    };
    let owner_end = open;
    let owner_start = trimmed[..owner_end]
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    let owner = &trimmed[owner_start..owner_end];
    matches!(
        owner,
        "Array" | "Map" | "Set" | "Iterator" | "Option" | "Result"
    ) && trimmed[open + 1..]
        .chars()
        .all(|ch| is_identifier_continue(ch) || matches!(ch, ':' | ',' | '<' | '>' | ' '))
}

fn unmatched_type_arg_open(trimmed: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (index, ch) in trimmed.char_indices().rev() {
        match ch {
            '>' => depth = depth.saturating_add(1),
            '<' if depth == 0 => return Some(index),
            '<' => depth = depth.saturating_sub(1),
            '\n' | ';' | '{' | '}' | '(' | ')' => return None,
            _ => {}
        }
    }
    None
}

fn module_path_start(before_colons: &str) -> usize {
    before_colons
        .char_indices()
        .rev()
        .find_map(|(index, ch)| {
            (!is_identifier_continue(ch) && ch != ':').then_some(index + ch.len_utf8())
        })
        .unwrap_or(0)
}

fn is_identifier(value: &str) -> bool {
    !value.is_empty() && value.chars().all(is_identifier_continue)
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use vela_analysis::{registry::RegistryFacts, type_fact::TypeFact};

    use super::*;
    use crate::{
        DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, Workspace,
        WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
        completion::{CompletionContextKind, CompletionList},
    };

    #[test]
    fn type_hint_completion_suggests_only_type_items() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub struct Player { level: i64 }\npub fn helper() { return 1 }\npub fn main(player: Pl) { return 1 }";
        let mut databases = databases_for(document.clone(), text);
        let mut schema = RegistryFacts::default();
        schema.insert_type("Planet", TypeFact::host("Planet"));
        schema.insert_function("play", TypeFact::function(Vec::new(), TypeFact::NULL));
        databases.set_schema_facts(schema);
        databases.update(&project_for(document.clone(), text));

        let completions = databases.completion_items(
            &document,
            Position::new(
                2,
                text.lines()
                    .nth(2)
                    .expect("main line")
                    .find("Pl)")
                    .expect("type prefix")
                    + "Pl".len(),
            ),
        );

        assert_eq!(
            completions.context().kind(),
            CompletionContextKind::TypeHint
        );
        assert_completion(&completions, "game::main::Player", CompletionKind::Type);
        assert_completion(&completions, "Planet", CompletionKind::Type);
        assert_no_completion(&completions, "game::main::helper");
        assert_no_completion(&completions, "play");
    }

    #[test]
    fn type_hint_completion_suggests_builtin_container_arguments() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub enum QuestState { Started, Done }\npub fn main(rewards: Map<QuestState, i) { return 1 }";
        let databases = databases_for(document.clone(), text);
        let completions = databases.completion_items(
            &document,
            Position::new(
                1,
                text.lines()
                    .nth(1)
                    .expect("main line")
                    .find("i)")
                    .expect("type arg prefix")
                    + "i".len(),
            ),
        );

        assert_eq!(
            completions.context().kind(),
            CompletionContextKind::TypeHint
        );
        assert_completion(&completions, "i64", CompletionKind::Type);
    }

    #[test]
    fn type_hint_completion_suggests_modules() {
        let main = DocumentId::from("/workspace/scripts/game/main.vela");
        let reward = DocumentId::from("/workspace/scripts/game/reward.vela");
        let files = vec![
            SourceFileSnapshot::new(main.clone(), "pub fn main(item: ga) { return 1 }"),
            SourceFileSnapshot::new(reward, "pub struct Reward { amount: i64 }"),
        ];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        let text = files[0].text();
        let completions = databases.completion_items(
            &main,
            Position::new(0, text.find("ga)").expect("module prefix") + "ga".len()),
        );

        assert_eq!(
            completions.context().kind(),
            CompletionContextKind::TypeHint
        );
        assert_completion(&completions, "game", CompletionKind::Module);
    }

    #[test]
    fn qualified_type_hint_completion_suggests_only_type_path_items() {
        let main = DocumentId::from("/workspace/scripts/game/main.vela");
        let reward = DocumentId::from("/workspace/scripts/game/reward.vela");
        let files = vec![
            SourceFileSnapshot::new(
                main.clone(),
                "pub fn main(item: game::reward::Re) { return 1 }",
            ),
            SourceFileSnapshot::new(
                reward,
                "pub struct Reward { amount: i64 }\npub fn redeem() { return 1 }",
            ),
        ];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        let text = files[0].text();
        let completions = databases.completion_items(
            &main,
            Position::new(0, text.find("Re)").expect("type prefix") + "Re".len()),
        );

        assert_eq!(
            completions.context().kind(),
            CompletionContextKind::TypeHint
        );
        assert_eq!(completions.context().module_base(), Some("game::reward"));
        assert_completion(&completions, "Reward", CompletionKind::Type);
        assert_no_completion(&completions, "redeem");
    }

    fn databases_for(document: DocumentId, text: &str) -> LanguageServiceDatabases {
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project_for(document, text));
        databases
    }

    fn project_for(document: DocumentId, text: &str) -> crate::ProjectSources {
        let files = vec![SourceFileSnapshot::new(document, text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        assemble_project_sources(&config, &files, &Workspace::new().snapshot())
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
