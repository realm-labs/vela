use vela_common::Span;
use vela_syntax::ast::{
    Block, ElseBranch, Expr, ExprKind, ItemKind, Pattern, SourceFile, Stmt, StmtKind,
};

use crate::TextRange;

pub(super) fn is_pattern_context(text: &str, source: &SourceFile, offset: usize) -> bool {
    source.items.iter().any(|item| match &item.kind {
        ItemKind::Function(item) => {
            item.params
                .iter()
                .filter_map(|param| param.default_value.as_ref())
                .any(|value| pattern_for_expr(text, value, offset))
                || pattern_for_block(text, &item.body, offset)
        }
        ItemKind::Const(item) => pattern_for_expr(text, &item.value, offset),
        _ => false,
    })
}

fn pattern_for_block(text: &str, block: &Block, offset: usize) -> bool {
    block_range(block).is_some_and(|range| {
        range_contains_offset(range, offset)
            && block
                .statements
                .iter()
                .any(|statement| pattern_for_statement(text, statement, offset))
    })
}

fn pattern_for_statement(text: &str, statement: &Stmt, offset: usize) -> bool {
    if !span_contains_usize(statement.span, offset) {
        return false;
    }
    match &statement.kind {
        StmtKind::For {
            index_pattern,
            pattern,
            iterable,
            body,
        } => {
            let pattern_region = TextRange::new(
                usize::try_from(statement.span.start).unwrap_or_default(),
                usize::try_from(iterable.span.start).unwrap_or_default(),
            );
            index_pattern.as_ref().is_some_and(|pattern| {
                pattern_contains_offset(text, pattern, pattern_region, offset)
            }) || pattern_contains_offset(text, pattern, pattern_region, offset)
                || pattern_for_expr(text, iterable, offset)
                || pattern_for_block(text, body, offset)
        }
        StmtKind::Let { value, .. } => value
            .as_ref()
            .is_some_and(|value| pattern_for_expr(text, value, offset)),
        StmtKind::Expr(value) | StmtKind::Return(Some(value)) => {
            pattern_for_expr(text, value, offset)
        }
        StmtKind::Block(block) => pattern_for_block(text, block, offset),
        StmtKind::Return(None) | StmtKind::Break | StmtKind::Continue => false,
    }
}

fn pattern_for_expr(text: &str, expr: &Expr, offset: usize) -> bool {
    if !span_contains_usize(expr.span, offset) {
        return false;
    }
    match &expr.kind {
        ExprKind::Match(match_expr) => {
            if pattern_for_expr(text, &match_expr.scrutinee, offset) {
                return true;
            }
            let mut arm_start = usize::try_from(match_expr.scrutinee.span.end).unwrap_or_default();
            for arm in &match_expr.arms {
                let arm_end = usize::try_from(arm.body.span.start).unwrap_or_default();
                let arm_region = TextRange::new(arm_start, arm_end);
                if pattern_contains_offset(text, &arm.pattern, arm_region, offset)
                    || arm
                        .guard
                        .as_ref()
                        .is_some_and(|guard| pattern_for_expr(text, guard, offset))
                    || pattern_for_expr(text, &arm.body, offset)
                {
                    return true;
                }
                arm_start = usize::try_from(arm.body.span.end).unwrap_or(arm_start);
            }
            false
        }
        ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => pattern_for_expr(text, expr, offset),
        ExprKind::Binary { left, right, .. }
        | ExprKind::Assign {
            target: left,
            value: right,
            ..
        } => pattern_for_expr(text, left, offset) || pattern_for_expr(text, right, offset),
        ExprKind::Field { base, .. } => pattern_for_expr(text, base, offset),
        ExprKind::Call { callee, args } => {
            pattern_for_expr(text, callee, offset)
                || args
                    .iter()
                    .any(|arg| pattern_for_expr(text, &arg.value, offset))
        }
        ExprKind::Index { base, index } => {
            pattern_for_expr(text, base, offset) || pattern_for_expr(text, index, offset)
        }
        ExprKind::Array(values) => values
            .iter()
            .any(|value| pattern_for_expr(text, value, offset)),
        ExprKind::Map(entries) => entries.iter().any(|entry| {
            pattern_for_expr(text, &entry.key, offset)
                || pattern_for_expr(text, &entry.value, offset)
        }),
        ExprKind::Record { fields, .. } => fields
            .iter()
            .filter_map(|field| field.value.as_ref())
            .any(|value| pattern_for_expr(text, value, offset)),
        ExprKind::Lambda { params, body } => {
            params
                .iter()
                .filter_map(|param| param.default_value.as_ref())
                .any(|value| pattern_for_expr(text, value, offset))
                || pattern_for_expr(text, body, offset)
        }
        ExprKind::If(if_expr) => {
            pattern_for_expr(text, &if_expr.condition, offset)
                || pattern_for_block(text, &if_expr.then_branch, offset)
                || if_expr
                    .else_branch
                    .as_ref()
                    .is_some_and(|branch| pattern_for_else_branch(text, branch, offset))
        }
        ExprKind::Block(block) => pattern_for_block(text, block, offset),
        ExprKind::Literal(_)
        | ExprKind::InterpolatedString(_)
        | ExprKind::Path(_)
        | ExprKind::SelfValue
        | ExprKind::Error => false,
    }
}

