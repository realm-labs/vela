mod walk;

use std::collections::{BTreeMap, BTreeSet};

use vela_hir::binding::BindingResolution;
use vela_hir::body::HirBody;
use vela_hir::ids::{HirExprId, HirLocalId};

use super::{HirSemanticFacts, pattern_local_facts};
use crate::facts::AnalysisFacts;
use crate::registry::RegistryFacts;
use crate::type_fact::TypeFact;

use super::ScriptTypeOrigins;

#[derive(Clone, Debug, PartialEq, Eq)]
struct LocalValue {
    fact: TypeFact,
    origins: ScriptTypeOrigins,
}

impl LocalValue {
    fn new(fact: TypeFact, origins: ScriptTypeOrigins) -> Self {
        let origins = if matches!(fact, TypeFact::Any | TypeFact::Unknown) {
            ScriptTypeOrigins::default()
        } else {
            origins
        };
        Self { fact, origins }
    }
}

type LocalEnvironment = BTreeMap<HirLocalId, LocalValue>;

impl HirSemanticFacts {
    pub(super) fn record_local_use_facts(
        &mut self,
        graph: &vela_hir::module_graph::ModuleGraph,
        body: &HirBody,
        schema: Option<&RegistryFacts>,
        base: &AnalysisFacts,
    ) {
        let mut environment =
            body_local_environment(&self.locals, &self.local_script_types, body, base);
        let mut flow = LocalFlow {
            graph,
            body,
            schema,
            base,
            expression_types: &self.types,
            source_origins: &self.source_origins,
            uses: BTreeMap::new(),
            loop_exits: Vec::new(),
        };
        flow.visit_root(&mut environment);
        for expression in body.expressions.keys() {
            self.local_use_types.remove(expression);
            self.local_use_origins.remove(expression);
        }
        for (expression, value) in flow.uses {
            self.local_use_types.insert(expression, value.fact);
            self.local_use_origins.insert(expression, value.origins);
        }
    }
}

struct LocalFlow<'facts> {
    graph: &'facts vela_hir::module_graph::ModuleGraph,
    body: &'facts HirBody,
    schema: Option<&'facts RegistryFacts>,
    base: &'facts AnalysisFacts,
    expression_types: &'facts BTreeMap<HirExprId, TypeFact>,
    source_origins: &'facts BTreeMap<HirExprId, ScriptTypeOrigins>,
    uses: BTreeMap<HirExprId, LocalValue>,
    loop_exits: Vec<Vec<LocalEnvironment>>,
}

impl LocalFlow<'_> {
    fn bind_pattern(
        &self,
        pattern: vela_hir::ids::HirPatternId,
        fact: &TypeFact,
        origins: &ScriptTypeOrigins,
        environment: &mut LocalEnvironment,
    ) {
        for inferred in pattern_local_facts(
            self.graph,
            self.schema,
            self.body,
            pattern,
            fact,
            origins.unique(),
        ) {
            let fact = self
                .base
                .local(inferred.local)
                .map_or(inferred.fact.clone(), |declared| {
                    refine_local_fact(declared, inferred.fact)
                });
            let origins = self
                .base
                .base_local_script_type(inferred.local)
                .cloned()
                .map(ScriptTypeOrigins::known)
                .unwrap_or_else(|| {
                    if matches!(
                        self.body
                            .patterns
                            .get(&pattern)
                            .map(|pattern| &pattern.kind),
                        Some(vela_hir::body::HirPatternKind::Binding { .. })
                    ) {
                        origins.clone()
                    } else {
                        inferred
                            .script_type
                            .clone()
                            .map(ScriptTypeOrigins::known)
                            .unwrap_or_default()
                    }
                });
            set_local(environment, inferred.local, LocalValue::new(fact, origins));
        }
    }

    fn origins(&self, expression: HirExprId, environment: &LocalEnvironment) -> ScriptTypeOrigins {
        if let Some(BindingResolution::Local(local)) = self.base.resolution(expression) {
            return environment
                .get(local)
                .map(|value| value.origins.clone())
                .unwrap_or_default();
        }
        self.source_origins
            .get(&expression)
            .cloned()
            .unwrap_or_default()
    }

    fn fact(&self, expression: HirExprId, environment: &LocalEnvironment) -> TypeFact {
        if let Some(BindingResolution::Local(local)) = self.base.resolution(expression) {
            return environment
                .get(local)
                .map(|value| value.fact.clone())
                .unwrap_or(TypeFact::Unknown);
        }
        self.expression_types
            .get(&expression)
            .cloned()
            .unwrap_or(TypeFact::Unknown)
    }
}

