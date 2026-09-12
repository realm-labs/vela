use super::{HirSemanticFacts, ScriptTypeTargetFact};
use vela_hir::binding::BindingResolution;
use vela_hir::body::{HirBody, HirExprKind};
use vela_hir::ids::HirExprId;
use vela_hir::module_graph::ModuleGraph;

/// Possible source owners at a value use. An incomplete path may carry an
/// unknown or non-source value, so its known candidates cannot prove uniqueness.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ScriptTypeOrigins {
    possible: Vec<ScriptTypeTargetFact>,
    complete: bool,
}

impl ScriptTypeOrigins {
    pub(crate) fn known(target: ScriptTypeTargetFact) -> Self {
        Self {
            possible: vec![target],
            complete: true,
        }
    }

    pub fn possible(&self) -> &[ScriptTypeTargetFact] {
        &self.possible
    }

    pub fn is_complete(&self) -> bool {
        self.complete
    }

    pub fn unique(&self) -> Option<&ScriptTypeTargetFact> {
        (self.complete && self.possible.len() == 1).then(|| &self.possible[0])
    }

    pub(crate) fn join(values: impl IntoIterator<Item = Self>) -> Self {
        let mut result = Self {
            possible: Vec::new(),
            complete: true,
        };
        let mut seen = false;
        for value in values {
            seen = true;
            result.complete &= value.complete;
            for target in value.possible {
                if let Some(prior) = result
                    .possible
                    .iter_mut()
                    .find(|prior| prior.declaration == target.declaration)
                {
                    if prior.variant != target.variant {
                        prior.variant = None;
                    }
                } else {
                    result.possible.push(target);
                }
            }
        }
        result.possible.sort_by(|a, b| {
            a.declaration
                .cmp(&b.declaration)
                .then(a.variant.cmp(&b.variant))
        });
        result.complete &= seen;
        result
    }

    fn map_targets(&self, map: impl FnMut(&ScriptTypeTargetFact) -> Self) -> Self {
        let mut result = Self::join(self.possible.iter().map(map));
        result.complete &= self.complete;
        result
    }
}

impl HirSemanticFacts {
    pub(super) fn owned_field_fact(
        &self,
        graph: &ModuleGraph,
        field: &vela_hir::body::HirField,
        schema: Option<&crate::registry::RegistryFacts>,
    ) -> crate::type_fact::TypeFact {
        use crate::type_fact::TypeFact;
        if let Some(origins) = self.source_origins.get(&field.receiver)
            && !origins.possible().is_empty()
        {
            let mut facts = origins
                .possible()
                .iter()
                .filter_map(|owner| {
                    super::targets::source_field_fact(graph, owner, &field.name, schema)
                        .map(|field| field.fact)
                })
                .collect::<Vec<_>>();
            facts.sort_by_key(TypeFact::display_name);
            return if facts.is_empty() {
                TypeFact::Unknown
            } else {
                TypeFact::union(facts)
            };
        }
        super::lookups::field_fact(
            graph,
            self.script_types.get(&field.receiver),
            &self.fact(field.receiver),
            &field.name,
            schema,
        )
    }

    pub(super) fn infer_source_origins(
        &self,
        graph: &ModuleGraph,
        body: &HirBody,
        id: HirExprId,
        base: &crate::facts::AnalysisFacts,
    ) -> ScriptTypeOrigins {
        let Some(expression) = body.expression(id) else {
            return ScriptTypeOrigins::default();
        };
        let origins = |id| self.source_origins.get(&id).cloned().unwrap_or_default();
        let join = |results: Vec<Option<HirExprId>>| {
            ScriptTypeOrigins::join(
                results
                    .into_iter()
                    .map(|id| id.map_or_else(ScriptTypeOrigins::default, origins)),
            )
        };
        match &expression.kind {
            HirExprKind::Path(_)
                if matches!(base.resolution(id), Some(BindingResolution::Local(_))) =>
            {
                self.local_use_origins.get(&id).cloned().unwrap_or_else(|| {
                    self.script_types
                        .get(&id)
                        .cloned()
                        .map(ScriptTypeOrigins::known)
                        .unwrap_or_default()
                })
            }
            HirExprKind::Paren { expression }
            | HirExprKind::Await { expression }
            | HirExprKind::Try { expression } => {
                expression.map_or_else(ScriptTypeOrigins::default, origins)
            }
            HirExprKind::Assign { value, .. } => {
                value.map_or_else(ScriptTypeOrigins::default, origins)
            }
            HirExprKind::Block { block } => join(super::value_flow::block_results(body, *block)),
            HirExprKind::If(value) => join(super::value_flow::if_results(body, value)),
            HirExprKind::Match(value) => join(super::value_flow::match_results(body, value)),
            HirExprKind::Field(field) => origins(field.receiver).map_targets(|owner| {
                super::targets::source_field_fact(graph, owner, &field.name, None)
                    .and_then(|field| field.target)
                    .map(ScriptTypeOrigins::known)
                    .unwrap_or_default()
            }),
            HirExprKind::Call(call) => {
                if let Some(lambda) = super::targets::direct_lambda_body(body, call.callee)
                    .and_then(|id| graph.body(id))
                {
                    return join(super::value_flow::body_results(lambda));
                }
                if let Some(field) = body.field(call.callee)
                    && let Some(receivers) = self.source_origins.get(&field.receiver)
                    && !receivers.possible().is_empty()
                {
                    return receivers.map_targets(|owner| {
                        super::lookups::source_method_return(
                            graph,
                            &self.fact(field.receiver),
                            Some(owner),
                            &field.name,
                            None,
                        )
                        .and_then(|method| method.target)
                        .map(ScriptTypeOrigins::known)
                        .unwrap_or_default()
                    });
                }
                self.script_types
                    .get(&id)
                    .cloned()
                    .map(ScriptTypeOrigins::known)
                    .unwrap_or_default()
            }
            _ => self
                .script_types
                .get(&id)
                .cloned()
                .map(ScriptTypeOrigins::known)
                .unwrap_or_default(),
        }
    }
}
