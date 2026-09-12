use super::{
    CompletionContext, CompletionItem,
    builtin_type::builtin_type_hint_completions,
    label_segment_matches,
    type_paths::{TypePaths, is_type_item},
};
use crate::{LanguageServiceDatabases, QueryContext};

pub(super) fn type_hint_completion_items(
    databases: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    context: &CompletionContext,
) -> Vec<CompletionItem> {
    let Some(paths) = TypePaths::new(databases, query) else {
        return Vec::new();
    };
    let mut items = Vec::new();
    if let Some(base) = context.module_base() {
        let Some(base) = paths.expand(base) else {
            return items;
        };
        let namespace = format!("{base}::");
        let mut children = paths
            .paths
            .keys()
            .filter_map(|path| path.strip_prefix(&namespace))
            .filter_map(|suffix| suffix.split("::").next())
            .map(str::to_owned)
            .collect::<std::collections::BTreeSet<_>>();
        let graph = databases.hir_db().graph();
        if let Some(key) = query.module_key().and_then(|current| {
            graph.resolve_module_path(
                current,
                &base.split("::").map(str::to_owned).collect::<Vec<_>>(),
            )
        }) {
            children.extend(
                graph
                    .module_child_segments(&key)
                    .into_iter()
                    .map(str::to_owned),
            );
            // Keep the namespace spelling that addresses this package.
            if let Some(module) = graph.module_id(&key) {
                children.extend(
                    graph
                        .declarations_in_module(module)
                        .into_iter()
                        .map(|declaration| declaration.name.clone()),
                );
            }
        }
        for label in children
            .into_iter()
            .filter(|label| label.starts_with(context.prefix()))
        {
            if let Some(item) = paths.item(&format!("{base}::{label}"), context.prefix(), true) {
                items.push(item);
            }
        }
    } else {
        items.extend(builtin_type_hint_completions());
        for address in paths
            .paths
            .keys()
            .filter(|path| label_segment_matches(path, context.prefix()))
        {
            if let Some(item) = paths.item(address, context.prefix(), false) {
                items.push(item);
            }
        }
        for item in paths
            .scope
            .completion_items(context)
            .into_iter()
            .filter(is_type_item)
        {
            if !items.iter().any(|other| {
                other.label == item.label
                    && other.insert_text == item.insert_text
                    && other.symbol() == item.symbol()
            }) {
                items.push(item);
            }
        }
    }
    super::dedupe_and_filter_service_items(
        items,
        context.replace_range(),
        context.prefix(),
        |item| label_segment_matches(item.label(), context.prefix()),
    )
}

#[cfg(test)]
mod tests {
    use vela_analysis::{registry::RegistryFacts, type_fact::TypeFact};

    use super::super::CompletionKind;
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
        schema.insert_function("play", TypeFact::function(Vec::new(), TypeFact::UNIT));
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
    fn type_hint_completion_suggests_unit_and_not_null() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub fn main(value: ) { return value }";
        let databases = databases_for(document.clone(), text);
        let completions = databases.completion_items(
            &document,
            Position::new(0, text.find(": )").expect("empty type hint") + ": ".len()),
        );

        assert_eq!(
            completions.context().kind(),
            CompletionContextKind::TypeHint
        );
        let unit = completion(&completions, "()");
        assert_eq!(unit.kind(), CompletionKind::Type);
        assert_eq!(unit.detail(), "()");
        assert_no_completion(&completions, "null");
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