/// Preserves an explicit local contract while filling only its erased shape
/// slots from the value currently known to flow into that local.
pub(super) fn refine_local_fact(declared: &TypeFact, inferred: TypeFact) -> TypeFact {
    if matches!(inferred, TypeFact::Unknown | TypeFact::Never) {
        return declared.clone();
    }

    match (declared, inferred) {
        (TypeFact::Unknown, inferred) => inferred,
        (TypeFact::Any, _) => TypeFact::Any,
        (TypeFact::Array { element }, TypeFact::Array { element: inferred }) => {
            TypeFact::array(refine_local_fact(element, *inferred))
        }
        (TypeFact::ArrayView { element }, TypeFact::ArrayView { element: inferred }) => {
            TypeFact::array_view(refine_local_fact(element, *inferred))
        }
        (
            TypeFact::ArrayMut { element, mutation },
            TypeFact::ArrayMut {
                element: inferred,
                mutation: inferred_mutation,
            },
        ) if mutation == &inferred_mutation => {
            TypeFact::array_mut(refine_local_fact(element, *inferred), *mutation)
        }
        (
            TypeFact::Map { key, value },
            TypeFact::Map {
                key: inferred_key,
                value: inferred_value,
            },
        ) => TypeFact::map(
            refine_local_fact(key, *inferred_key),
            refine_local_fact(value, *inferred_value),
        ),
        (
            TypeFact::MapView { key, value },
            TypeFact::MapView {
                key: inferred_key,
                value: inferred_value,
            },
        ) => TypeFact::map_view(
            refine_local_fact(key, *inferred_key),
            refine_local_fact(value, *inferred_value),
        ),
        (
            TypeFact::MapMut {
                key,
                value,
                mutation,
            },
            TypeFact::MapMut {
                key: inferred_key,
                value: inferred_value,
                mutation: inferred_mutation,
            },
        ) if mutation == &inferred_mutation => TypeFact::map_mut(
            refine_local_fact(key, *inferred_key),
            refine_local_fact(value, *inferred_value),
            *mutation,
        ),
        (TypeFact::Set { element }, TypeFact::Set { element: inferred }) => {
            TypeFact::set(refine_local_fact(element, *inferred))
        }
        (TypeFact::SetView { element }, TypeFact::SetView { element: inferred }) => {
            TypeFact::set_view(refine_local_fact(element, *inferred))
        }
        (
            TypeFact::SetMut { element, mutation },
            TypeFact::SetMut {
                element: inferred,
                mutation: inferred_mutation,
            },
        ) if mutation == &inferred_mutation => {
            TypeFact::set_mut(refine_local_fact(element, *inferred), *mutation)
        }
        (TypeFact::Iterator { item }, TypeFact::Iterator { item: inferred }) => {
            TypeFact::iterator(refine_local_fact(item, *inferred))
        }
        (TypeFact::ScopedIterator { item }, TypeFact::ScopedIterator { item: inferred }) => {
            TypeFact::scoped_iterator(refine_local_fact(item, *inferred))
        }
        (TypeFact::Tuple { elements }, TypeFact::Tuple { elements: inferred })
            if elements.len() == inferred.len() =>
        {
            TypeFact::tuple(
                elements
                    .iter()
                    .zip(inferred)
                    .map(|(declared, inferred)| refine_local_fact(declared, inferred)),
            )
        }
        (TypeFact::Option { some }, TypeFact::Option { some: inferred }) => {
            TypeFact::option(refine_local_fact(some, *inferred))
        }
        (TypeFact::Option { some }, TypeFact::OptionSome { some: inferred }) => {
            TypeFact::option(refine_local_fact(some, *inferred))
        }
        (TypeFact::Option { .. }, TypeFact::OptionNone) => declared.clone(),
        (TypeFact::OptionSome { some }, TypeFact::OptionSome { some: inferred }) => {
            TypeFact::option_some(refine_local_fact(some, *inferred))
        }
        (
            TypeFact::Result { ok, err },
            TypeFact::Result {
                ok: inferred_ok,
                err: inferred_err,
            },
        ) => TypeFact::result(
            refine_local_fact(ok, *inferred_ok),
            refine_local_fact(err, *inferred_err),
        ),
        (TypeFact::Result { ok, err }, TypeFact::ResultOk { ok: inferred }) => {
            TypeFact::result(refine_local_fact(ok, *inferred), (**err).clone())
        }
        (TypeFact::Result { ok, err }, TypeFact::ResultErr { err: inferred }) => {
            TypeFact::result((**ok).clone(), refine_local_fact(err, *inferred))
        }
        (TypeFact::ResultOk { ok }, TypeFact::ResultOk { ok: inferred }) => {
            TypeFact::result_ok(refine_local_fact(ok, *inferred))
        }
        (TypeFact::ResultErr { err }, TypeFact::ResultErr { err: inferred }) => {
            TypeFact::result_err(refine_local_fact(err, *inferred))
        }
        (
            TypeFact::Function { params, returns },
            TypeFact::Function {
                params: inferred_params,
                returns: inferred_returns,
            },
        ) if params.is_empty() && matches!(returns.as_ref(), TypeFact::Unknown) => {
            TypeFact::function(inferred_params, *inferred_returns)
        }
        (
            TypeFact::Function { params, returns },
            TypeFact::Function {
                params: inferred_params,
                returns: inferred_returns,
            },
        ) if params.len() == inferred_params.len() => TypeFact::function(
            params
                .iter()
                .zip(inferred_params)
                .map(|(declared, inferred)| refine_local_fact(declared, inferred))
                .collect(),
            refine_local_fact(returns, *inferred_returns),
        ),
        (declared, _) => declared.clone(),
    }
}

