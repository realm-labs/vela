use vela_hir::binding::ConstructorResolution;
use vela_hir::body::{HirBody, HirRecordField};
use vela_hir::ids::{HirExprId, HirPathId};
use vela_hir::module_graph::ModuleGraph;

use crate::logical_records::{LogicalRecordKind, map_entry};
use crate::type_fact::TypeFact;

use super::MemberTargetFact;

pub(super) fn logical_member_target(receiver: &TypeFact, name: &str) -> Option<MemberTargetFact> {
    let TypeFact::LogicalRecord(record) = receiver else {
        return None;
    };
    Some(record.field_target(name).map_or(
        MemberTargetFact::Unresolved,
        MemberTargetFact::LogicalRecordField,
    ))
}

pub(super) fn logical_record_constructor_fact(
    kind: LogicalRecordKind,
    fields: &[HirRecordField],
    expression_fact: impl Fn(HirExprId) -> TypeFact,
) -> Option<TypeFact> {
    if kind != LogicalRecordKind::MapEntry || fields.len() != 2 {
        return None;
    }

    let mut key = None;
    let mut value = None;
    for field in fields {
        let fact = field
            .value
            .map(&expression_fact)
            .unwrap_or(TypeFact::Unknown);
        let target = match field.name.as_str() {
            "key" => &mut key,
            "value" => &mut value,
            _ => return None,
        };
        if target.replace(fact).is_some() {
            return None;
        }
    }
    Some(map_entry(key?, value?))
}

pub(super) fn logical_record_constructor_target(
    graph: &ModuleGraph,
    body: &HirBody,
    expression: HirExprId,
    constructor: Option<HirPathId>,
) -> Option<LogicalRecordKind> {
    let path = constructor.and_then(|constructor| body.paths.get(&constructor))?;
    let ConstructorResolution::Dynamic(resolved_path) = graph
        .bindings_for_body(body.id)
        .and_then(|bindings| bindings.constructor_resolution(expression))?
    else {
        return None;
    };
    (resolved_path == path.path)
        .then(|| LogicalRecordKind::from_source_constructor_path(&resolved_path))
        .flatten()
}
