use std::collections::BTreeMap;

use vela_hir::binding::BindingResolution;
use vela_hir::ids::{HirBlockId, HirDeclId, HirExprId, HirLocalId, HirPatternId, HirStmtId};
use vela_hir::module_graph::ModuleGraph;

use crate::literals::{LiteralFacts, LiteralPrimitiveContext, LiteralResult};
use crate::semantic_facts::{
    CallTargetFact, ConstructorTargetFact, ControlFlowFact, HirSemanticFacts, HostPathTargetFact,
    MemberTargetFact, OperatorTargetFact, ScriptTypeTargetFact,
};
use crate::type_fact::TypeFact;

mod build;

pub(crate) use build::ExecutableReceiverSeed;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AnalysisFacts {
    declarations: BTreeMap<HirDeclId, TypeFact>,
    locals: BTreeMap<HirLocalId, TypeFact>,
    expressions: BTreeMap<HirExprId, TypeFact>,
    local_script_types: BTreeMap<HirLocalId, ScriptTypeTargetFact>,
    resolutions: BTreeMap<HirExprId, BindingResolution>,
    literals: LiteralFacts,
    semantic: HirSemanticFacts,
}

impl AnalysisFacts {
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

    #[must_use]
    pub fn script_type(&self, expression: HirExprId) -> Option<&ScriptTypeTargetFact> {
        self.semantic.script_type(expression)
    }

    #[must_use]
    pub fn literal(&self, expression: HirExprId) -> Option<&LiteralResult> {
        self.literals.get(expression)
    }

    /// Revalidates numeric literals with the exact primitive or dynamic
    /// contexts selected by the compile-target analysis.
    pub fn resolve_literal_contexts(
        &mut self,
        graph: &ModuleGraph,
        contexts: &BTreeMap<HirExprId, LiteralPrimitiveContext>,
    ) {
        self.literals = LiteralFacts::from_module_graph_with_contexts(graph, contexts);
    }

    #[must_use]
    pub const fn literal_facts(&self) -> &LiteralFacts {
        &self.literals
    }

    #[must_use]
    pub fn literal_diagnostics(&self, graph: &ModuleGraph) -> Vec<vela_common::Diagnostic> {
        self.literals.compiler_diagnostics(graph)
    }

