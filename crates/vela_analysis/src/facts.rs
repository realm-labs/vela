use std::collections::BTreeMap;

use vela_hir::binding::BindingResolution;
use vela_hir::ids::{HirBlockId, HirDeclId, HirExprId, HirLocalId, HirPatternId, HirStmtId};
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};

use crate::hints::{declaration_schema_fact, type_fact_from_hint_in_module};
use crate::registry::RegistryFacts;
use crate::semantic_facts::{
    CallTargetFact, ConstructorTargetFact, ControlFlowFact, HirSemanticFacts, HostPathTargetFact,
    MemberTargetFact, OperatorTargetFact,
};
use crate::type_fact::TypeFact;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AnalysisFacts {
    declarations: BTreeMap<HirDeclId, TypeFact>,
    locals: BTreeMap<HirLocalId, TypeFact>,
    expressions: BTreeMap<HirExprId, TypeFact>,
    resolutions: BTreeMap<HirExprId, BindingResolution>,
    semantic: HirSemanticFacts,
}

impl AnalysisFacts {
    #[must_use]
    pub fn from_module_graph(graph: &ModuleGraph) -> Self {
        Self::from_module_graph_with_schema(graph, None)
    }

    #[must_use]
    pub fn from_module_graph_and_schema(graph: &ModuleGraph, schema: &RegistryFacts) -> Self {
        Self::from_module_graph_with_schema(graph, Some(schema))
    }

    fn from_module_graph_with_schema(graph: &ModuleGraph, schema: Option<&RegistryFacts>) -> Self {
        let mut facts = Self::default();

        for declaration in graph.declarations() {
            if let Some(fact) = declaration_fact(graph, declaration.id) {
                facts.declarations.insert(declaration.id, fact);
            }

            if let Some(bindings) = graph.bindings(declaration.id) {
                facts.locals.extend(bindings.locals().filter_map(|local| {
                    let hint = local.type_hint.as_ref()?;
                    let fact = type_fact_from_hint_in_module(graph, declaration.module, hint);
                    let fact = if matches!(fact, TypeFact::Unknown) {
                        schema
                            .and_then(|schema| schema_fact_for_hint(schema, &hint.path))
                            .unwrap_or(fact)
                    } else {
                        fact
                    };
                    Some((local.id, fact))
                }));
            }
        }

        for declaration in graph.declarations() {
            let Some(bindings) = graph.bindings(declaration.id) else {
                continue;
            };
            for (expression, resolution) in bindings.resolutions() {
                facts.resolutions.insert(expression, resolution.clone());
                if let Some(fact) = facts.fact_for_resolution(resolution).cloned() {
                    facts.expressions.insert(expression, fact);
                }
            }
        }

        facts.semantic = HirSemanticFacts::from_module_graph(graph, schema, &facts);
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
        self.semantic
            .local(local)
            .or_else(|| self.locals.get(&local))
    }

    pub fn locals(&self) -> impl Iterator<Item = (HirLocalId, &TypeFact)> {
        self.locals.iter().map(|(local, fact)| (*local, fact))
    }

    #[must_use]
    pub fn expression(&self, expression: HirExprId) -> Option<&TypeFact> {
        self.semantic
            .type_fact(expression)
            .or_else(|| self.expressions.get(&expression))
    }

    pub fn expressions(&self) -> impl Iterator<Item = (HirExprId, &TypeFact)> {
        self.expressions
            .iter()
            .map(|(expression, fact)| (*expression, fact))
    }

    pub(crate) fn base_expression(&self, expression: HirExprId) -> Option<&TypeFact> {
        self.expressions.get(&expression)
    }

    pub(crate) fn resolution(&self, expression: HirExprId) -> Option<&BindingResolution> {
        self.resolutions.get(&expression)
    }

    #[must_use]
    pub fn pattern(&self, pattern: HirPatternId) -> Option<&TypeFact> {
        self.semantic.pattern(pattern)
    }

    #[must_use]
    pub fn call_target(&self, expression: HirExprId) -> Option<&CallTargetFact> {
        self.semantic.call_target(expression)
    }

    #[must_use]
    pub fn member_target(&self, expression: HirExprId) -> Option<&MemberTargetFact> {
        self.semantic.member_target(expression)
    }

    #[must_use]
    pub fn operator_target(&self, expression: HirExprId) -> Option<OperatorTargetFact> {
        self.semantic.operator_target(expression)
    }

