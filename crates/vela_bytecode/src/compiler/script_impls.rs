//! Direct-lowerer adapter for the backend-neutral HIR method catalog.
//!
//! This richer view remains only while the old body emitter exists. MIR input
//! consumes [`vela_hir::script_methods::ScriptMethodCatalog`] directly.

use vela_def::MethodId;
use vela_hir::binding::BindingMap;
use vela_hir::body::HirBody;
use vela_hir::ids::HirBodyId;
use vela_hir::module_graph::ModuleGraph;
use vela_hir::script_methods::{ScriptMethod, ScriptMethodCatalog};
use vela_hir::type_hint::FunctionSignature;
use vela_mir::{MirBuildError, MirSourceOrigin};

use super::error::{CompileError, CompileErrorKind, CompileResult};
use super::param_defaults::{ParamDefaultValue, param_default_values_from_ids};

#[derive(Clone)]
pub(super) struct ScriptImplMethod<'graph> {
    pub(super) target_type: String,
    pub(super) method_name: String,
    pub(super) method_id: MethodId,
    pub(super) symbol: String,
    pub(super) origin: MirSourceOrigin,
    pub(super) default_values: Vec<Option<ParamDefaultValue>>,
    pub(super) body: HirBodyId,
    pub(super) signature: FunctionSignature,
    pub(super) bindings: &'graph BindingMap,
    pub(super) hir_bodies: Vec<&'graph HirBody>,
}

pub(super) fn direct_methods<'graph>(
    graph: &'graph ModuleGraph,
    catalog: &ScriptMethodCatalog,
) -> CompileResult<Vec<ScriptImplMethod<'graph>>> {
    catalog
        .methods()
        .map(|method| direct_method(graph, method))
        .collect()
}

fn direct_method<'graph>(
    graph: &'graph ModuleGraph,
    method: &ScriptMethod,
) -> CompileResult<ScriptImplMethod<'graph>> {
    let body = graph.body(method.body()).ok_or_else(|| {
        method_input_error(method, "catalog method body is missing from the HIR graph")
    })?;
    let bindings = graph.bindings_for_body(method.body()).ok_or_else(|| {
        method_input_error(
            method,
            "catalog method body has no direct-lowerer binding map",
        )
    })?;
    Ok(ScriptImplMethod {
        target_type: method.owner().target_type().to_owned(),
        method_name: method.name().to_owned(),
        method_id: method.method_id(),
        symbol: method.symbol_seed(),
        origin: MirSourceOrigin::body(body.id, body.origin.span),
        default_values: param_default_values_from_ids(
            method.parameter_default_bodies().iter().copied(),
            method.signature(),
        ),
        body: method.body(),
        signature: method.signature().clone(),
        bindings,
        hir_bodies: graph.bodies().collect(),
    })
}

fn method_input_error(method: &ScriptMethod, message: impl Into<String>) -> CompileError {
    let origin = MirSourceOrigin::body(method.body(), method.origin().span);
    CompileError::new(CompileErrorKind::MirInput(Box::new(
        MirBuildError::InconsistentInput {
            origin,
            message: format!(
                "script method `{}` for `{}`: {}",
                method.name(),
                method.owner().target_type(),
                message.into()
            ),
        },
    )))
    .with_span(method.origin().span)
}