    #[must_use]
    pub fn local_script_type(&self, local: HirLocalId) -> Option<&ScriptTypeTargetFact> {
        self.semantic
            .local_script_type(local)
            .or_else(|| self.local_script_types.get(&local))
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

    pub(crate) fn base_local_script_type(
        &self,
        local: HirLocalId,
    ) -> Option<&ScriptTypeTargetFact> {
        self.local_script_types.get(&local)
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

#[cfg(test)]
mod body_binding_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::RegistryFacts;
    use vela_common::{HostTypeId, SourceId};
    use vela_def::{FieldId, TypeId};
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
    fn analysis_facts_unwrap_try_payloads_and_preserve_dynamic_facts() {
        use vela_hir::body::HirExprKind;

        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            SourceId::new(19),
            ModulePath::from_qualified("game"),
            r#"
            fn main() {
                let option_payload = fixture::option()?;
                let result_payload = fixture::result()?;
                let union_payload = fixture::union()?;
                let known_failure = fixture::failure()?;
                let dynamic_payload = fixture::dynamic()?;
                let unknown_payload = fixture::unknown()?;
            }
            "#,
        ));
        graph.resolve_imports();
        assert_eq!(graph.diagnostics(), &[]);

        let mut schema = RegistryFacts::default();
        schema.insert_function(
            "fixture::option",
            TypeFact::function(Vec::new(), TypeFact::option(TypeFact::I64)),
        );
        schema.insert_function(
            "fixture::result",
            TypeFact::function(
                Vec::new(),
                TypeFact::result(TypeFact::STRING, TypeFact::BOOL),
            ),
        );
        schema.insert_function(
            "fixture::union",
            TypeFact::function(
                Vec::new(),
                TypeFact::union([
                    TypeFact::option_some(TypeFact::I64),
                    TypeFact::result_ok(TypeFact::STRING),
                    TypeFact::option_none(),
                ]),
            ),
        );
        schema.insert_function(
            "fixture::failure",
            TypeFact::function(
                Vec::new(),
                TypeFact::union([
                    TypeFact::option_none(),
                    TypeFact::result_err(TypeFact::STRING),
                ]),
            ),
        );
        schema.insert_function(
            "fixture::dynamic",
            TypeFact::function(Vec::new(), TypeFact::Any),
        );
        schema.insert_function(
            "fixture::unknown",
            TypeFact::function(Vec::new(), TypeFact::Unknown),
        );

        let facts = AnalysisFacts::from_module_graph_and_schema(&graph, &schema);
        let body = graph
            .bodies()
            .find(|body| matches!(body.owner, vela_hir::body::HirBodyOwner::Declaration(_)))
            .expect("main body");
        let try_facts = body
            .expressions
            .values()
            .filter(|expression| matches!(expression.kind, HirExprKind::Try { .. }))
            .map(|expression| {
                facts
                    .expression(expression.id)
                    .cloned()
                    .unwrap_or(TypeFact::Unknown)
            })
            .collect::<Vec<_>>();

        assert_eq!(
            try_facts,
            vec![
                TypeFact::I64,
                TypeFact::STRING,
                TypeFact::union([TypeFact::I64, TypeFact::STRING]),
                TypeFact::Never,
                TypeFact::Any,
                TypeFact::Unknown,
            ]
        );
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
        schema.insert_type_target(crate::registry::RegistryTypeTargetFact::new(
            "Player",
            TypeId::new(1),
            Some(HostTypeId::new(1)),
        ));
        schema.insert_field("Player", "level", TypeFact::I64);
        let level_access = crate::registry::RegistryFieldAccessFact {
            owner: "Player".to_owned(),
            name: "level".to_owned(),
            readable: true,
            writable: true,
            reflect_readable: false,
            reflect_writable: false,
            required_permissions: Vec::new(),
        };
        schema.insert_field_access(level_access.clone());
        schema.insert_field_target(crate::registry::RegistryFieldTargetFact::new(
            TypeId::new(1),
            "Player",
            "level",
            FieldId::new(2),
            Some(FieldId::new(2)),
            false,
            level_access,
        ));
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
        let reward_count = body
            .expressions
            .values()
            .find(|expression| {
                matches!(&expression.kind, HirExprKind::Field(field)
                    if field.name == "count"
                        && facts.expression(field.receiver)
                            == Some(&TypeFact::record("game::Reward")))
            })
            .expect("Reward.count field");
        assert_eq!(facts.expression(reward_count.id), Some(&TypeFact::I64));
        assert_eq!(
            facts.member_target(reward_count.id),
            Some(&MemberTargetFact::ScriptField {
                owner: reward.id,
                variant: None,
                name: "count".to_owned(),
            })
        );

        let level = body.expressions.values().find(|expression| {
            matches!(&expression.kind, HirExprKind::Field(field) if field.name == "level")
        }).expect("level field");
        assert_eq!(facts.expression(level.id), Some(&TypeFact::I64));
        assert!(matches!(
            facts.member_target(level.id),
            Some(MemberTargetFact::HostField(target))
                if target.owner_name == "Player" && target.name == "level"
                    && target.semantic == FieldId::new(2)
        ));
        assert!(matches!(
            facts.host_path_target(level.id),
            Some(path)
                if path.root_type.semantic == TypeId::new(1)
                    && matches!(path.segments.as_slice(),
                        [HostPathSegmentFact::Field(target)]
                            if target.semantic == FieldId::new(2))
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

    #[test]
    fn semantic_facts_resolve_source_fields_and_direct_lambdas_by_hir_identity() {
        use crate::semantic_facts::{CallTargetFact, MemberTargetFact};
        use vela_hir::body::{HirBodyOwner, HirExprKind};

        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            SourceId::new(17),
            ModulePath::from_qualified("game"),
            r#"
            struct Reward { count: i64 }
            fn identity(reward: Reward) -> Reward { return reward; }
            fn main(reward: Reward) -> i64 {
                let count = identity(reward).count;
                return (|value: i64| value + count)(1);
            }
            "#,
        ));
        graph.resolve_imports();
        assert_eq!(graph.diagnostics(), &[]);

        let reward = graph
            .declarations()
            .find(|declaration| declaration.name == "Reward")
            .expect("Reward declaration");
        let main = graph
            .declarations()
            .find(|declaration| declaration.name == "main")
            .expect("main declaration");
        let body = graph.function_body(main.id).expect("main body");
        let lambda = graph
            .bodies()
            .find(|candidate| {
                matches!(candidate.owner, HirBodyOwner::Lambda { parent, .. } if parent == body.id)
            })
            .expect("lambda body");
        let facts = AnalysisFacts::from_module_graph(&graph);

        let count = body
            .expressions
            .values()
            .find(|expression| {
                matches!(&expression.kind, HirExprKind::Field(field) if field.name == "count")
            })
            .expect("count field");
        let HirExprKind::Field(count_field) = &count.kind else {
            unreachable!("count expression is a field")
        };
        assert_eq!(
            facts.script_type(count_field.receiver),
            Some(&ScriptTypeTargetFact::declaration(reward.id))
        );
        assert_eq!(facts.expression(count.id), Some(&TypeFact::I64));
        assert_eq!(
            facts.member_target(count.id),
            Some(&MemberTargetFact::ScriptField {
                owner: reward.id,
                variant: None,
                name: "count".to_owned(),
            })
        );

        let call = body
            .expressions
            .values()
            .find(|expression| {
                matches!(&expression.kind, HirExprKind::Call(_))
                    && matches!(
                        facts.call_target(expression.id),
                        Some(CallTargetFact::Lambda(body)) if *body == lambda.id
                    )
            })
            .expect("direct lambda call");
        assert_eq!(
            facts.call_target(call.id),
            Some(&CallTargetFact::Lambda(lambda.id))
        );
    }

    #[test]
    fn semantic_host_paths_preserve_stable_targets_and_index_capabilities() {
        use crate::semantic_facts::{HostPathIndexKindFact, HostPathSegmentFact};
        use vela_hir::body::HirExprKind;
        use vela_reflect::registry::{
            FieldDesc, HostIndexCapability, TypeDesc, TypeKey, TypeRegistry,
        };

        let player_id = TypeId::new(101);
        let inventory_id = TypeId::new(102);
        let entry_id = TypeId::new(103);
        let inventory_field = FieldId::new(201);
        let amount_field = FieldId::new(202);
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(player_id, "Player"))
                .host_type(HostTypeId::new(11))
                .field(
                    FieldDesc::new(inventory_field, "inventory")
                        .type_hint("Inventory")
                        .writable(true),
                ),
        );
        registry.register(
            TypeDesc::new(TypeKey::new(inventory_id, "Inventory"))
                .host_type(HostTypeId::new(12))
                .index_capability(
                    HostIndexCapability::new()
                        .readable(true)
                        .writable(true)
                        .addable(true)
                        .removable(true)
                        .key_type("i64")
                        .value_type("Entry"),
                ),
        );
        registry.register(
            TypeDesc::new(TypeKey::new(entry_id, "Entry"))
                .host_type(HostTypeId::new(13))
                .field(
                    FieldDesc::new(amount_field, "amount")
                        .type_hint("i64")
                        .writable(true),
                ),
        );
        let schema = RegistryFacts::from_registry(&registry);

        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            SourceId::new(18),
            ModulePath::from_qualified("game"),
            r#"
            fn main(player: Player, slot: i64) -> i64 {
                return player.inventory[slot].amount;
            }
            "#,
        ));
        graph.resolve_imports();
        assert_eq!(graph.diagnostics(), &[]);
        let facts = AnalysisFacts::from_module_graph_and_schema(&graph, &schema);
        let body = graph
            .bodies()
            .find(|body| matches!(body.owner, vela_hir::body::HirBodyOwner::Declaration(_)))
            .expect("main body");
        let amount = body
            .expressions
            .values()
            .find(|expression| {
                matches!(&expression.kind, HirExprKind::Field(field) if field.name == "amount")
            })
            .expect("amount field");
        let path = facts
            .host_path_target(amount.id)
            .expect("resolved host path");

        assert_eq!(path.root_type.semantic, player_id);
        assert_eq!(path.root_type.host_runtime, Some(HostTypeId::new(11)));
        let [
            HostPathSegmentFact::Field(inventory),
            HostPathSegmentFact::Index {
                owner,
                kind,
                capability,
                ..
            },
            HostPathSegmentFact::Field(amount),
        ] = path.segments.as_slice()
        else {
            panic!("unexpected host path: {path:?}");
        };
        assert_eq!(inventory.owner, player_id);
        assert_eq!(inventory.semantic, inventory_field);
        assert_eq!(inventory.host_runtime, Some(inventory_field));
        assert!(inventory.access.writable);
        assert!(!inventory.variant_field);
        assert_eq!(owner.semantic, inventory_id);
        assert_eq!(owner.host_runtime, Some(HostTypeId::new(12)));
        assert_eq!(*kind, HostPathIndexKindFact::Index);
        assert!(
            capability.readable
                && capability.writable
                && capability.addable
                && capability.removable
        );
        assert_eq!(capability.key, TypeFact::I64);
        assert_eq!(capability.value, TypeFact::host("Entry"));
        assert_eq!(amount.owner, entry_id);
        assert_eq!(amount.semantic, amount_field);
        assert_eq!(amount.host_runtime, Some(amount_field));
    }
}