    #[must_use]
    pub fn constructor_target(&self, expression: HirExprId) -> Option<&ConstructorTargetFact> {
        self.semantic.constructor_target(expression)
    }

    #[must_use]
    pub fn pattern_constructor_target(
        &self,
        pattern: HirPatternId,
    ) -> Option<&ConstructorTargetFact> {
        self.semantic.pattern_constructor_target(pattern)
    }

    #[must_use]
    pub fn host_path_target(&self, expression: HirExprId) -> Option<&HostPathTargetFact> {
        self.semantic.host_path_target(expression)
    }

    #[must_use]
    pub fn effect(&self, expression: HirExprId) -> Option<&crate::registry::RegistryEffectFact> {
        self.semantic.effect(expression)
    }

    #[must_use]
    pub fn control_flow(&self, expression: HirExprId) -> Option<&ControlFlowFact> {
        self.semantic.control_flow(expression)
    }

    #[must_use]
    pub fn block_control_flow(&self, block: HirBlockId) -> Option<&ControlFlowFact> {
        self.semantic.block_control_flow(block)
    }

    #[must_use]
    pub fn statement_control_flow(&self, statement: HirStmtId) -> Option<&ControlFlowFact> {
        self.semantic.statement_control_flow(statement)
    }

    fn fact_for_resolution(&self, resolution: &BindingResolution) -> Option<&TypeFact> {
        match resolution {
            BindingResolution::Local(local) => self.locals.get(local),
            BindingResolution::Declaration(declaration) => self.declarations.get(declaration),
            BindingResolution::Import(_) | BindingResolution::QualifiedPath(_) => None,
        }
    }
}

