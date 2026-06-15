use vela_common::Span;
use vela_syntax::ast::{
    Argument, Attribute, Block, ElseBranch, Expr, ExprKind, FunctionItem, ImplMethod,
    InterpolatedStringPart, Item, ItemKind, MapEntry, Param, RecordField, Stmt, StmtKind,
    StructField, TraitMethod, TypeHint,
};

use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, LineIndex, Position, SourceRecord,
    TextRange,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SelectionRange {
    range: DiagnosticRange,
    parent: Option<Box<SelectionRange>>,
}

impl SelectionRange {
    #[must_use]
    pub fn new(range: DiagnosticRange, parent: Option<SelectionRange>) -> Self {
        Self {
            range,
            parent: parent.map(Box::new),
        }
    }

    #[must_use]
    pub const fn range(&self) -> DiagnosticRange {
        self.range
    }

    #[must_use]
    pub fn parent(&self) -> Option<&SelectionRange> {
        self.parent.as_deref()
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
struct SelectionRangeKey {
    start: usize,
    end: usize,
}

impl SelectionRangeKey {
    const fn new(range: TextRange) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }

    const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn selection_ranges(
        &self,
        document_id: &DocumentId,
        positions: &[Position],
    ) -> Vec<SelectionRange> {
        let Some(source) = self.source_db().records().get(document_id) else {
            return positions.iter().copied().map(point_selection).collect();
        };
        let Some(parsed) = self.parse_db().parsed_source(document_id) else {
            return positions.iter().copied().map(point_selection).collect();
        };
        let line_index = LineIndex::new(source.text());

        positions
            .iter()
            .copied()
            .map(|position| {
                let offset = line_index.offset(position);
                let mut ranges = Vec::new();
                if let Some(token) = token_range_at(source.text(), offset) {
                    push_range(token, &mut ranges);
                }
                for item in &parsed.items {
                    collect_item_ranges(item, offset, &mut ranges);
                }
                build_selection_chain(source, &line_index, position, ranges)
            })
            .collect()
    }
}

fn collect_item_ranges(item: &Item, offset: usize, ranges: &mut Vec<SelectionRangeKey>) {
    if !push_span(item.span, offset, ranges) {
        return;
    }
    for attr in &item.attrs {
        collect_attribute_ranges(attr, offset, ranges);
    }
    match &item.kind {
        ItemKind::Use(_) => {}
        ItemKind::Const(item) => {
            collect_optional_type_hint_ranges(item.type_hint.as_ref(), offset, ranges);
            collect_expr_ranges(&item.value, offset, ranges);
        }
        ItemKind::Global(item) => collect_type_hint_ranges(&item.type_hint, offset, ranges),
        ItemKind::Function(function) => collect_function_ranges(function, offset, ranges),
        ItemKind::Struct(item) => {
            for field in &item.fields {
                collect_struct_field_ranges(field, offset, ranges);
            }
        }
        ItemKind::Enum(item) => {
            for variant in &item.variants {
                if !push_span(variant.span, offset, ranges) {
                    continue;
                }
                match &variant.fields {
                    vela_syntax::ast::EnumVariantFields::Unit => {}
                    vela_syntax::ast::EnumVariantFields::Tuple(params) => {
                        for param in params {
                            collect_param_ranges(param, offset, ranges);
                        }
                    }
                    vela_syntax::ast::EnumVariantFields::Record(fields) => {
                        for field in fields {
                            collect_struct_field_ranges(field, offset, ranges);
                        }
                    }
                }
            }
        }
        ItemKind::Trait(item) => {
            for method in &item.methods {
                collect_trait_method_ranges(method, offset, ranges);
            }
        }
        ItemKind::Impl(item) => {
            for method in &item.methods {
                collect_impl_method_ranges(method, offset, ranges);
            }
        }
    }
}

fn collect_attribute_ranges(attr: &Attribute, offset: usize, ranges: &mut Vec<SelectionRangeKey>) {
    push_span(attr.span, offset, ranges);
}

fn collect_function_ranges(
    function: &FunctionItem,
    offset: usize,
    ranges: &mut Vec<SelectionRangeKey>,
) {
    for param in &function.params {
        collect_param_ranges(param, offset, ranges);
    }
    collect_optional_type_hint_ranges(function.return_type.as_ref(), offset, ranges);
    collect_block_ranges(&function.body, offset, ranges);
}

fn collect_trait_method_ranges(
    method: &TraitMethod,
    offset: usize,
    ranges: &mut Vec<SelectionRangeKey>,
) {
    if !push_span(method.span, offset, ranges) {
        return;
    }
    for attr in &method.attrs {
        collect_attribute_ranges(attr, offset, ranges);
    }
    for param in &method.params {
        collect_param_ranges(param, offset, ranges);
    }
    collect_optional_type_hint_ranges(method.return_type.as_ref(), offset, ranges);
    if let Some(body) = &method.default_body {
        collect_block_ranges(body, offset, ranges);
    }
}

fn collect_impl_method_ranges(
    method: &ImplMethod,
    offset: usize,
    ranges: &mut Vec<SelectionRangeKey>,
) {
    for attr in &method.attrs {
        collect_attribute_ranges(attr, offset, ranges);
    }
    collect_function_ranges(&method.function, offset, ranges);
}

fn collect_param_ranges(param: &Param, offset: usize, ranges: &mut Vec<SelectionRangeKey>) {
    if !push_span(param.span, offset, ranges) {
        return;
    }
    collect_optional_type_hint_ranges(param.type_hint.as_ref(), offset, ranges);
    if let Some(default_value) = &param.default_value {
        collect_expr_ranges(default_value, offset, ranges);
    }
}

fn collect_struct_field_ranges(
    field: &StructField,
    offset: usize,
    ranges: &mut Vec<SelectionRangeKey>,
) {
    if !push_span(field.span, offset, ranges) {
        return;
    }
    for attr in &field.attrs {
        collect_attribute_ranges(attr, offset, ranges);
    }
    collect_optional_type_hint_ranges(field.type_hint.as_ref(), offset, ranges);
    if let Some(default_value) = &field.default_value {
        collect_expr_ranges(default_value, offset, ranges);
    }
}

fn collect_optional_type_hint_ranges(
    type_hint: Option<&TypeHint>,
    offset: usize,
    ranges: &mut Vec<SelectionRangeKey>,
) {
    if let Some(type_hint) = type_hint {
        collect_type_hint_ranges(type_hint, offset, ranges);
    }
}

fn collect_type_hint_ranges(
    type_hint: &TypeHint,
    offset: usize,
    ranges: &mut Vec<SelectionRangeKey>,
) {
    if !push_span(type_hint.span, offset, ranges) {
        return;
    }
    for arg in &type_hint.args {
        collect_type_hint_ranges(arg, offset, ranges);
    }
}

fn collect_block_ranges(block: &Block, offset: usize, ranges: &mut Vec<SelectionRangeKey>) {
    if !push_span(block.span, offset, ranges) {
        return;
    }
    for statement in &block.statements {
        collect_stmt_ranges(statement, offset, ranges);
    }
}

fn collect_stmt_ranges(statement: &Stmt, offset: usize, ranges: &mut Vec<SelectionRangeKey>) {
    if !push_span(statement.span, offset, ranges) {
        return;
    }
    for attr in &statement.attrs {
        collect_attribute_ranges(attr, offset, ranges);
    }
    match &statement.kind {
        StmtKind::Let {
            type_hint, value, ..
        } => {
            collect_optional_type_hint_ranges(type_hint.as_ref(), offset, ranges);
            if let Some(value) = value {
                collect_expr_ranges(value, offset, ranges);
            }
        }
        StmtKind::Return(value) => {
            if let Some(value) = value {
                collect_expr_ranges(value, offset, ranges);
            }
        }
        StmtKind::Break | StmtKind::Continue => {}
        StmtKind::For { iterable, body, .. } => {
            collect_expr_ranges(iterable, offset, ranges);
            collect_block_ranges(body, offset, ranges);
        }
        StmtKind::Expr(expr) => collect_expr_ranges(expr, offset, ranges),
        StmtKind::Block(block) => collect_block_ranges(block, offset, ranges),
    }
}

fn collect_expr_ranges(expr: &Expr, offset: usize, ranges: &mut Vec<SelectionRangeKey>) {
    if !push_span(expr.span, offset, ranges) {
        return;
    }
    match &expr.kind {
        ExprKind::Literal(_) | ExprKind::Path(_) | ExprKind::SelfValue | ExprKind::Error => {}
        ExprKind::InterpolatedString(parts) => {
            for part in parts {
                if let InterpolatedStringPart::Expr(expr) = part {
                    collect_expr_ranges(expr, offset, ranges);
                }
            }
        }
        ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => {
            collect_expr_ranges(expr, offset, ranges);
        }
        ExprKind::Binary { left, right, .. } => {
            collect_expr_ranges(left, offset, ranges);
            collect_expr_ranges(right, offset, ranges);
        }
        ExprKind::Assign { target, value, .. } => {
            collect_expr_ranges(target, offset, ranges);
            collect_expr_ranges(value, offset, ranges);
        }
        ExprKind::Field { base, .. } => collect_expr_ranges(base, offset, ranges),
        ExprKind::Call { callee, args } => {
            collect_expr_ranges(callee, offset, ranges);
            for arg in args {
                collect_argument_ranges(arg, offset, ranges);
            }
        }
        ExprKind::Index { base, index } => {
            collect_expr_ranges(base, offset, ranges);
            collect_expr_ranges(index, offset, ranges);
        }
        ExprKind::Array(items) => {
            for item in items {
                collect_expr_ranges(item, offset, ranges);
            }
        }
        ExprKind::Map(entries) => {
            for entry in entries {
                collect_map_entry_ranges(entry, offset, ranges);
            }
        }
        ExprKind::Record { fields, .. } => {
            for field in fields {
                collect_record_field_ranges(field, offset, ranges);
            }
        }
        ExprKind::Lambda { params, body } => {
            for param in params {
                collect_param_ranges(param, offset, ranges);
            }
            collect_expr_ranges(body, offset, ranges);
        }
        ExprKind::If(if_expr) => {
            collect_expr_ranges(&if_expr.condition, offset, ranges);
            collect_block_ranges(&if_expr.then_branch, offset, ranges);
            if let Some(else_branch) = &if_expr.else_branch {
                match else_branch {
                    ElseBranch::If(if_expr) => {
                        collect_expr_ranges(&if_expr.condition, offset, ranges);
                        collect_block_ranges(&if_expr.then_branch, offset, ranges);
                    }
                    ElseBranch::Block(block) => collect_block_ranges(block, offset, ranges),
                }
            }
        }
        ExprKind::Match(match_expr) => {
            collect_expr_ranges(&match_expr.scrutinee, offset, ranges);
            for arm in &match_expr.arms {
                if let Some(guard) = &arm.guard {
                    collect_expr_ranges(guard, offset, ranges);
                }
                collect_expr_ranges(&arm.body, offset, ranges);
            }
        }
        ExprKind::Block(block) => collect_block_ranges(block, offset, ranges),
    }
}

fn collect_argument_ranges(arg: &Argument, offset: usize, ranges: &mut Vec<SelectionRangeKey>) {
    collect_expr_ranges(&arg.value, offset, ranges);
}

fn collect_map_entry_ranges(entry: &MapEntry, offset: usize, ranges: &mut Vec<SelectionRangeKey>) {
    collect_expr_ranges(&entry.key, offset, ranges);
    collect_expr_ranges(&entry.value, offset, ranges);
}

fn collect_record_field_ranges(
    field: &RecordField,
    offset: usize,
    ranges: &mut Vec<SelectionRangeKey>,
) {
    if !push_span(field.span, offset, ranges) {
        return;
    }
    if let Some(value) = &field.value {
        collect_expr_ranges(value, offset, ranges);
    }
}

fn push_span(span: Span, offset: usize, ranges: &mut Vec<SelectionRangeKey>) -> bool {
    let Some(range) = text_range(span) else {
        return false;
    };
    let contains = range.start <= offset && offset < range.end;
    if contains {
        push_range(range, ranges);
    }
    contains
}

fn push_range(range: TextRange, ranges: &mut Vec<SelectionRangeKey>) {
    let key = SelectionRangeKey::new(range);
    if !ranges.contains(&key) {
        ranges.push(key);
    }
}

fn build_selection_chain(
    source: &SourceRecord,
    line_index: &LineIndex,
    position: Position,
    mut ranges: Vec<SelectionRangeKey>,
) -> SelectionRange {
    if ranges.is_empty() {
        return point_selection(position);
    }
    ranges.sort_by_key(|range| (range.len(), std::cmp::Reverse(range.start), range.end));
    ranges.dedup();

    let mut selection = None;
    for range in ranges.into_iter().rev() {
        let diagnostic_range = diagnostic_range(source.text(), line_index, range);
        selection = Some(SelectionRange::new(diagnostic_range, selection));
    }
    selection.unwrap_or_else(|| point_selection(position))
}

fn point_selection(position: Position) -> SelectionRange {
    SelectionRange::new(DiagnosticRange::new(position, position), None)
}

fn diagnostic_range(
    text: &str,
    line_index: &LineIndex,
    range: SelectionRangeKey,
) -> DiagnosticRange {
    let end = range.end.min(text.len());
    DiagnosticRange::new(line_index.position(range.start), line_index.position(end))
}

fn token_range_at(text: &str, offset: usize) -> Option<TextRange> {
    let offset = offset.min(text.len());
    let left = text[..offset]
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    let right = text[offset..]
        .char_indices()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(offset + index))
        .unwrap_or(text.len());
    if left < right {
        return Some(TextRange::new(left, right));
    }

