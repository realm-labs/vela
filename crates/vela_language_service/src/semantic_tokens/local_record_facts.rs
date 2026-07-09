use std::collections::BTreeMap;

use vela_analysis::type_fact::TypeFact;
use vela_common::{SourceId, Span};
use vela_hir::{
    body::{HirPatternKind, HirStmtKind},
    ids::HirLocalId,
    module_graph::ModuleGraph,
};
use vela_syntax::ast::{AstNode, SyntaxLetStmt, SyntaxSourceFile};
use vela_syntax::{Parse as SyntaxParse, TextRange as SyntaxTextRange, TextSize};

pub(super) fn collect(
    graph: &ModuleGraph,
    parsed: &SyntaxParse<SyntaxSourceFile>,
    source_id: SourceId,
) -> BTreeMap<HirLocalId, TypeFact> {
    let mut facts = BTreeMap::new();
    let source = parsed.tree();
    for statement in source
        .syntax()
        .descendants()
        .filter_map(SyntaxLetStmt::cast)
    {
        let Some((local, record_fact)) = local_record_fact(graph, source_id, &statement) else {
            continue;
        };
        facts.insert(local, record_fact);
    }
    facts
}

fn local_record_fact(
    graph: &ModuleGraph,
    source_id: SourceId,
    statement: &SyntaxLetStmt,
) -> Option<(HirLocalId, TypeFact)> {
    statement.name_text()?;
    let record = statement.initializer()?.as_record()?;
    let record_path = record.path_text()?;
    let statement_span = span_from_text_range(source_id, statement.syntax().text_range());
    let local = local_for_statement(graph, statement_span)?;
    Some((local, TypeFact::record(record_path)))
}

fn local_for_statement(graph: &ModuleGraph, statement_span: Span) -> Option<HirLocalId> {
    for body in graph.bodies() {
        let Some(statement) = body.statements.values().find(|statement| {
            statement.kind == HirStmtKind::Let && statement.origin.span == statement_span
        }) else {
            continue;
        };
        return statement.patterns.iter().find_map(|pattern| {
            let pattern = body.patterns.get(pattern)?;
            if pattern.kind != HirPatternKind::Binding {
                return None;
            }
            pattern.local
        });
    }
    None
}

fn span_from_text_range(source_id: SourceId, range: SyntaxTextRange) -> Span {
    Span::new(
        source_id,
        text_size_to_u32(range.start()),
        text_size_to_u32(range.end()),
    )
}

fn text_size_to_u32(size: TextSize) -> u32 {
    u32::from(size)
}