fn schema_fact_for_hint(schema: &RegistryFacts, path: &[String]) -> Option<TypeFact> {
    if path.is_empty() {
        return None;
    }
    let qualified = path.join("::");
    schema
        .type_fact(&qualified)
        .or_else(|| schema.trait_fact(&qualified))
        .or_else(|| path.last().and_then(|name| schema.type_fact(name)))
        .or_else(|| path.last().and_then(|name| schema.trait_fact(name)))
        .cloned()
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
        DeclarationKind::Global => graph
            .global_metadata(declaration)
            .map(|global| type_fact_from_hint_in_module(graph, metadata.module, &global.type_hint)),
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
            struct Player { level: i64 }
            fn grant(player: Player, amount: i64) -> bool {
                let rewards: Map = {};
                let title: String = "hero";
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
                vec![TypeFact::record("game::Player"), TypeFact::I64],
                TypeFact::BOOL,
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
        assert_eq!(facts.local(title.id), Some(&TypeFact::STRING));
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
            const BONUS: i64 = 3
            fn grant(amount: i64) -> i64 {
                let base: i64 = amount;
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
                        assert_eq!(facts.expression(expression), Some(&TypeFact::I64));
                    }
                    if local.name == "base" {
                        saw_base = true;
                        assert_eq!(facts.expression(expression), Some(&TypeFact::I64));
                    }
                }
                BindingResolution::Declaration(declaration) => {
                    let declaration = graph.declaration(*declaration).expect("declaration");
                    if declaration.name == "BONUS" {
                        saw_bonus = true;
                        assert_eq!(facts.expression(expression), Some(&TypeFact::I64));
                    }
                }
                BindingResolution::Import(_) | BindingResolution::QualifiedPath(_) => {}
            }
        }

        assert!(saw_amount);
        assert!(saw_base);
        assert!(saw_bonus);
    }

    #[test]
    fn analysis_facts_include_schema_local_hints_for_expression_facts() {
        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_qualified("game"),
            r#"
            fn main(enemy: Enemy) {
                return enemy;
            }
            "#,
        ));
        graph.resolve_imports();
        assert_eq!(graph.diagnostics(), &[]);
        let mut schema = RegistryFacts::default();
        schema.insert_type("Enemy", TypeFact::host("Enemy"));

        let main = graph
            .declarations()
            .find(|declaration| declaration.name == "main")
            .expect("main declaration");
        let bindings = graph.bindings(main.id).expect("main bindings");
        let facts = AnalysisFacts::from_module_graph_and_schema(&graph, &schema);
        let [enemy] = bindings.locals_named("enemy") else {
            panic!("expected enemy parameter");
        };

        assert_eq!(facts.local(*enemy), Some(&TypeFact::host("Enemy")));
        assert!(bindings.resolutions().any(|(expression, resolution)| {
            if resolution != &BindingResolution::Local(*enemy) {
                return false;
            }
            facts.expression(expression) == Some(&TypeFact::host("Enemy"))
        }));
    }

    #[test]
    fn analysis_facts_include_global_type_hints() {
        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_qualified("game"),
            r#"
            struct Player { level: i64 }
            global active: Player
            fn current() {
                return active;
            }
            "#,
        ));
        graph.resolve_imports();
        assert_eq!(graph.diagnostics(), &[]);

        let active = graph
            .declarations()
            .find(|declaration| declaration.name == "active")
            .expect("active global declaration");
        let current = graph
            .declarations()
            .find(|declaration| declaration.name == "current")
            .expect("current function declaration");
        let bindings = graph.bindings(current.id).expect("current bindings");
        let facts = AnalysisFacts::from_module_graph(&graph);

        assert_eq!(
            facts.declaration(active.id),
            Some(&TypeFact::record("game::Player"))
        );

        let mut saw_active = false;
        for (expression, resolution) in bindings.resolutions() {
            let BindingResolution::Declaration(declaration) = resolution else {
                continue;
            };
            if *declaration == active.id {
                saw_active = true;
                assert_eq!(
                    facts.expression(expression),
                    Some(&TypeFact::record("game::Player"))
                );
            }
        }

        assert!(saw_active);
    }

    #[test]
    fn analysis_facts_evaluate_complete_hir_body_and_resolve_targets() {
        use crate::registry::RegistryEffectFact;
        use crate::semantic_facts::{
            CallTargetFact, ConstructorTargetFact, HostPathSegmentFact, MemberTargetFact,
            OperatorTargetFact,
        };
        use vela_hir::body::{HirBodyRoot, HirExprKind};

        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_qualified("game"),
            r#"
            struct Reward { count: i64 }
            enum State { Ready(value) }
            fn main(player: Player, values: Array<i64>, state: State) -> bool {
                let reward = Reward { count: 1 };
                let ready = State::Ready(1);
                values.len();
                math::max(1, 2);
                audit::log("saved");
                player.save();
                match state { State::Ready(value) => value, _ => 0 }
                return player.level > values[0] && reward.count > 0;
            }
            "#,
        ));
        graph.resolve_imports();
        assert_eq!(graph.diagnostics(), &[]);

        let mut schema = RegistryFacts::default();
        schema.insert_type("Player", TypeFact::host("Player"));
        schema.insert_field("Player", "level", TypeFact::I64);
        schema.insert_method(
            "Player",
            "save",
            TypeFact::function(Vec::new(), TypeFact::BOOL),
        );
        schema.insert_method_effect("Player", "save", RegistryEffectFact::host_write());
        schema.insert_function(
            "audit::log",
            TypeFact::function(vec![TypeFact::STRING], TypeFact::UNIT),
        );
        schema.insert_function_origin("audit::log", vela_reflect::modules::DeclOrigin::Host);
        schema.insert_function_effect("audit::log", RegistryEffectFact::host_read());
        let facts = AnalysisFacts::from_module_graph_and_schema(&graph, &schema);

        let body = graph
            .bodies()
            .find(|body| matches!(body.owner, vela_hir::body::HirBodyOwner::Declaration(_)))
            .expect("main body");
        let record = body
            .expressions
            .values()
            .find(|expression| matches!(&expression.kind, HirExprKind::Record { .. }))
            .expect("record expression");
        let reward = graph
            .declarations()
            .find(|declaration| declaration.name == "Reward")
            .expect("Reward declaration");
        assert_eq!(
            facts.constructor_target(record.id),
            Some(&ConstructorTargetFact::Declaration(reward.id))
        );

        let level = body.expressions.values().find(|expression| {
            matches!(&expression.kind, HirExprKind::Field(field) if field.name == "level")
        }).expect("level field");
        assert_eq!(facts.expression(level.id), Some(&TypeFact::I64));
        assert!(matches!(
            facts.member_target(level.id),
            Some(MemberTargetFact::HostField { owner, name })
                if owner == "Player" && name == "level"
        ));
        assert!(matches!(
            facts.host_path_target(level.id),
            Some(path) if path.segments == [HostPathSegmentFact::Field("level".to_owned())]
        ));

        let save_call = body
            .expressions
            .values()
            .find(|expression| {
                matches!(&expression.kind, HirExprKind::Call(call)
                if body.field(call.callee).is_some_and(|field| field.name == "save"))
            })
            .expect("save call");
        assert_eq!(facts.expression(save_call.id), Some(&TypeFact::BOOL));
        assert!(matches!(
            facts.call_target(save_call.id),
            Some(CallTargetFact::HostMethod { owner, name })
                if owner == "Player" && name == "save"
        ));
        assert_eq!(
            facts.effect(save_call.id),
            Some(&RegistryEffectFact::host_write())
        );

        let len_call = body
            .expressions
            .values()
            .find(|expression| {
                matches!(&expression.kind, HirExprKind::Call(call)
                    if body.field(call.callee).is_some_and(|field| field.name == "len"))
            })
            .expect("len call");
        assert_eq!(facts.expression(len_call.id), Some(&TypeFact::I64));
        assert!(matches!(
            facts.call_target(len_call.id),
            Some(CallTargetFact::StdlibMethod { name }) if name == "len"
        ));
        assert_eq!(facts.effect(len_call.id), Some(&RegistryEffectFact::pure()));

        let path_call = |wanted: &[&str]| {
            body.expressions.values().find(|expression| {
                let HirExprKind::Call(call) = &expression.kind else {
                    return false;
                };
                body.paths.iter().any(|path| {
                    path.owner == vela_hir::body::HirPathOwner::Expression(call.callee)
                        && path
                            .path
                            .iter()
                            .map(String::as_str)
                            .eq(wanted.iter().copied())
                })
            })
        };
        let max_call = path_call(&["math", "max"]).expect("stdlib function call");
        assert_eq!(facts.expression(max_call.id), Some(&TypeFact::I64));
        assert!(matches!(
            facts.call_target(max_call.id),
            Some(CallTargetFact::StdlibFunction { path }) if path == "math::max"
        ));
        let native_call = path_call(&["audit", "log"]).expect("native function call");
        assert_eq!(facts.expression(native_call.id), Some(&TypeFact::UNIT));
        assert!(matches!(
            facts.call_target(native_call.id),
            Some(CallTargetFact::NativeFunction { path }) if path == "audit::log"
        ));
        assert_eq!(
            facts.effect(native_call.id),
            Some(&RegistryEffectFact::host_read())
        );

        let comparison = body
            .expressions
            .values()
            .find(|expression| {
                matches!(
                    &expression.kind,
                    HirExprKind::Binary {
                        op: Some(vela_hir::body::HirBinaryOp::Greater),
                        ..
                    }
                )
            })
            .expect("comparison expression");
        assert_eq!(
            facts.operator_target(comparison.id),
            Some(OperatorTargetFact::Binary(
                vela_hir::body::HirBinaryOp::Greater
            ))
        );

        let state = graph
            .declarations()
            .find(|declaration| declaration.name == "State")
            .expect("State declaration");
        let variant_call = body
            .expressions
            .values()
            .find(|expression| {
                matches!(&expression.kind, HirExprKind::Call(call)
                if body.paths.iter().any(|path| {
                    path.owner == vela_hir::body::HirPathOwner::Expression(call.callee)
                        && path.path.iter().map(String::as_str).eq(["State", "Ready"])
                }))
            })
            .expect("variant call");
        assert_eq!(
            facts.call_target(variant_call.id),
            Some(&CallTargetFact::Variant {
                enum_declaration: state.id,
                variant: "Ready".to_owned(),
            })
        );
        let pattern = body
            .patterns
            .values()
            .find(|pattern| {
                matches!(
                    &pattern.kind,
                    vela_hir::body::HirPatternKind::TupleVariant { .. }
                )
            })
            .expect("variant pattern");
        assert_eq!(
            facts.pattern_constructor_target(pattern.id),
            Some(&ConstructorTargetFact::Variant {
                enum_declaration: state.id,
                variant: "Ready".to_owned(),
            })
        );

        let HirBodyRoot::Block(root) = body.root else {
            panic!("main should have a root block");
        };
        assert!(matches!(
            facts.block_control_flow(root),
            Some(flow) if flow.may_return && !flow.can_fallthrough
        ));
    }
}
