use std::collections::BTreeMap;

use vela_hir::binding::BindingResolution;
use vela_hir::ids::{HirDeclId, HirExprId, HirLocalId};
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};

use crate::hints::{declaration_schema_fact, type_fact_from_hint_in_module};
use crate::type_fact::TypeFact;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AnalysisFacts {
    declarations: BTreeMap<HirDeclId, TypeFact>,
    locals: BTreeMap<HirLocalId, TypeFact>,
    expressions: BTreeMap<HirExprId, TypeFact>,
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
                    Some((
                        local.id,
                        type_fact_from_hint_in_module(graph, declaration.module, hint),
                    ))
                }));
            }
        }

        for declaration in graph.declarations() {
            let Some(bindings) = graph.bindings(declaration.id) else {
                continue;
            };
            for (expression, resolution) in bindings.resolutions() {
                if let Some(fact) = facts.fact_for_resolution(resolution).cloned() {
                    facts.expressions.insert(expression, fact);
                }
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

    #[must_use]
    pub fn expression(&self, expression: HirExprId) -> Option<&TypeFact> {
        self.expressions.get(&expression)
    }

    pub fn expressions(&self) -> impl Iterator<Item = (HirExprId, &TypeFact)> {
        self.expressions
            .iter()
            .map(|(expression, fact)| (*expression, fact))
    }

    fn fact_for_resolution(&self, resolution: &BindingResolution) -> Option<&TypeFact> {
        match resolution {
            BindingResolution::Local(local) => self.locals.get(local),
            BindingResolution::Declaration(declaration) => self.declarations.get(declaration),
            BindingResolution::Import(_) | BindingResolution::QualifiedPath(_) => None,
        }
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
            .map(|hint| type_fact_from_hint_in_module(graph, metadata.module, hint)),
        DeclarationKind::Function => graph.function_signature(declaration).map(|signature| {
            let params = signature
                .params
                .iter()
                .map(|param| {
                    param.type_hint.as_ref().map_or(TypeFact::Unknown, |hint| {
                        type_fact_from_hint_in_module(graph, metadata.module, hint)
                    })
                })
                .collect();
            let returns = signature
                .return_type
                .as_ref()
                .map_or(TypeFact::Unknown, |hint| {
                    type_fact_from_hint_in_module(graph, metadata.module, hint)
                });
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
    use vela_hir::binding::LocalBindingKind;
    use vela_hir::module_graph::{ModulePath, ModuleSource};

    #[test]
    fn analysis_facts_collect_function_signature_and_local_hints() {
        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_qualified("game"),
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
                vec![TypeFact::record("game::Player"), TypeFact::Int],
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
            ModulePath::from_qualified("game"),
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
            Some(&TypeFact::enum_type("game::QuestState", None::<String>))
        );
    }

    #[test]
    fn analysis_facts_include_resolved_expression_facts() {
        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_qualified("game"),
            r#"
            const BONUS: int = 3
            fn grant(amount: int) -> int {
                let base: int = amount;
                return BONUS + base;
            }
            "#,
        ));
        graph.resolve_imports();
        assert_eq!(graph.diagnostics(), &[]);

        let grant = graph
            .declarations()
            .find(|declaration| declaration.name == "grant")
            .expect("grant declaration");
        let bindings = graph.bindings(grant.id).expect("grant bindings");
        let facts = AnalysisFacts::from_module_graph(&graph);

        let mut saw_amount = false;
        let mut saw_base = false;
        let mut saw_bonus = false;
        for (expression, resolution) in bindings.resolutions() {
            match resolution {
                BindingResolution::Local(local) => {
                    let local = bindings.local(*local).expect("local binding");
                    if local.name == "amount" {
                        saw_amount = true;
                        assert_eq!(facts.expression(expression), Some(&TypeFact::Int));
                    }
                    if local.name == "base" {
                        saw_base = true;
                        assert_eq!(facts.expression(expression), Some(&TypeFact::Int));
                    }
                }
                BindingResolution::Declaration(declaration) => {
                    let declaration = graph.declaration(*declaration).expect("declaration");
                    if declaration.name == "BONUS" {
                        saw_bonus = true;
                        assert_eq!(facts.expression(expression), Some(&TypeFact::Int));
                    }
                }
                BindingResolution::Import(_) | BindingResolution::QualifiedPath(_) => {}
            }
        }

        assert!(saw_amount);
        assert!(saw_base);
        assert!(saw_bonus);
    }
}