    text[offset..].chars().next().and_then(|ch| {
        (!ch.is_whitespace()).then(|| TextRange::new(offset, offset + ch.len_utf8()))
    })
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn text_range(span: Span) -> Option<TextRange> {
    let start = usize::try_from(span.start).ok()?;
    let end = usize::try_from(span.end).ok()?;
    (start < end).then(|| TextRange::new(start, end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
    };

    #[test]
    fn selection_ranges_walk_syntax_ancestors() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "\
pub fn main(player: Player) -> i64 {
    let next = player.level + 1
    if next > 1 {
        return next
    }
    return 0
}";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
        let position = Position::new(
            1,
            text.lines()
                .nth(1)
                .expect("line should exist")
                .find("level")
                .expect("token should exist"),
        );

        let ranges = databases.selection_ranges(&document, &[position]);

        assert_eq!(ranges.len(), 1);
        let chain = flatten(&ranges[0]);
        assert!(
            chain.iter().any(|range| range.start().line == 1
                && range.start().character == 22
                && range.end().line == 1
                && range.end().character == 27),
            "{chain:?}"
        );
        assert!(
            chain.iter().any(|range| range.start().line == 1
                && range.start().character == 15
                && range.end().line == 1
                && range.end().character == 27),
            "{chain:?}"
        );
        assert!(
            chain.iter().any(|range| range.start().line == 1
                && range.start().character == 15
                && range.end().line == 1
                && range.end().character == 31),
            "{chain:?}"
        );
        assert!(
            chain.iter().any(|range| range.start().line == 1
                && range.start().character == 4
                && range.end().line == 1
                && range.end().character == 31),
            "{chain:?}"
        );
        assert!(
            chain
                .iter()
                .any(|range| range.start().line == 0 && range.end().line == 6),
            "{chain:?}"
        );
    }

    fn flatten(range: &SelectionRange) -> Vec<DiagnosticRange> {
        let mut ranges = Vec::new();
        let mut current = Some(range);
        while let Some(range) = current {
            ranges.push(range.range());
            current = range.parent();
        }
        ranges
    }

    fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        databases
    }
}
