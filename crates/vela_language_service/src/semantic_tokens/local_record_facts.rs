use std::collections::BTreeMap;

use vela_analysis::type_fact::TypeFact;
use vela_common::SourceId;
use vela_hir::{
    body::{HirBody, HirPathKind, HirPathOwner, HirStmt, HirStmtTag},
    ids::HirLocalId,
    module_graph::ModuleGraph,
};

pub(super) fn collect(graph: &ModuleGraph, source_id: SourceId) -> BTreeMap<HirLocalId, TypeFact> {
    let mut facts = BTreeMap::new();
    for body in graph.bodies() {
        for statement in body.statements.values() {
            if statement.origin.source != source_id {
                continue;
            }
            let Some((local, record_fact)) = local_record_fact(body, statement) else {
                continue;
            };
            facts.insert(local, record_fact);
        }
    }
    facts
}

fn local_record_fact(body: &HirBody, statement: &HirStmt) -> Option<(HirLocalId, TypeFact)> {
    if statement.tag() != HirStmtTag::Let {
        return None;
    }
    let initializer = statement.initializer()?;
    let record_path = body
        .paths
        .iter()
        .find(|path| {
            path.kind == HirPathKind::Constructor
                && path.owner == HirPathOwner::Expression(initializer)
                && !path.path.is_empty()
        })?
        .path
        .join("::");
    let local = local_for_statement(body, statement)?;
    Some((local, TypeFact::record(record_path)))
}

fn local_for_statement(body: &HirBody, statement: &HirStmt) -> Option<HirLocalId> {
    statement.patterns().iter().find_map(|pattern| {
        let pattern = body.patterns.get(pattern)?;
        if !pattern.is_binding() {
            return None;
        }
        pattern.local()
    })
}
