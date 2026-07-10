use std::collections::{BTreeMap, btree_map::Entry};

use vela_def::FunctionId;
use vela_hir::body::HirBodyOwner;
use vela_hir::ids::{HirBodyId, HirExprId, HirLocalId, HirParamId};
use vela_hir::module_graph::ModuleGraph;

use crate::{MirSourceOrigin, MirTypeContract};

use super::{
    CompileFunctionTargets, CompileTargetSnapshot, CompileTargetSnapshotBuilder, MirBuildError,
};

/// Backend-neutral entry contract for one nested lambda parameter.
///
/// Lambda parameters deliberately retain HIR-local identity. They are not
/// assigned stable [`FunctionId`] identities because nested functions are
/// generation-local implementation details.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileLambdaParameterTarget {
    pub parameter: HirParamId,
    pub local: HirLocalId,
    pub name: String,
    pub contract: Option<MirTypeContract>,
    pub origin: MirSourceOrigin,
}

/// Compile-time identity and entry contract for a nested lambda body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileLambdaTarget {
    pub body: HirBodyId,
    /// Nearest enclosing runtime function or lambda body. Parameter-default
    /// bodies are prologue regions and therefore are not nested functions.
    pub parent: HirBodyId,
    pub expression: HirExprId,
    pub code_symbol: String,
    pub parameters: Vec<CompileLambdaParameterTarget>,
    pub origin: MirSourceOrigin,
}

impl CompileLambdaTarget {
    #[must_use]
    pub fn parameter(&self, parameter: HirParamId) -> Option<&CompileLambdaParameterTarget> {
        self.parameters
            .iter()
            .find(|target| target.parameter == parameter)
    }
}

impl<'a> CompileFunctionTargets<'a> {
    #[must_use]
    pub fn lambda(self, body: HirBodyId) -> Option<&'a CompileLambdaTarget> {
        self.snapshot.lambda(self.function(), body)
    }

    #[must_use]
    pub fn lambda_parameter(
        self,
        body: HirBodyId,
        parameter: HirParamId,
    ) -> Option<&'a CompileLambdaParameterTarget> {
        self.lambda(body)?.parameter(parameter)
    }

    pub fn lambdas(self) -> impl Iterator<Item = &'a CompileLambdaTarget> {
        let function = self.function();
        self.snapshot
            .lambdas
            .range((function, HirBodyId::new(0))..=(function, HirBodyId::new(u32::MAX)))
            .map(|(_, target)| target)
    }
}

impl CompileTargetSnapshot {
    #[must_use]
    pub fn lambda(&self, function: FunctionId, body: HirBodyId) -> Option<&CompileLambdaTarget> {
        self.lambdas.get(&(function, body))
    }
}

impl CompileTargetSnapshotBuilder {
    pub fn insert_lambda(
        &mut self,
        function: FunctionId,
        target: CompileLambdaTarget,
    ) -> Result<(), MirBuildError> {
        let key = (function, target.body);
        match self.snapshot.lambdas.entry(key) {
            Entry::Vacant(entry) => {
                let origin = target.origin;
                entry.insert(target);
                self.snapshot.origins.lambdas.insert(key, origin);
                Ok(())
            }
            Entry::Occupied(_) => Err(MirBuildError::InconsistentInput {
                origin: target.origin,
                message: format!(
                    "duplicate lambda target for function #{} and HIR body {:?}",
                    function.get(),
                    target.body
                ),
            }),
        }
    }
}

pub(super) fn validate_hir_closure(
    graph: &ModuleGraph,
    snapshot: &CompileTargetSnapshot,
    function: FunctionId,
    root: HirBodyId,
    root_symbol: &str,
) -> Result<(), MirBuildError> {
    let root_body = graph
        .body(root)
        .ok_or(MirBuildError::MissingCompilationRoot {
            function,
            body: root,
        })?;
    let root_origin = MirSourceOrigin::body(root, root_body.origin.span);
    let mut expected = graph
        .bodies()
        .filter(|body| matches!(body.owner, HirBodyOwner::Lambda { .. }))
        .filter(|body| {
            graph
                .body_and_ancestors(body.id)
                .any(|ancestor| ancestor.id == root)
        })
        .map(|body| {
            let depth = graph
                .body_and_ancestors(body.id)
                .filter(|ancestor| matches!(ancestor.owner, HirBodyOwner::Lambda { .. }))
                .count();
            (depth, body.origin.span, body.id)
        })
        .collect::<Vec<_>>();
    expected.sort_unstable_by_key(|(depth, span, body)| {
        (*depth, span.source, span.start, span.end, *body)
    });
    let actual = snapshot
        .lambdas
        .keys()
        .filter(|(owner, _)| *owner == function)
        .count();
    if actual != expected.len() {
        return Err(inconsistent(
            root_origin,
            format!(
                "executable root #{} owns {} HIR lambda bodies but {} compile targets",
                function.get(),
                expected.len(),
                actual
            ),
        ));
    }

    let mut symbols = BTreeMap::from([(root, root_symbol.to_owned())]);
    for (_, _, body) in expected {
        validate_hir_lambda(graph, snapshot, function, root, body, &mut symbols)?;
    }
    Ok(())
}

