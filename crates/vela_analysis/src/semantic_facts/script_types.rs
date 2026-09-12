use vela_hir::binding::BindingResolution;
use vela_hir::body::{HirBody, HirBodyRoot, HirExprKind, HirPatternKind, HirStmtKind};
use vela_hir::ids::HirExprId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};

use crate::facts::AnalysisFacts;
use crate::hints::schema_declaration_from_hint_in_module;

use super::HirSemanticFacts;
use super::targets::{ScriptTypeTargetFact, direct_lambda_body, source_field_fact};

impl HirSemanticFacts {
    pub(super) fn infer_script_type(
        &self,
        graph: &ModuleGraph,
        body: &HirBody,
        id: HirExprId,
        base: &AnalysisFacts,
    ) -> Option<ScriptTypeTargetFact> {
        let expression = body.expressions.get(&id)?;
        match &expression.kind {
            HirExprKind::Path(_) => match base.resolution(id) {
                Some(BindingResolution::Local(local)) => self
                    .local_script_types
                    .get(local)
                    .or_else(|| base.base_local_script_type(*local))
                    .cloned(),
                Some(BindingResolution::Declaration(declaration)) => {
                    let metadata = graph.declaration(*declaration)?;
                    let hint = match metadata.kind {
                        DeclarationKind::Const => graph
                            .const_metadata(*declaration)
                            .and_then(|metadata| metadata.type_hint.as_ref()),
                        DeclarationKind::State => graph
                            .state_metadata(*declaration)
                            .map(|metadata| &metadata.type_hint),
                        _ => None,
                    }?;
                    schema_declaration_from_hint_in_module(graph, metadata.module, hint)
                        .map(ScriptTypeTargetFact::declaration)
                }
                _ => None,
            },
            HirExprKind::Paren { expression }
            | HirExprKind::Try { expression }
            | HirExprKind::Await { expression } => {
                expression.and_then(|expression| self.script_types.get(&expression).cloned())
            }
            HirExprKind::Block { block } => {
                self.joined_script_type(super::value_flow::block_results(body, *block))
            }
            HirExprKind::If(value) => {
                self.joined_script_type(super::value_flow::if_results(body, value))
            }
            HirExprKind::Match(value) => {
                self.joined_script_type(super::value_flow::match_results(body, value))
            }
            HirExprKind::Assign { value, .. } => {
                value.and_then(|value| self.script_types.get(&value).cloned())
            }
            HirExprKind::Field(field) => {
                let receiver = self.script_types.get(&field.receiver)?;
                source_field_fact(graph, receiver, &field.name, None)?.target
            }
            HirExprKind::Call(call) => {
                if let Some(lambda) = direct_lambda_body(body, call.callee)
                    && let Some(lambda) = graph.body(lambda)
                {
                    return match lambda.root {
                        HirBodyRoot::Expr(expression) => {
                            self.script_types.get(&expression).cloned()
                        }
                        HirBodyRoot::Block(block) => {
                            self.joined_script_type(super::value_flow::block_results(lambda, block))
                        }
                        HirBodyRoot::Empty => None,
                    };
                }
                if let Some(metadata) = super::lookups::source_function(graph, body, call.callee) {
                    let hint = graph
                        .function_signature(metadata.id)?
                        .return_type
                        .as_ref()?;
                    return schema_declaration_from_hint_in_module(graph, metadata.module, hint)
                        .map(ScriptTypeTargetFact::declaration);
                }
                let field = body.field(call.callee)?;
                super::lookups::source_method_return(
                    graph,
                    &self.fact(field.receiver),
                    self.script_types.get(&field.receiver),
                    &field.name,
                    None,
                )?
                .target
            }
            _ => None,
        }
    }

    fn joined_script_type(&self, results: Vec<Option<HirExprId>>) -> Option<ScriptTypeTargetFact> {
        let mut results = results.into_iter();
        let mut target = self.script_types.get(&results.next()??)?.clone();
        for expression in results {
            let other = self.script_types.get(&expression?)?;
            if other.declaration != target.declaration {
                return None;
            }
            if other.variant != target.variant {
                target.variant = None;
            }
        }
        Some(target)
    }

    pub(super) fn infer_local_script_types(&mut self, body: &HirBody) {
        let inferred = body.statements.values().filter_map(|statement| {
            let HirStmtKind::Let {
                pattern: Some(pattern),
                initializer: Some(initializer),
                ..
            } = &statement.kind
            else {
                return None;
            };
            let HirPatternKind::Binding { local: Some(local) } = &body.patterns.get(pattern)?.kind
            else {
                return None;
            };
            self.script_types
                .get(initializer)
                .cloned()
                .map(|target| (*local, target))
        });
        for (local, target) in inferred.collect::<Vec<_>>() {
            self.local_script_types.entry(local).or_insert(target);
        }
    }
}
