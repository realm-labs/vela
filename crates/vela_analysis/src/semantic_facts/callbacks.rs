use std::collections::BTreeSet;

use vela_hir::body::{HirBody, HirCall, HirPathKind};
use vela_hir::ids::{HirBodyId, HirLocalId};
use vela_hir::module_graph::ModuleGraph;

use crate::facts::AnalysisFacts;
use crate::registry::RegistryFacts;
use crate::stdlib::{StdlibMethodFact, stdlib_method_fact_for_call};
use crate::type_fact::TypeFact;

use super::local_flow::refine_local_fact;
use super::lookups::type_owner;
use super::targets::{ScriptTypeTargetFact, direct_lambda_body};
use super::{HirSemanticFacts, expression_path, source_declaration_for_path};

#[derive(Clone, Debug)]
struct CallbackSeed {
    local: HirLocalId,
    fact: TypeFact,
}

impl HirSemanticFacts {
    pub(super) fn infer_callback_params(
        &mut self,
        graph: &ModuleGraph,
        body: &HirBody,
        schema: Option<&RegistryFacts>,
        base: &AnalysisFacts,
    ) {
        let mut seeds = Vec::new();
        for expression in body.expressions.values() {
            let vela_hir::body::HirExprKind::Call(call) = &expression.kind else {
                continue;
            };
            seeds.extend(self.callback_seeds(graph, body, call, schema));
        }

        for seed in seeds {
            if matches!(seed.fact, TypeFact::Unknown) {
                continue;
            }
            let declared = base.local(seed.local);
            let fact = declared.map_or(seed.fact.clone(), |declared| {
                refine_local_fact(declared, seed.fact)
            });
            let script_type = source_script_type(graph, &fact);
            self.locals.insert(seed.local, fact);
            match script_type {
                Some(script_type) if base.base_local_script_type(seed.local).is_none() => {
                    self.local_script_types.insert(seed.local, script_type);
                }
                Some(_) => {}
                None if base.base_local_script_type(seed.local).is_none() => {
                    self.local_script_types.remove(&seed.local);
                }
                None => {}
            }
        }
    }

    fn callback_seeds(
        &self,
        graph: &ModuleGraph,
        body: &HirBody,
        call: &HirCall,
        schema: Option<&RegistryFacts>,
    ) -> Vec<CallbackSeed> {
        let mut seeds = Vec::new();
        let mut specialized_bodies = BTreeSet::new();
        if let Some((lambda_body, method)) = self.contextual_stdlib_callback(graph, body, call) {
            let params = method
                .lambda
                .map(|lambda| lambda.params)
                .unwrap_or_default();
            seeds.extend(lambda_param_seeds(graph, lambda_body, params));
            specialized_bodies.insert(lambda_body);
        }

        if call
            .arguments
            .iter()
            .any(|argument| argument.name.is_some())
        {
            return seeds;
        }
        let TypeFact::Function { params, .. } = self.resolved_callable_fact(body, call, schema)
        else {
            return seeds;
        };
        for (argument, expected) in call.arguments.iter().zip(params) {
            let (Some(value), TypeFact::Function { params, .. }) = (argument.value, expected)
            else {
                continue;
            };
            let Some(lambda_body) = direct_lambda_body(body, value) else {
                continue;
            };
            if specialized_bodies.contains(&lambda_body) {
                continue;
            }
            seeds.extend(lambda_param_seeds(graph, lambda_body, params));
        }
        seeds
    }

    pub(super) fn contextual_stdlib_method_fact(
        &self,
        graph: &ModuleGraph,
        body: &HirBody,
        call: &HirCall,
    ) -> Option<StdlibMethodFact> {
        let field = body.field(call.callee)?;
        let receiver = self.fact(field.receiver);
        let lambda = direct_lambda_context(self, graph, body, call);
        let arguments = call
            .arguments
            .iter()
            .map(|argument| {
                argument
                    .value
                    .map_or(TypeFact::Unknown, |value| self.fact(value))
            })
            .collect::<Vec<_>>();
        stdlib_method_fact_for_call(
            &receiver,
            &field.name,
            lambda.as_ref().and_then(|context| context.returns.as_ref()),
            lambda.as_ref().map(|context| context.param_count),
            &arguments,
        )
    }

    fn contextual_stdlib_callback(
        &self,
        graph: &ModuleGraph,
        body: &HirBody,
        call: &HirCall,
    ) -> Option<(HirBodyId, StdlibMethodFact)> {
        let context = direct_lambda_context(self, graph, body, call)?;
        let method = self.contextual_stdlib_method_fact(graph, body, call)?;
        method.lambda.as_ref()?;
        Some((context.body, method))
    }

    fn resolved_callable_fact(
        &self,
        body: &HirBody,
        call: &HirCall,
        schema: Option<&RegistryFacts>,
    ) -> TypeFact {
        let direct = self.fact(call.callee);
        if matches!(direct, TypeFact::Function { .. }) {
            return direct;
        }
        if let Some(field) = body.field(call.callee) {
            let receiver = self.fact(field.receiver);
            return type_owner(&receiver)
                .and_then(|owner| schema?.method_fact(owner, &field.name))
                .cloned()
                .unwrap_or(TypeFact::Unknown);
        }
        expression_path(body, call.callee, HirPathKind::Callee)
            .map(|path| path.join("::"))
            .and_then(|path| schema?.function_fact(&path))
            .cloned()
            .unwrap_or(TypeFact::Unknown)
    }
}

struct DirectLambdaContext {
    body: HirBodyId,
    returns: Option<TypeFact>,
    param_count: usize,
}

fn direct_lambda_context(
    facts: &HirSemanticFacts,
    graph: &ModuleGraph,
    body: &HirBody,
    call: &HirCall,
) -> Option<DirectLambdaContext> {
    let mut lambdas = call.arguments.iter().filter_map(|argument| {
        argument
            .value
            .and_then(|value| direct_lambda_body(body, value))
    });
    let lambda_body = lambdas.next()?;
    if lambdas.next().is_some() {
        return None;
    }
    let lambda = graph.body(lambda_body)?;
    let returns = facts.body_value(lambda);
    Some(DirectLambdaContext {
        body: lambda_body,
        returns: (!matches!(returns, TypeFact::Unknown)).then_some(returns),
        param_count: lambda.params.len(),
    })
}

fn lambda_param_seeds(
    graph: &ModuleGraph,
    lambda_body: HirBodyId,
    params: Vec<TypeFact>,
) -> Vec<CallbackSeed> {
    graph.body(lambda_body).map_or_else(Vec::new, |body| {
        body.params
            .iter()
            .zip(params)
            .map(|(param, fact)| CallbackSeed {
                local: param.local,
                fact,
            })
            .collect()
    })
}

fn source_script_type(graph: &ModuleGraph, fact: &TypeFact) -> Option<ScriptTypeTargetFact> {
    let (name, variant) = match fact {
        TypeFact::Record { name } => (name, None),
        TypeFact::Enum { name, variant } => (name, variant.clone()),
        _ => return None,
    };
    let path = name.split("::").map(str::to_owned).collect::<Vec<_>>();
    let declaration = source_declaration_for_path(graph, &path)?.id;
    Some(ScriptTypeTargetFact {
        declaration,
        variant,
    })
}
