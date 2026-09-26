use vela_hir::body::{HirBody, HirCall};
use vela_hir::ids::{HirBodyId, HirExprId, HirLocalId};
use vela_hir::module_graph::ModuleGraph;

use crate::facts::AnalysisFacts;
use crate::registry::RegistryFacts;
use crate::stdlib::{StdlibMethodFact, StdlibParameterMetadata, stdlib_method_fact_for_call};
use crate::type_fact::TypeFact;

use super::local_flow::refine_local_fact;
use super::lookups::type_owner;
use super::targets::{ScriptTypeTargetFact, direct_lambda_body};
use super::{HirSemanticFacts, source_declaration_for_path};

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
        if let Some(context) = self.contextual_stdlib_call(graph, body, call) {
            return match (context.lambda_body, context.method.lambda) {
                (Some(lambda_body), Some(lambda)) => {
                    lambda_param_seeds(graph, lambda_body, lambda.params)
                }
                _ => Vec::new(),
            };
        }

        let mut seeds = Vec::new();
        if call
            .arguments
            .iter()
            .any(|argument| argument.name.is_some())
        {
            return seeds;
        }
        let TypeFact::Function { params, .. } =
            self.resolved_callable_fact(graph, body, call, schema)
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
        self.contextual_stdlib_call(graph, body, call)
            .map(|context| context.method)
    }

    fn contextual_stdlib_call(
        &self,
        graph: &ModuleGraph,
        body: &HirBody,
        call: &HirCall,
    ) -> Option<StdlibCallContext> {
        let field = body.field(call.callee)?;
        let receiver = self.fact(field.receiver);
        let method = stdlib_method_fact_for_call(&receiver, &field.name, None, None, &[])?;
        let slots = method
            .parameter_metadata()
            .and_then(|parameters| argument_slots(call, &parameters));
        let Some(slots) = slots else {
            // Invalid or ambiguous placement must not lend a lambda or argument
            // fact to another parameter. Validation owns the diagnostics.
            return Some(StdlibCallContext {
                method,
                lambda_body: None,
            });
        };
        let callback = method.lambda.as_ref().and_then(|_| {
            method
                .params
                .iter()
                .position(|fact| matches!(fact, TypeFact::Function { .. }))
                .and_then(|index| slots[index])
        });
        let lambda = direct_lambda_context(self, graph, body, callback);
        let arguments = slots
            .iter()
            .map(|value| value.map_or(TypeFact::Unknown, |value| self.fact(value)))
            .collect::<Vec<_>>();
        let method = stdlib_method_fact_for_call(
            &receiver,
            &field.name,
            lambda.as_ref().and_then(|context| context.returns.as_ref()),
            lambda.as_ref().map(|context| context.param_count),
            &arguments,
        )?;
        Some(StdlibCallContext {
            method,
            lambda_body: lambda.map(|context| context.body),
        })
    }

    fn resolved_callable_fact(
        &self,
        graph: &ModuleGraph,
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
        super::external_calls::path(graph, body, call.callee)
            .and_then(|path| schema?.function_fact(&path))
            .cloned()
            .unwrap_or(TypeFact::Unknown)
    }
}

struct StdlibCallContext {
    method: StdlibMethodFact,
    lambda_body: Option<HirBodyId>,
}

fn argument_slots(
    call: &HirCall,
    parameters: &[StdlibParameterMetadata],
) -> Option<Vec<Option<HirExprId>>> {
    let mut slots = vec![None; parameters.len()];
    let mut occupied = vec![false; parameters.len()];
    let mut seen_named = false;
    for (ordinal, argument) in call.arguments.iter().enumerate() {
        let index = if let Some(name) = argument.name.as_deref() {
            seen_named = true;
            parameters
                .iter()
                .position(|parameter| parameter.name == name)?
        } else if seen_named {
            return None;
        } else {
            ordinal
        };
        let occupied = occupied.get_mut(index)?;
        if *occupied {
            return None;
        }
        *occupied = true;
        slots[index] = argument.value;
    }
    Some(slots)
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
    callback: Option<HirExprId>,
) -> Option<DirectLambdaContext> {
    let lambda_body = direct_lambda_body(body, callback?)?;
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
