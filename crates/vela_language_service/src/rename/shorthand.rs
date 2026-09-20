use std::collections::BTreeMap;

use vela_common::{SourceId, Span};
use vela_hir::body::{HirExprKind, HirPatternKind};
use vela_hir::module_graph::ModuleGraph;

pub(super) fn local_labels(graph: &ModuleGraph, source: SourceId) -> BTreeMap<(u32, u32), &str> {
    let mut labels = BTreeMap::new();
    for body in graph.bodies_in_source(source) {
        for expression in body.expressions.values() {
            if let HirExprKind::Record { fields, .. } = &expression.kind {
                for field in fields.iter().filter(|field| field.shorthand) {
                    let span = field.name_origin.span;
                    labels.insert((span.start, span.end), field.name.as_str());
                }
            }
        }
        for pattern in body.patterns.values() {
            if let HirPatternKind::RecordVariant { fields, .. } = &pattern.kind {
                for field in fields.iter().filter(|field| field.shorthand) {
                    let span = field.name_origin.span;
                    labels.insert((span.start, span.end), field.name.as_str());
                }
            }
        }
    }
    labels
}

pub(super) fn local_replacement(
    labels: &BTreeMap<(u32, u32), &str>,
    span: Span,
    name: &str,
) -> String {
    match labels.get(&(span.start, span.end)) {
        Some(label) if *label != name => format!("{label}: {name}"),
        _ => name.to_owned(),
    }
}
