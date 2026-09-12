use std::collections::BTreeMap;

use vela_analysis::type_fact::TypeFact;
use vela_hir::ids::HirLocalId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::service_impl::ServiceImpl;

use crate::incremental::SchemaDb;

pub(super) fn parameter_facts(
    graph: &ModuleGraph,
    schema: &SchemaDb,
) -> BTreeMap<HirLocalId, TypeFact> {
    let mut locals = BTreeMap::new();
    let Some(set) = schema.service_set() else {
        return locals;
    };
    for declaration in graph.declarations_by_kind(DeclarationKind::Impl) {
        let Ok(Some(implementation)) = ServiceImpl::from_declaration(graph, declaration) else {
            continue;
        };
        let Some(service) = set
            .services()
            .iter()
            .find(|s| s.path() == implementation.service_path_text())
        else {
            continue;
        };
        for method in implementation.methods() {
            if !service.methods().iter().any(|m| m.name() == method.name()) {
                continue;
            }
            let Some(contract) = schema
                .facts()
                .trait_method_signature_fact(service.path(), method.name())
            else {
                continue;
            };
            let signature = method.signature();
            // Mirror the source service boundary's structural checks. An
            // invalid method cannot lend parameter slots to another contract.
            if signature.asyncness != contract.asyncness
                || signature.params.len() != contract.parameters.len()
                || signature
                    .params
                    .iter()
                    .any(|p| p.default_value_span.is_some())
            {
                continue;
            }
            let Some(body) = graph.body(method.body()) else {
                continue;
            };
            if body.params.len() != contract.parameters.len() {
                continue;
            }
            for ((parameter, hint), registered) in body
                .params
                .iter()
                .zip(&signature.params)
                .zip(&contract.parameters)
            {
                if hint.type_hint.is_none() {
                    locals.insert(parameter.local, registered.type_fact.clone());
                }
            }
        }
    }
    locals
}