fn pattern_for_else_branch(text: &str, branch: &ElseBranch, offset: usize) -> bool {
    match branch {
        ElseBranch::Block(block) => pattern_for_block(text, block, offset),
        ElseBranch::If(if_expr) => {
            pattern_for_expr(text, &if_expr.condition, offset)
                || pattern_for_block(text, &if_expr.then_branch, offset)
                || if_expr
                    .else_branch
                    .as_ref()
                    .is_some_and(|branch| pattern_for_else_branch(text, branch, offset))
        }
    }
}

fn pattern_contains_offset(
    text: &str,
    pattern: &Pattern,
    search_range: TextRange,
    offset: usize,
) -> bool {
    match pattern {
        Pattern::Binding(name) => ident_occurrence_contains(text, search_range, name, offset),
        Pattern::Path(path) => path_occurrence_contains(text, search_range, path, offset),
        Pattern::TupleVariant { path, fields } => {
            path_occurrence_contains(text, search_range, path, offset)
                || fields
                    .iter()
                    .any(|field| pattern_contains_offset(text, field, search_range, offset))
        }
        Pattern::RecordVariant { path, fields } => {
            path_occurrence_contains(text, search_range, path, offset)
                || fields.iter().any(|field| {
                    span_contains_usize(field.span, offset)
                        || field.pattern.as_ref().is_some_and(|pattern| {
                            let field_start =
                                usize::try_from(field.span.start).unwrap_or(search_range.start);
                            pattern_contains_offset(
                                text,
                                pattern,
                                TextRange::new(field_start, search_range.end),
                                offset,
                            )
                        })
                })
        }
        Pattern::Wildcard | Pattern::Literal(_) => false,
    }
}

fn path_occurrence_contains(
    text: &str,
    search_range: TextRange,
    path: &[String],
    offset: usize,
) -> bool {
    if path.is_empty() {
        return false;
    }
    let joined = path.join("::");
    if ident_occurrence_contains(text, search_range, &joined, offset) {
        return true;
    }
    path.iter()
        .any(|segment| ident_occurrence_contains(text, search_range, segment, offset))
}

fn ident_occurrence_contains(
    text: &str,
    search_range: TextRange,
    ident: &str,
    offset: usize,
) -> bool {
    if ident.is_empty() || !range_contains_offset(search_range, offset) {
        return false;
    }
    let Some(haystack) = text.get(search_range.start..search_range.end) else {
        return false;
    };
    let mut cursor = 0;
    while let Some(relative) = haystack[cursor..].find(ident) {
        let start = search_range.start + cursor + relative;
        let end = start + ident.len();
        if identifier_boundary(text, start, end) && start <= offset && offset <= end {
            return true;
        }
        cursor += relative + ident.len();
    }
    false
}

fn identifier_boundary(text: &str, start: usize, end: usize) -> bool {
    let before = text[..start].chars().next_back();
    let after = text[end..].chars().next();
    before.is_none_or(|ch| !is_identifier_continue(ch))
        && after.is_none_or(|ch| !is_identifier_continue(ch))
}

fn range_contains_offset(range: TextRange, offset: usize) -> bool {
    range.start <= offset && offset <= range.end
}

fn block_range(block: &Block) -> Option<TextRange> {
    Some(TextRange::new(
        usize::try_from(block.span.start).ok()?,
        usize::try_from(block.span.end).ok()?,
    ))
}

fn span_contains_usize(span: Span, offset: usize) -> bool {
    let Some(offset) = u32::try_from(offset).ok() else {
        return false;
    };
    span.start <= offset && offset <= span.end
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}