fn set_local(environment: &mut LocalEnvironment, local: HirLocalId, fact: LocalValue) {
    if matches!(fact.fact, TypeFact::Unknown) {
        environment.remove(&local);
    } else {
        environment.insert(local, fact);
    }
}

/// Seeds flow narrowing with the locals a single body can observe.
///
/// The walk reads the environment only through [`LocalFlow::fact`], which looks
/// up locals reached by this body's binding resolutions, so seeding every local
/// in the workspace changed no answer while making each branch join copy a map
/// sized by the whole project.
fn body_local_environment(
    locals: &BTreeMap<HirLocalId, TypeFact>,
    source_locals: &BTreeMap<HirLocalId, super::ScriptTypeTargetFact>,
    body: &HirBody,
    base: &AnalysisFacts,
) -> LocalEnvironment {
    let declared = body
        .locals
        .iter()
        .copied()
        .chain(body.params.iter().map(|param| param.local))
        .chain(body.self_binding);
    let resolved =
        body.expressions
            .keys()
            .filter_map(|expression| match base.resolution(*expression) {
                Some(BindingResolution::Local(local)) => Some(*local),
                _ => None,
            });
    declared
        .chain(resolved)
        .filter_map(|local| {
            locals.get(&local).map(|fact| {
                (
                    local,
                    LocalValue::new(
                        fact.clone(),
                        source_locals
                            .get(&local)
                            .or_else(|| base.base_local_script_type(local))
                            .cloned()
                            .map(ScriptTypeOrigins::known)
                            .unwrap_or_default(),
                    ),
                )
            })
        })
        .collect()
}

fn join_environments<'a>(
    environments: impl IntoIterator<Item = &'a LocalEnvironment>,
    base: &AnalysisFacts,
) -> LocalEnvironment {
    let environments = environments.into_iter().collect::<Vec<_>>();
    let locals = environments
        .iter()
        .flat_map(|environment| environment.keys().copied())
        .collect::<BTreeSet<_>>();
    let mut joined = LocalEnvironment::new();
    for local in locals {
        let Some(first) = environments
            .first()
            .and_then(|environment| environment.get(&local))
        else {
            continue;
        };
        if environments.iter().all(|environment| {
            environment
                .get(&local)
                .is_some_and(|value| value.fact == first.fact)
        }) {
            let origins = ScriptTypeOrigins::join(environments.iter().filter_map(|environment| {
                environment.get(&local).map(|value| value.origins.clone())
            }));
            joined.insert(local, LocalValue::new(first.fact.clone(), origins));
        } else if let Some(declared) = base.local(local) {
            joined.insert(
                local,
                LocalValue::new(
                    declared.clone(),
                    base.base_local_script_type(local)
                        .cloned()
                        .map(ScriptTypeOrigins::known)
                        .unwrap_or_default(),
                ),
            );
        }
    }
    joined
}
