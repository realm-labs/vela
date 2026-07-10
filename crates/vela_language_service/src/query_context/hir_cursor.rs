use vela_common::{SourceId, Span};
use vela_hir::body::HirBody;
use vela_hir::ids::HirExprId;
use vela_hir::module_graph::ModuleGraph;

use crate::{CursorContext, CursorContextKind, TextRange};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HirCallRanges {
    callee: TextRange,
    member_receiver: Option<TextRange>,
}

pub(super) fn refine_cursor_with_hir(
    graph: &ModuleGraph,
    source_id: SourceId,
    offset: usize,
    cursor: &mut CursorContext,
) {
    match cursor.kind() {
        CursorContextKind::MemberAccess => {
            if let Some(receiver) = hir_member_receiver_range(graph, source_id, offset) {
                cursor.refine_member_receiver(receiver);
            }
        }
        CursorContextKind::CallArgument => {
            if let Some(open) = cursor.call_open()
                && let Some(ranges) = hir_call_ranges(graph, source_id, offset, open)
            {
                cursor.refine_call_ranges(ranges.callee, ranges.member_receiver);
            }
        }
        CursorContextKind::Item
        | CursorContextKind::Statement
        | CursorContextKind::Expression
        | CursorContextKind::Pattern
        | CursorContextKind::Type
        | CursorContextKind::UseImport
        | CursorContextKind::ModulePath
        | CursorContextKind::RecordExpressionField
        | CursorContextKind::RecordTypeField
        | CursorContextKind::LambdaParameter
        | CursorContextKind::MapKey
        | CursorContextKind::RenameTarget
        | CursorContextKind::Unknown => {}
    }
}

fn hir_member_receiver_range(
    graph: &ModuleGraph,
    source_id: SourceId,
    offset: usize,
) -> Option<TextRange> {
    let offset = u32::try_from(offset).ok()?;
    graph
        .fields_in_source(source_id)
        .filter(|field| span_contains_cursor_offset(field.member_origin.span, offset))
        .filter_map(|field| {
            graph
                .expression_span(field.receiver)
                .and_then(span_text_range)
        })
        .min_by_key(|range| range.len())
}

fn hir_call_ranges(
    graph: &ModuleGraph,
    source_id: SourceId,
    offset: usize,
    open: usize,
) -> Option<HirCallRanges> {
    let (body, expression) = hir_call_at(graph, source_id, open, offset)?;
    let call = body.call(expression)?;
    let callee = body.expressions.get(&call.callee)?;
    let callee_range = span_text_range(callee.origin.span)?;
    let member_receiver = body
        .field(call.callee)
        .and_then(|field| body.expressions.get(&field.receiver))
        .and_then(|receiver| span_text_range(receiver.origin.span));
    Some(HirCallRanges {
        callee: callee_range,
        member_receiver,
    })
}

pub(super) fn hir_call_at(
    graph: &ModuleGraph,
    source_id: SourceId,
    open: usize,
    offset: usize,
) -> Option<(&HirBody, HirExprId)> {
    graph
        .bodies()
        .flat_map(|body| {
            body.calls().filter_map(move |(_, call)| {
                let call_expression = body.expressions.get(&call.expression)?;
                if call_expression.origin.source != source_id
                    || !span_contains_usize(call_expression.origin.span, open)
                    || !span_contains_usize(call_expression.origin.span, offset)
                {
                    return None;
                }
                Some((
                    call_expression
                        .origin
                        .span
                        .end
                        .saturating_sub(call_expression.origin.span.start),
                    body,
                    call.expression,
                ))
            })
        })
        .min_by_key(|(width, _, _)| *width)
        .map(|(_, body, expression)| (body, expression))
}

fn span_contains_usize(span: Span, offset: usize) -> bool {
    u32::try_from(offset).is_ok_and(|offset| span_contains_cursor_offset(span, offset))
}

fn span_contains_cursor_offset(span: Span, offset: u32) -> bool {
    span.start <= offset && offset <= span.end
}

fn span_text_range(span: Span) -> Option<TextRange> {
    Some(TextRange::new(
        usize::try_from(span.start).ok()?,
        usize::try_from(span.end).ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DocumentId, LanguageServiceDatabases, SourceFileSnapshot, Workspace, WorkspaceConfig,
        WorkspaceRoot, assemble_project_sources,
    };

    #[test]
    fn hir_cursor_ranges_use_body_facts() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let source = "pub fn current_player() -> Player { return Player { level: 1 } }\n\
                      pub fn main(player: Player, scores: Array<i64>) { player.level; grant(current_player().level); scores.filter(player); current_player().grant(player) }";
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let workspace = Workspace::new();
        let files = vec![SourceFileSnapshot::new(document.clone(), source)];
        let project = assemble_project_sources(&config, &files, &workspace.snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        let source_record = databases
            .source_db()
            .records()
            .get(&document)
            .expect("source record");
        let graph = databases.hir_db().graph();

        let member_start = source.find("player.level").expect("member receiver");
        let member_offset = source.find("level;").expect("member name") + "level".len();
        assert_eq!(
            hir_member_receiver_range(graph, source_record.source_id(), member_offset),
            Some(TextRange::new(member_start, member_start + "player".len()))
        );

        let call_start = source.find("grant(").expect("function call");
        let call_open = call_start + "grant".len();
        let call_ranges =
            hir_call_ranges(graph, source_record.source_id(), call_open + 1, call_open)
                .expect("function call ranges");
        assert_eq!(
            call_ranges.callee,
            TextRange::new(call_start, call_start + "grant".len())
        );
        assert_eq!(call_ranges.member_receiver, None);

        let method_receiver_start = source.find("scores.filter").expect("method receiver");
        let method_open = source.find("filter(").expect("method call") + "filter".len();
        let method_ranges = hir_call_ranges(
            graph,
            source_record.source_id(),
            method_open + 1,
            method_open,
        )
        .expect("method call ranges");
        assert_eq!(
            method_ranges.callee,
            TextRange::new(
                method_receiver_start,
                method_receiver_start + "scores.filter".len()
            )
        );
        assert_eq!(
            method_ranges.member_receiver,
            Some(TextRange::new(
                method_receiver_start,
                method_receiver_start + "scores".len()
            ))
        );

        let complex_receiver_start = source
            .find("current_player().grant")
            .expect("complex method receiver");
        let complex_open = source.find(".grant(").expect("complex method call") + ".grant".len();
        let complex_ranges = hir_call_ranges(
            graph,
            source_record.source_id(),
            complex_open + 1,
            complex_open,
        )
        .expect("complex method call ranges");
        assert_eq!(
            complex_ranges.member_receiver,
            Some(TextRange::new(
                complex_receiver_start,
                complex_receiver_start + "current_player()".len()
            ))
        );
    }
}
