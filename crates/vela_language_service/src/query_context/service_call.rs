use std::collections::{BTreeMap, BTreeSet};

use vela_analysis::semantic_facts::CallTargetFact;
use vela_hir::body::HirBodyOwner;
use vela_hir::ids::HirBodyId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::service_impl::ServiceImpl;

use super::QueryContext;
use crate::callable_context::{CallableFacts, service_callable_fact};
use crate::{LanguageServiceDatabases, SchemaServiceFact};

pub(super) fn is_service_path(path: &[String]) -> bool {
    matches!(path, [root, namespace, ..] if root == "service" && matches!(namespace.as_str(), "base" | "pinned"))
}

impl QueryContext<'_> {
    pub(crate) fn service_path_owner<'db>(
        &self,
        databases: &'db LanguageServiceDatabases,
        path: &[&str],
    ) -> Option<&'db SchemaServiceFact> {
        resolve_service_owner(databases, self.body()?.id, path)
    }

    pub(crate) fn is_service_call(&self) -> bool {
        self.call_argument_facts()
            .and_then(|call| call.callee_path())
            .is_some_and(is_service_path)
    }

    // Some(empty) owns invalid/unresolved reserved paths and prevents ordinary
    // source/schema lookup from lending them unrelated callable signatures.
    pub(super) fn service_callable_facts(
        &self,
        databases: &LanguageServiceDatabases,
        path: &[String],
    ) -> Option<Vec<CallableFacts>> {
        is_service_path(path).then(|| {
            self.resolve_service_callable(databases, path)
                .into_iter()
                .collect()
        })
    }

    fn resolve_service_callable(
        &self,
        databases: &LanguageServiceDatabases,
        path: &[String],
    ) -> Option<CallableFacts> {
        let graph = databases.hir_db().graph();
        let (body, call) = self.hir_call_for_cursor()?;
        if matches!(
            body.owner,
            HirBodyOwner::Lambda { .. } | HirBodyOwner::ParameterDefault { .. }
        ) {
            return None;
        }
        let callee = body.call(call)?.callee;
        graph
            .bindings_for_body(body.id)?
            .service_capability(callee)?;
        let (method, owner_path) = path.split_last()?;
        let owner = resolve_service_owner(
            databases,
            body.id,
            &owner_path.iter().map(String::as_str).collect::<Vec<_>>(),
        )?;
        owner
            .methods()
            .iter()
            .find(|candidate| candidate.name() == method)?;
        service_callable_fact(
            databases.schema_db().facts(),
            owner.path(),
            method,
            &path.join("::"),
        )
    }
}

fn resolve_service_owner<'db>(
    databases: &'db LanguageServiceDatabases,
    body: HirBodyId,
    path: &[&str],
) -> Option<&'db SchemaServiceFact> {
    let schema = databases.schema_db().service_set()?;
    match path {
        ["service", "base"] => {
            let origins = service_origins(databases, body);
            let [owner] = origins.as_slice() else {
                return None;
            };
            schema
                .services()
                .iter()
                .find(|service| service.path() == owner)
        }
        ["service", "pinned", member] => schema
            .services()
            .iter()
            .find(|service| service.member() == *member),
        _ => None,
    }
}

fn service_origins(databases: &LanguageServiceDatabases, target: HirBodyId) -> Vec<String> {
    let graph = databases.hir_db().graph();
    let facts = databases.graph_analysis_facts();
    // Match the compiler's static declaration-call reachability, including
    // calls in nested bodies, with a visited set for recursion/cycles.
    let mut callers: BTreeMap<HirBodyId, BTreeSet<HirBodyId>> = BTreeMap::new();
    for body in graph.bodies() {
        let root = root_body(graph, body.id);
        for (expression, _) in body.calls() {
            let Some(CallTargetFact::Declaration(declaration)) = facts.call_target(expression)
            else {
                continue;
            };
            let Some(callee) = graph.function_body(*declaration) else {
                continue;
            };
            callers.entry(callee.id).or_default().insert(root);
        }
    }
    let mut pending = vec![target];
    let mut reachable = BTreeSet::new();
    while let Some(body) = pending.pop() {
        if reachable.insert(body) {
            pending.extend(callers.get(&body).into_iter().flatten().copied());
        }
    }
    graph
        .declarations_by_kind(DeclarationKind::Impl)
        .into_iter()
        .filter_map(|declaration| {
            ServiceImpl::from_declaration(graph, declaration)
                .ok()
                .flatten()
        })
        .filter(|implementation| {
            implementation
                .methods()
                .any(|method| reachable.contains(&method.body()))
        })
        .map(|implementation| implementation.service_path_text())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn root_body(graph: &ModuleGraph, body: HirBodyId) -> HirBodyId {
    graph
        .body_and_ancestors(body)
        .last()
        .map_or(body, |body| body.id)
}
