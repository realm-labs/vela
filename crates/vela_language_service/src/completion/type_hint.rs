use vela_analysis::{
    completion::{
        CompletionItem as AnalysisCompletionItem, CompletionKind as AnalysisCompletionKind,
        declaration_completion, type_completions,
    },
    facts::AnalysisFacts,
    registry::RegistryFacts,
    type_fact::TypeFact,
};
use vela_hir::module_graph::{ModuleGraph, ModulePath};

use crate::{TextRange, symbol_ref::schema_symbol};

use super::{
    CompletionInsertFormat, CompletionItem, CompletionKind, display_type_detail_parts,
    label_segment_matches, type_display::type_completion_item,
};

pub(super) fn type_hint_completion_items(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    replace_range: TextRange,
    prefix: &str,
    module_base: Option<&str>,
) -> Vec<CompletionItem> {
    let facts = AnalysisFacts::from_module_graph(graph);
    if let Some(module_base) = module_base {
        return qualified_type_hint_completion_items(
            graph,
            schema,
            &facts,
            replace_range,
            prefix,
            module_base,
        );
    }
    let mut items = builtin_type_hint_completions();
    items.extend(
        type_completions(schema)
            .into_iter()
            .map(|item| service_item_from_schema_type(item, prefix)),
    );
    items.extend(
        graph
            .declarations_by_name_prefix(prefix)
            .into_iter()
            .filter_map(|declaration| declaration_completion(graph, &facts, declaration))
            .filter(|item| {
                matches!(
                    item.kind,
                    AnalysisCompletionKind::Type | AnalysisCompletionKind::Trait
                )
            })
            .map(|item| {
                let qualified_name = item.label.clone();
                type_completion_item(item, &qualified_name, prefix)
            }),
    );
    items.extend(
        graph
            .module_child_segments(&ModulePath::root())
            .into_iter()
            .filter(|segment| segment.starts_with(prefix))
            .map(|segment| {
                service_item_from_analysis(AnalysisCompletionItem {
                    label: segment.to_owned(),
                    kind: AnalysisCompletionKind::Module,
                    fact: TypeFact::module(segment),
                })
            }),
    );
    super::dedupe_and_filter_service_items(items, replace_range, prefix, |item| {
        label_segment_matches(item.label(), prefix)
    })
}

fn qualified_type_hint_completion_items(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    facts: &AnalysisFacts,
    replace_range: TextRange,
    prefix: &str,
    module_base: &str,
) -> Vec<CompletionItem> {
    let mut items = type_completions(schema);
    let module_path = ModulePath::from_qualified(module_base);
    if let Some(module) = graph.module_id(&module_path) {
        items.extend(
            graph
                .declarations_in_module(module)
                .into_iter()
                .filter_map(|declaration| declaration_completion(graph, facts, declaration))
                .filter(is_type_position_analysis_item),
        );
    }
    items.extend(
        graph
            .module_child_segments(&module_path)
            .into_iter()
            .map(|segment| AnalysisCompletionItem {
                label: format!("{module_base}::{segment}"),
                kind: AnalysisCompletionKind::Module,
                fact: TypeFact::module(format!("{module_base}::{segment}")),
            }),
    );
    super::dedupe_and_filter_service_items(
        items
            .into_iter()
            .filter_map(|item| service_item_for_qualified_type_path(item, module_base, prefix))
            .collect(),
        replace_range,
        prefix,
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
    let qualified_name = format!("{module_base}::{suffix}");
    let mut completion = type_completion_item(item, &qualified_name, prefix);
    completion.label = label;
    completion.insert_text = Some(completion.label.clone());
    Some(completion)
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
    .map(|(label, fact)| {
        let detail_parts = display_type_detail_parts(fact.display_name());
        CompletionItem {
            label: label.to_owned(),
            kind: CompletionKind::Type,
            detail: detail_parts.render(),
            insert_text: None,
            insert_format: CompletionInsertFormat::PlainText,
            sort_text: None,
            metadata: Default::default(),
        }
        .with_detail_parts(detail_parts)
    })
    .collect()
}

fn service_item_from_analysis(item: AnalysisCompletionItem) -> CompletionItem {
    let detail_parts = display_type_detail_parts(item.fact.display_name());
    CompletionItem {
        label: item.label,
        kind: CompletionKind::from(item.kind),
        detail: detail_parts.render(),
        insert_text: None,
        insert_format: CompletionInsertFormat::PlainText,
        sort_text: None,
        metadata: Default::default(),
    }
    .with_detail_parts(detail_parts)
}

fn service_item_from_schema_type(item: AnalysisCompletionItem, prefix: &str) -> CompletionItem {
    let symbol = schema_symbol(&item.label);
    let qualified_name = item.label.clone();
    type_completion_item(item, &qualified_name, prefix).with_symbol(symbol)
}

#[cfg(test)]
mod tests {
    use vela_analysis::{registry::RegistryFacts, type_fact::TypeFact};

    use super::*;
    use crate::{
        DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, Workspace,
        WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
        completion::{CompletionContextKind, CompletionItem, CompletionList},
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
        assert_completion(&completions, "Player", CompletionKind::Type);
        let player = completion(&completions, "Player");
        assert_eq!(player.lookup(), "game::main::Player");
        assert_eq!(player.filter_text(), "game::main::Player");
        assert_eq!(player.label_details().description(), Some("game::main"));
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

    fn completion<'a>(list: &'a CompletionList, label: &str) -> &'a CompletionItem {
        list.items()
            .iter()
            .find(|item| item.label() == label)
            .unwrap_or_else(|| panic!("completion {label} should exist in {list:?}"))
    }

    fn assert_no_completion(list: &CompletionList, label: &str) {
        assert!(
            list.items().iter().all(|item| item.label() != label),
            "{list:?}"
        );
    }
}
