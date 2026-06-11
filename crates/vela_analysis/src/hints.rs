use vela_common::PrimitiveTag;
use vela_hir::ids::ModuleId;
use vela_hir::module_graph::{Declaration, DeclarationKind, ImportResolution, ModuleGraph};
use vela_hir::type_hint::HirTypeHint;

use crate::type_fact::TypeFact;

pub fn type_fact_from_hint(graph: &ModuleGraph, hint: &HirTypeHint) -> TypeFact {
    type_fact_from_path(graph, &hint.path)
}

pub(crate) fn type_fact_from_hint_in_module(
    graph: &ModuleGraph,
    module: ModuleId,
    hint: &HirTypeHint,
) -> TypeFact {
    imported_schema_fact(graph, module, &hint.path)
        .unwrap_or_else(|| type_fact_from_hint(graph, hint))
}

pub fn type_fact_from_path(graph: &ModuleGraph, path: &[String]) -> TypeFact {
    if path.is_empty() {
        return TypeFact::Unknown;
    }

    if let [name] = path
        && let Some(fact) = builtin_type_fact(name)
    {
        return fact;
    }

    resolved_schema_fact(graph, path).unwrap_or(TypeFact::Unknown)
}

pub(crate) fn qualified_declaration_name(graph: &ModuleGraph, declaration: &Declaration) -> String {
    graph
        .module_path(declaration.module)
        .map(|path| {
            path.segments()
                .iter()
                .chain(std::iter::once(&declaration.name))
                .cloned()
                .collect::<Vec<_>>()
                .join("::")
        })
        .unwrap_or_else(|| declaration.name.clone())
}

pub(crate) fn declaration_schema_fact(
    graph: &ModuleGraph,
    declaration: &Declaration,
) -> Option<TypeFact> {
    let name = qualified_declaration_name(graph, declaration);
    match declaration.kind {
        DeclarationKind::Struct => Some(TypeFact::record(name)),
        DeclarationKind::Enum => Some(TypeFact::enum_type(name, None::<String>)),
        DeclarationKind::Trait => Some(TypeFact::trait_type(name)),
        _ => None,
    }
}

fn builtin_type_fact(name: &str) -> Option<TypeFact> {
    if let Some(tag) = PrimitiveTag::from_name(name) {
        return Some(TypeFact::primitive(tag));
    }

    match name {
        "any" => Some(TypeFact::Any),
        "array" => Some(TypeFact::array(TypeFact::Unknown)),
        "map" => Some(TypeFact::map(TypeFact::Unknown, TypeFact::Unknown)),
        "set" => Some(TypeFact::set(TypeFact::Unknown)),
        "function" => Some(TypeFact::function(Vec::new(), TypeFact::Unknown)),
        "Option" => Some(TypeFact::option(TypeFact::Unknown)),
        "Result" => Some(TypeFact::result(TypeFact::Unknown, TypeFact::Unknown)),
        _ => None,
    }
}

fn resolved_schema_fact(graph: &ModuleGraph, path: &[String]) -> Option<TypeFact> {
    let matches = graph
        .declarations()
        .filter(|declaration| schema_path_matches(graph, declaration, path))
        .collect::<Vec<_>>();

    let [declaration] = matches.as_slice() else {
        return None;
    };
    declaration_schema_fact(graph, declaration)
}

fn imported_schema_fact(
    graph: &ModuleGraph,
    module: ModuleId,
    path: &[String],
) -> Option<TypeFact> {
    let [name] = path else {
        return None;
    };
    graph.imports(module)?.iter().find_map(|import| {
        let imported_name = import.alias.as_ref().or_else(|| import.path.last())?;
        if imported_name != name {
            return None;
        }
        let Some(ImportResolution::Declaration(declaration)) = import.resolution else {
            return None;
        };
        declaration_schema_fact(graph, graph.declaration(declaration)?)
    })
}

fn schema_path_matches(graph: &ModuleGraph, declaration: &Declaration, path: &[String]) -> bool {
    declaration_schema_fact(graph, declaration).is_some()
        && ((path.len() == 1 && path[0] == declaration.name)
            || graph.module_path(declaration.module).is_some_and(|module| {
                module
                    .segments()
                    .iter()
                    .chain(std::iter::once(&declaration.name))
                    .eq(path.iter())
            }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_common::SourceId;
    use vela_hir::module_graph::{ModulePath, ModuleSource};

    fn graph(source: &str) -> ModuleGraph {
        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_qualified("game"),
            source,
        ));
        graph.resolve_imports();
        assert_eq!(graph.diagnostics(), &[]);
        graph
    }

    #[test]
    fn builtin_hints_map_to_internal_facts_without_generics() {
        let graph = graph("");
        assert_eq!(
            type_fact_from_path(&graph, &["array".to_owned()]),
            TypeFact::array(TypeFact::Unknown)
        );
        assert_eq!(
            type_fact_from_path(&graph, &["map".to_owned()]),
            TypeFact::map(TypeFact::Unknown, TypeFact::Unknown)
        );
        assert_eq!(
            type_fact_from_path(&graph, &["Option".to_owned()]),
            TypeFact::option(TypeFact::Unknown)
        );
    }

    #[test]
    fn schema_hints_map_to_qualified_record_enum_and_trait_facts() {
        let graph = graph(
            r#"
            struct Player { level: i64 }
            enum QuestState { Active, Done }
            trait Rewardable { fn reward(self) -> i64; }
            "#,
        );

        assert_eq!(
            type_fact_from_path(&graph, &["game".to_owned(), "Player".to_owned()]),
            TypeFact::record("game::Player")
        );
        assert_eq!(
            type_fact_from_path(&graph, &["QuestState".to_owned()]),
            TypeFact::enum_type("game::QuestState", None::<String>)
        );
        assert_eq!(
            type_fact_from_path(&graph, &["Rewardable".to_owned()]),
            TypeFact::trait_type("game::Rewardable")
        );
    }

    #[test]
    fn ambiguous_schema_hint_degrades_to_unknown() {
        let mut graph = graph("struct Player { level: i64 }");
        graph.add_source(ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_qualified("arena"),
            "struct Player { level: i64 }",
        ));
        graph.resolve_imports();
        assert_eq!(graph.diagnostics(), &[]);

        assert_eq!(
            type_fact_from_path(&graph, &["Player".to_owned()]),
            TypeFact::Unknown
        );
        assert_eq!(
            type_fact_from_path(&graph, &["arena".to_owned(), "Player".to_owned()]),
            TypeFact::record("arena::Player")
        );
    }

    #[test]
    fn imported_schema_alias_hints_map_to_qualified_facts() {
        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_qualified("game::main"),
            r#"
            use game::reward::Reward as Prize
            fn grant(reward: Prize) -> Prize {
                return reward;
            }
            "#,
        ));
        graph.add_source(ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_qualified("game::reward"),
            "pub struct Reward { count: i64 }",
        ));
        graph.resolve_imports();
        assert_eq!(graph.diagnostics(), &[]);

        let grant = graph
            .declarations()
            .find(|declaration| declaration.name == "grant")
            .expect("grant declaration");
        let signature = graph.function_signature(grant.id).expect("grant signature");

        assert_eq!(
            type_fact_from_hint_in_module(
                &graph,
                grant.module,
                signature.params[0]
                    .type_hint
                    .as_ref()
                    .expect("param type hint")
            ),
            TypeFact::record("game::reward::Reward")
        );
        assert_eq!(
            type_fact_from_hint_in_module(
                &graph,
                grant.module,
                signature.return_type.as_ref().expect("return type hint")
            ),
            TypeFact::record("game::reward::Reward")
        );
    }
}
