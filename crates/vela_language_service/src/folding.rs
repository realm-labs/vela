use std::collections::BTreeSet;

use vela_common::Span;
use vela_syntax::ast::{
    Argument, Block, ElseBranch, Expr, ExprKind, FunctionItem, ImplMethod, InterpolatedStringPart,
    Item, ItemKind, MapEntry, MatchArm, RecordField, Stmt, StmtKind, TraitMethod,
};

use crate::{DocumentId, LanguageServiceDatabases, LineIndex, Position, SourceRecord, TextRange};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub enum FoldingRangeKind {
    Imports,
    Region,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct FoldingRange {
    kind: FoldingRangeKind,
    start: Position,
    end: Position,
}

impl FoldingRange {
    #[must_use]
    pub const fn new(kind: FoldingRangeKind, start: Position, end: Position) -> Self {
        Self { kind, start, end }
    }

    #[must_use]
    pub const fn kind(self) -> FoldingRangeKind {
        self.kind
    }

    #[must_use]
    pub const fn start(self) -> Position {
        self.start
    }

    #[must_use]
    pub const fn end(self) -> Position {
        self.end
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
struct FoldingRangeKey {
    kind: FoldingRangeKind,
    start_line: usize,
    start_character: usize,
    end_line: usize,
    end_character: usize,
}

impl FoldingRangeKey {
    const fn new(kind: FoldingRangeKind, start: Position, end: Position) -> Self {
        Self {
            kind,
            start_line: start.line,
            start_character: start.character,
            end_line: end.line,
            end_character: end.character,
        }
    }

    const fn into_range(self) -> FoldingRange {
        FoldingRange::new(
            self.kind,
            Position::new(self.start_line, self.start_character),
            Position::new(self.end_line, self.end_character),
        )
    }
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn folding_ranges(&self, document_id: &DocumentId) -> Vec<FoldingRange> {
        let Some(source) = self.source_db().records().get(document_id) else {
            return Vec::new();
        };
        let Some(parsed) = self.parse_db().parsed_source(document_id) else {
            return Vec::new();
        };

        let line_index = LineIndex::new(source.text());
        let mut ranges = BTreeSet::new();
        collect_import_groups(&parsed.items, source, &line_index, &mut ranges);
        for item in &parsed.items {
            collect_item_ranges(item, source, &line_index, &mut ranges);
        }
        ranges
            .into_iter()
            .map(FoldingRangeKey::into_range)
            .collect()
    }
}

fn collect_import_groups(
    items: &[Item],
    source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    let mut group_start: Option<Span> = None;
    let mut group_end: Option<Span> = None;

    for item in items {
        if matches!(item.kind, ItemKind::Use(_)) {
            group_start.get_or_insert(item.span);
            group_end = Some(item.span);
        } else {
            push_import_group(
                group_start.take(),
                group_end.take(),
                source,
                line_index,
                ranges,
            );
        }
    }

    push_import_group(group_start, group_end, source, line_index, ranges);
}

fn push_import_group(
    start: Option<Span>,
    end: Option<Span>,
    source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    let (Some(start), Some(end)) = (start, end) else {
        return;
    };
    push_span_range(
        FoldingRangeKind::Imports,
        span_from_bounds(start, end),
        source,
        line_index,
        ranges,
    );
}

fn collect_item_ranges(
    item: &Item,
    source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    match &item.kind {
        ItemKind::Use(_) | ItemKind::Global(_) => {}
        ItemKind::Const(item) => collect_expr_ranges(&item.value, source, line_index, ranges),
        ItemKind::Function(function) => {
            push_span_range(
                FoldingRangeKind::Region,
                item.span,
                source,
                line_index,
                ranges,
            );
            collect_function_ranges(function, source, line_index, ranges);
        }
        ItemKind::Struct(_) | ItemKind::Enum(_) => {
            push_span_range(
                FoldingRangeKind::Region,
                item.span,
                source,
                line_index,
                ranges,
            );
        }
        ItemKind::Trait(trait_item) => {
            push_span_range(
                FoldingRangeKind::Region,
                item.span,
                source,
                line_index,
                ranges,
            );
            for method in &trait_item.methods {
                collect_trait_method_ranges(method, source, line_index, ranges);
            }
        }
        ItemKind::Impl(impl_item) => {
            push_span_range(
                FoldingRangeKind::Region,
                item.span,
                source,
                line_index,
                ranges,
            );
            for method in &impl_item.methods {
                collect_impl_method_ranges(method, source, line_index, ranges);
            }
        }
    }
}

fn collect_function_ranges(
    function: &FunctionItem,
    source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    collect_block_ranges(&function.body, source, line_index, ranges);
    for param in &function.params {
        if let Some(default_value) = &param.default_value {
            collect_expr_ranges(default_value, source, line_index, ranges);
        }
    }
}

fn collect_trait_method_ranges(
    method: &TraitMethod,
    source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    if let Some(body) = &method.default_body {
        collect_block_ranges(body, source, line_index, ranges);
    }
    for param in &method.params {
        if let Some(default_value) = &param.default_value {
            collect_expr_ranges(default_value, source, line_index, ranges);
        }
    }
}

fn collect_impl_method_ranges(
    method: &ImplMethod,
    source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    collect_function_ranges(&method.function, source, line_index, ranges);
}

fn collect_block_ranges(
    block: &Block,
    source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    push_span_range(
        FoldingRangeKind::Region,
        block.span,
        source,
        line_index,
        ranges,
    );
    for statement in &block.statements {
        collect_stmt_ranges(statement, source, line_index, ranges);
    }
}

fn collect_stmt_ranges(
    statement: &Stmt,
    source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    match &statement.kind {
        StmtKind::Let { value, .. } => {
            if let Some(value) = value {
                collect_expr_ranges(value, source, line_index, ranges);
            }
        }
        StmtKind::Return(value) => {
            if let Some(value) = value {
                collect_expr_ranges(value, source, line_index, ranges);
            }
        }
        StmtKind::Break | StmtKind::Continue => {}
        StmtKind::For { iterable, body, .. } => {
            collect_expr_ranges(iterable, source, line_index, ranges);
            collect_block_ranges(body, source, line_index, ranges);
        }
        StmtKind::Expr(expr) => collect_expr_ranges(expr, source, line_index, ranges),
        StmtKind::Block(block) => collect_block_ranges(block, source, line_index, ranges),
    }
}

fn collect_expr_ranges(
    expr: &Expr,
    source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    match &expr.kind {
        ExprKind::Literal(_) | ExprKind::InterpolatedString(_) => {
            if is_multiline_span(expr.span, line_index) {
                push_span_range(
                    FoldingRangeKind::Region,
                    expr.span,
                    source,
                    line_index,
                    ranges,
                );
            }
            if let ExprKind::InterpolatedString(parts) = &expr.kind {
                for part in parts {
                    if let InterpolatedStringPart::Expr(expr) = part {
                        collect_expr_ranges(expr, source, line_index, ranges);
                    }
                }
            }
        }
        ExprKind::Path(_) | ExprKind::SelfValue | ExprKind::Error => {}
        ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => {
            collect_expr_ranges(expr, source, line_index, ranges);
        }
        ExprKind::Binary { left, right, .. } => {
            collect_expr_ranges(left, source, line_index, ranges);
            collect_expr_ranges(right, source, line_index, ranges);
        }
        ExprKind::Assign { target, value, .. } => {
            collect_expr_ranges(target, source, line_index, ranges);
            collect_expr_ranges(value, source, line_index, ranges);
        }
        ExprKind::Field { base, .. } => collect_expr_ranges(base, source, line_index, ranges),
        ExprKind::Call { callee, args } => {
            collect_expr_ranges(callee, source, line_index, ranges);
            for arg in args {
                collect_argument_ranges(arg, source, line_index, ranges);
            }
        }
        ExprKind::Index { base, index } => {
            collect_expr_ranges(base, source, line_index, ranges);
            collect_expr_ranges(index, source, line_index, ranges);
        }
        ExprKind::Array(items) => {
            push_multiline_literal(expr, source, line_index, ranges);
            for item in items {
                collect_expr_ranges(item, source, line_index, ranges);
            }
        }
        ExprKind::Map(entries) => {
            push_multiline_literal(expr, source, line_index, ranges);
            for entry in entries {
                collect_map_entry_ranges(entry, source, line_index, ranges);
            }
        }
        ExprKind::Record { fields, .. } => {
            push_multiline_literal(expr, source, line_index, ranges);
            for field in fields {
                collect_record_field_ranges(field, source, line_index, ranges);
            }
        }
        ExprKind::Lambda { params, body } => {
            push_multiline_literal(expr, source, line_index, ranges);
            for param in params {
                if let Some(default_value) = &param.default_value {
                    collect_expr_ranges(default_value, source, line_index, ranges);
                }
            }
            collect_expr_ranges(body, source, line_index, ranges);
        }
        ExprKind::If(if_expr) => {
            collect_expr_ranges(&if_expr.condition, source, line_index, ranges);
            collect_block_ranges(&if_expr.then_branch, source, line_index, ranges);
            if let Some(else_branch) = &if_expr.else_branch {
                match else_branch {
                    ElseBranch::If(if_expr) => {
                        collect_expr_ranges(&if_expr.condition, source, line_index, ranges);
                        collect_block_ranges(&if_expr.then_branch, source, line_index, ranges);
                    }
                    ElseBranch::Block(block) => {
                        collect_block_ranges(block, source, line_index, ranges)
                    }
                }
            }
        }
        ExprKind::Match(match_expr) => {
            push_span_range(
                FoldingRangeKind::Region,
                expr.span,
                source,
                line_index,
                ranges,
            );
            collect_expr_ranges(&match_expr.scrutinee, source, line_index, ranges);
            for arm in &match_expr.arms {
                collect_match_arm_ranges(arm, source, line_index, ranges);
            }
        }
        ExprKind::Block(block) => collect_block_ranges(block, source, line_index, ranges),
    }
}

fn collect_argument_ranges(
    argument: &Argument,
    source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    collect_expr_ranges(&argument.value, source, line_index, ranges);
}

fn collect_map_entry_ranges(
    entry: &MapEntry,
    source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    collect_expr_ranges(&entry.key, source, line_index, ranges);
    collect_expr_ranges(&entry.value, source, line_index, ranges);
}

fn collect_record_field_ranges(
    field: &RecordField,
    source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    if let Some(value) = &field.value {
        collect_expr_ranges(value, source, line_index, ranges);
    }
}

fn collect_match_arm_ranges(
    arm: &MatchArm,
    source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    if let Some(guard) = &arm.guard {
        collect_expr_ranges(guard, source, line_index, ranges);
    }
    push_span_range(
        FoldingRangeKind::Region,
        arm.body.span,
        source,
        line_index,
        ranges,
    );
    collect_expr_ranges(&arm.body, source, line_index, ranges);
}

fn push_multiline_literal(
    expr: &Expr,
    source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    if is_multiline_span(expr.span, line_index) {
        push_span_range(
            FoldingRangeKind::Region,
            expr.span,
            source,
            line_index,
            ranges,
        );
    }
}

fn push_span_range(
    kind: FoldingRangeKind,
    span: Span,
    _source: &SourceRecord,
    line_index: &LineIndex,
    ranges: &mut BTreeSet<FoldingRangeKey>,
) {
    let Some(range) = text_range(span) else {
        return;
    };
    let start = line_index.position(range.start);
    let end = line_index.position(range.end);
    if start.line < end.line {
        ranges.insert(FoldingRangeKey::new(kind, start, end));
    }
}

fn is_multiline_span(span: Span, line_index: &LineIndex) -> bool {
    let Some(range) = text_range(span) else {
        return false;
    };
    line_index.position(range.start).line < line_index.position(range.end).line
}

fn text_range(span: Span) -> Option<TextRange> {
    let start = usize::try_from(span.start).ok()?;
    let end = usize::try_from(span.end).ok()?;
    (start < end).then(|| TextRange::new(start, end))
}

fn span_from_bounds(start: Span, end: Span) -> Span {
    Span::new(start.source, start.start, end.end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
    };

    #[test]
    fn folding_ranges_cover_items_and_blocks() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "\
use game::reward::grant
use game::reward::Reward

pub struct Player {
    level: i64
}

pub fn main(player: Player) -> i64 {
    if player.level > 1 {
        return match player.level {
            1 => {
                return 1
            }
            _ => {
                return 2
            }
        }
    }
    return 0
}";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

        let ranges = databases.folding_ranges(&document);

        assert!(
            ranges
                .iter()
                .any(|range| range.kind() == FoldingRangeKind::Imports
                    && range.start().line == 0
                    && range.end().line == 1),
            "{ranges:?}"
        );
        assert!(
            ranges
                .iter()
                .any(|range| range.kind() == FoldingRangeKind::Region
                    && range.start().line == 3
                    && range.end().line == 5),
            "{ranges:?}"
        );
        assert!(
            ranges
                .iter()
                .any(|range| range.start().line == 7 && range.end().line == 19),
            "{ranges:?}"
        );
        assert!(
            ranges
                .iter()
                .any(|range| range.start().line == 9 && range.end().line == 16),
            "{ranges:?}"
        );
        assert!(
            ranges
                .iter()
                .any(|range| range.start().line == 10 && range.end().line == 12),
            "{ranges:?}"
        );
        assert!(
            ranges
                .iter()
                .any(|range| range.start().line == 13 && range.end().line == 15),
            "{ranges:?}"
        );
    }

    fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        databases
    }
}