fn validate_hir_lambda(
    graph: &ModuleGraph,
    snapshot: &CompileTargetSnapshot,
    function: FunctionId,
    root: HirBodyId,
    body: HirBodyId,
    symbols: &mut BTreeMap<HirBodyId, String>,
) -> Result<(), MirBuildError> {
    let hir_body = graph.body(body).ok_or(MirBuildError::MissingHirBody {
        body,
        origin: snapshot
            .lambda(function, body)
            .map_or_else(|| graph_root_origin(graph, root), |target| target.origin),
    })?;
    let origin = MirSourceOrigin::body(body, hir_body.origin.span);
    let target = snapshot.lambda(function, body).ok_or_else(|| {
        inconsistent(
            origin,
            format!("missing compile target for lambda HIR body {body:?}"),
        )
    })?;
    let HirBodyOwner::Lambda {
        parent: hir_parent,
        expression,
    } = hir_body.owner
    else {
        return Err(inconsistent(
            origin,
            "lambda target refers to a non-lambda HIR body",
        ));
    };
    let parent = graph
        .body_and_ancestors(hir_parent)
        .find(|candidate| {
            candidate.id == root || matches!(candidate.owner, HirBodyOwner::Lambda { .. })
        })
        .map(|candidate| candidate.id)
        .ok_or_else(|| inconsistent(origin, "lambda HIR body has no executable parent"))?;
    if target.parent != parent || target.expression != expression || target.origin != origin {
        return Err(inconsistent(
            origin,
            "lambda target disagrees with its HIR owner, expression, or origin",
        ));
    }
    let parent_symbol = symbols.get(&parent).ok_or_else(|| {
        inconsistent(
            origin,
            format!("lambda target has unresolved executable parent {parent:?}"),
        )
    })?;
    let expected_symbol = format!("{parent_symbol}::<lambda@{}>", hir_body.origin.span.start);
    if target.code_symbol != expected_symbol {
        return Err(inconsistent(
            origin,
            format!(
                "lambda target code symbol {:?} does not match {:?}",
                target.code_symbol, expected_symbol
            ),
        ));
    }
    let bindings = graph
        .bindings_for_body(body)
        .ok_or_else(|| inconsistent(origin, "lambda HIR body has no binding generation"))?;
    if target.parameters.len() != hir_body.params.len() {
        return Err(inconsistent(
            origin,
            "lambda target parameter count disagrees with Heavy HIR",
        ));
    }
    for (target, parameter) in target.parameters.iter().zip(&hir_body.params) {
        let binding = bindings.local(parameter.local).ok_or_else(|| {
            inconsistent(
                origin,
                format!("lambda parameter {:?} has no local binding", parameter.id),
            )
        })?;
        let parameter_origin = MirSourceOrigin::body(body, parameter.origin.span);
        if target.parameter != parameter.id
            || target.local != parameter.local
            || target.name != binding.name
            || target.origin != parameter_origin
        {
            return Err(inconsistent(
                parameter_origin,
                format!(
                    "lambda parameter {:?} target disagrees with Heavy HIR order or identity",
                    parameter.id
                ),
            ));
        }
        if binding.type_hint.is_none() && target.contract.is_some() {
            return Err(inconsistent(
                parameter_origin,
                format!(
                    "untyped lambda parameter {:?} owns a compile-time contract",
                    parameter.id
                ),
            ));
        }
    }
    symbols.insert(body, target.code_symbol.clone());
    Ok(())
}

fn graph_root_origin(graph: &ModuleGraph, root: HirBodyId) -> MirSourceOrigin {
    let body = graph.body(root).expect("validated MIR root body");
    MirSourceOrigin::body(root, body.origin.span)
}

fn inconsistent(origin: MirSourceOrigin, message: impl Into<String>) -> MirBuildError {
    MirBuildError::InconsistentInput {
        origin,
        message: message.into(),
    }
}
