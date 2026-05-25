use std::collections::BTreeMap;

use vela_hir::{DeclarationKind, HirDeclId, HirLocalId, ModuleGraph};

use crate::TypeFact;
use crate::hints::{declaration_schema_fact, type_fact_from_hint};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AnalysisFacts {
    declarations: BTreeMap<HirDeclId, TypeFact>,
    locals: BTreeMap<HirLocalId, TypeFact>,
}

impl AnalysisFacts {
    #[must_use]
    pub fn from_module_graph(graph: &ModuleGraph) -> Self {
        let mut facts = Self::default();

        for declaration in graph.declarations() {
            if let Some(fact) = declaration_fact(graph, declaration.id) {
                facts.declarations.insert(declaration.id, fact);
            }

            if let Some(bindings) = graph.bindings(declaration.id) {
                facts.locals.extend(bindings.locals().filter_map(|local| {
                    let hint = local.type_hint.as_ref()?;
                    Some((local.id, type_fact_from_hint(graph, hint)))
                }));
            }
        }

        facts
    }

    #[must_use]
    pub fn declaration(&self, declaration: HirDeclId) -> Option<&TypeFact> {
        self.declarations.get(&declaration)
    }

    pub fn declarations(&self) -> impl Iterator<Item = (HirDeclId, &TypeFact)> {
        self.declarations
            .iter()
            .map(|(declaration, fact)| (*declaration, fact))
    }

    #[must_use]
    pub fn local(&self, local: HirLocalId) -> Option<&TypeFact> {
        self.locals.get(&local)
    }

    pub fn locals(&self) -> impl Iterator<Item = (HirLocalId, &TypeFact)> {
        self.locals.iter().map(|(local, fact)| (*local, fact))
    }
}

fn declaration_fact(graph: &ModuleGraph, declaration: HirDeclId) -> Option<TypeFact> {
    let metadata = graph.declaration(declaration)?;
    if let Some(schema_fact) = declaration_schema_fact(graph, metadata) {
        return Some(schema_fact);
    }

    match metadata.kind {
        DeclarationKind::Const => graph
            .const_metadata(declaration)?
            .type_hint
            .as_ref()
            .map(|hint| type_fact_from_hint(graph, hint)),
        DeclarationKind::Function => graph.function_signature(declaration).map(|signature| {
            let params = signature
                .params
                .iter()
                .map(|param| {
                    param
                        .type_hint
                        .as_ref()
                        .map_or(TypeFact::Unknown, |hint| type_fact_from_hint(graph, hint))
                })
                .collect();
            let returns = signature
                .return_type
                .as_ref()
                .map_or(TypeFact::Unknown, |hint| type_fact_from_hint(graph, hint));
            TypeFact::function(params, returns)
        }),
        DeclarationKind::Impl => None,
        DeclarationKind::Struct | DeclarationKind::Enum | DeclarationKind::Trait => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_common::SourceId;
    use vela_hir::{LocalBindingKind, ModulePath, ModuleSource};

    #[test]
    fn analysis_facts_collect_function_signature_and_local_hints() {
        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_dotted("game"),
            r#"
            struct Player { level: int }
            fn grant(player: Player, amount: int) -> bool {
                let rewards: map = {};
                let title: string = "hero";
                return amount > 0;
            }
            "#,
        ));
        graph.resolve_imports();
        assert_eq!(graph.diagnostics(), &[]);

        let function = graph
            .declarations()
            .find(|declaration| declaration.name == "grant")
            .expect("grant declaration");
        let facts = AnalysisFacts::from_module_graph(&graph);

        assert_eq!(
            facts.declaration(function.id),
            Some(&TypeFact::function(
                vec![TypeFact::record("game.Player"), TypeFact::Int],
                TypeFact::Bool,
            ))
        );

        let bindings = graph.bindings(function.id).expect("grant bindings");
        let rewards = bindings
            .locals()
            .find(|local| local.name == "rewards" && local.kind == LocalBindingKind::Let)
            .expect("rewards local");
        let title = bindings
            .locals()
            .find(|local| local.name == "title" && local.kind == LocalBindingKind::Let)
            .expect("title local");

        assert_eq!(
            facts.local(rewards.id),
            Some(&TypeFact::map(TypeFact::Unknown, TypeFact::Unknown))
        );
        assert_eq!(facts.local(title.id), Some(&TypeFact::String));
    }

    #[test]
    fn analysis_facts_include_schema_declarations() {
        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_dotted("game"),
            "enum QuestState { Active, Done }",
        ));
        graph.resolve_imports();
        assert_eq!(graph.diagnostics(), &[]);

        let declaration = graph
            .declarations()
            .find(|declaration| declaration.name == "QuestState")
            .expect("QuestState declaration");
        let facts = AnalysisFacts::from_module_graph(&graph);

        assert_eq!(
            facts.declaration(declaration.id),
            Some(&TypeFact::enum_type("game.QuestState", None::<String>))
        );
    }
}
