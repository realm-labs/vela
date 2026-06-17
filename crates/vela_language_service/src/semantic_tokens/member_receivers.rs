use std::collections::BTreeMap;

use vela_syntax::ast::{
    Argument, Block, ElseBranch, Expr, ExprKind, IfExpr, InterpolatedStringPart, ItemKind,
    MapEntry, MatchArm, RecordField, SourceFile, Stmt, StmtKind,
};

use crate::TextRange;

pub(super) fn member_receiver_ranges(parsed: &SourceFile) -> BTreeMap<(usize, usize), TextRange> {
    let mut ranges = BTreeMap::new();
    for item in &parsed.items {
        match &item.kind {
            ItemKind::Use(_) | ItemKind::Global(_) | ItemKind::Struct(_) | ItemKind::Enum(_) => {}
            ItemKind::Const(item) => collect_expr(&item.value, &mut ranges),
            ItemKind::Function(item) => {
                for param in &item.params {
                    if let Some(default) = &param.default_value {
                        collect_expr(default, &mut ranges);
                    }
                }
                collect_block(&item.body, &mut ranges);
            }
            ItemKind::Trait(item) => {
                for method in &item.methods {
                    for param in &method.params {
                        if let Some(default) = &param.default_value {
                            collect_expr(default, &mut ranges);
                        }
                    }
                    if let Some(body) = &method.default_body {
                        collect_block(body, &mut ranges);
                    }
                }
            }
            ItemKind::Impl(item) => {
                for method in &item.methods {
                    for param in &method.function.params {
                        if let Some(default) = &param.default_value {
                            collect_expr(default, &mut ranges);
                        }
                    }
                    collect_block(&method.function.body, &mut ranges);
                }
            }
        }
    }
    ranges
}

fn collect_block(block: &Block, ranges: &mut BTreeMap<(usize, usize), TextRange>) {
    for statement in &block.statements {
        collect_statement(statement, ranges);
    }
}

fn collect_statement(statement: &Stmt, ranges: &mut BTreeMap<(usize, usize), TextRange>) {
    match &statement.kind {
        StmtKind::Let { value, .. } => {
            if let Some(value) = value {
                collect_expr(value, ranges);
            }
        }
        StmtKind::Return(value) => {
            if let Some(value) = value {
                collect_expr(value, ranges);
            }
        }
        StmtKind::Break | StmtKind::Continue => {}
        StmtKind::For { iterable, body, .. } => {
            collect_expr(iterable, ranges);
            collect_block(body, ranges);
        }
        StmtKind::Expr(expr) => collect_expr(expr, ranges),
        StmtKind::Block(block) => collect_block(block, ranges),
    }
}

fn collect_expr(expr: &Expr, ranges: &mut BTreeMap<(usize, usize), TextRange>) {
    match &expr.kind {
        ExprKind::Literal(_) | ExprKind::Path(_) | ExprKind::SelfValue | ExprKind::Error => {}
        ExprKind::InterpolatedString(parts) => {
            for part in parts {
                if let InterpolatedStringPart::Expr(expr) = part {
                    collect_expr(expr, ranges);
                }
            }
        }
        ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => collect_expr(expr, ranges),
        ExprKind::Binary { left, right, .. } => {
            collect_expr(left, ranges);
            collect_expr(right, ranges);
        }
        ExprKind::Assign { target, value, .. } => {
            collect_expr(target, ranges);
            collect_expr(value, ranges);
        }
        ExprKind::Field { base, name } => {
            collect_expr(base, ranges);
            if let (Some(receiver), Some(member)) =
                (span_range(base.span), member_range(expr, name))
            {
                ranges.insert((member.start, member.end), receiver);
            }
        }
        ExprKind::Call { callee, args } => {
            collect_expr(callee, ranges);
            for arg in args {
                collect_argument(arg, ranges);
            }
        }
        ExprKind::Index { base, index } => {
            collect_expr(base, ranges);
            collect_expr(index, ranges);
        }
        ExprKind::Array(values) => {
            for value in values {
                collect_expr(value, ranges);
            }
        }
        ExprKind::Map(entries) => {
            for entry in entries {
                collect_map_entry(entry, ranges);
            }
        }
        ExprKind::Record { fields, .. } => {
            for field in fields {
                collect_record_field(field, ranges);
            }
        }
        ExprKind::Lambda { params, body } => {
            for param in params {
                if let Some(default) = &param.default_value {
                    collect_expr(default, ranges);
                }
            }
            collect_expr(body, ranges);
        }
        ExprKind::If(if_expr) => collect_if(if_expr, ranges),
        ExprKind::Match(match_expr) => {
            collect_expr(&match_expr.scrutinee, ranges);
            for arm in &match_expr.arms {
                collect_match_arm(arm, ranges);
            }
        }
        ExprKind::Block(block) => collect_block(block, ranges),
    }
}

fn collect_argument(argument: &Argument, ranges: &mut BTreeMap<(usize, usize), TextRange>) {
    collect_expr(&argument.value, ranges);
}

fn collect_map_entry(entry: &MapEntry, ranges: &mut BTreeMap<(usize, usize), TextRange>) {
    collect_expr(&entry.key, ranges);
    collect_expr(&entry.value, ranges);
}

fn collect_record_field(field: &RecordField, ranges: &mut BTreeMap<(usize, usize), TextRange>) {
    if let Some(value) = &field.value {
        collect_expr(value, ranges);
    }
}

fn collect_if(if_expr: &IfExpr, ranges: &mut BTreeMap<(usize, usize), TextRange>) {
    collect_expr(&if_expr.condition, ranges);
    collect_block(&if_expr.then_branch, ranges);
    if let Some(branch) = &if_expr.else_branch {
        match branch {
            ElseBranch::If(if_expr) => collect_if(if_expr, ranges),
            ElseBranch::Block(block) => collect_block(block, ranges),
        }
    }
}

fn collect_match_arm(arm: &MatchArm, ranges: &mut BTreeMap<(usize, usize), TextRange>) {
    if let Some(guard) = &arm.guard {
        collect_expr(guard, ranges);
    }
    collect_expr(&arm.body, ranges);
}

fn member_range(expr: &Expr, name: &str) -> Option<TextRange> {
    let span = span_range(expr.span)?;
    span.end
        .checked_sub(name.len())
        .map(|start| TextRange::new(start, span.end))
}

fn span_range(span: vela_common::Span) -> Option<TextRange> {
    let start = usize::try_from(span.start).ok()?;
    let end = usize::try_from(span.end).ok()?;
    Some(TextRange::new(start, end))
}

#[cfg(test)]
mod tests {
    use vela_common::SourceId;
    use vela_syntax::parser::parse_source;

    use super::*;

    #[test]
    fn member_receiver_ranges_come_from_field_expression_spans() {
        let source = "\
fn main(player: Player) {
    let level = player.level
    player.grant(level)
}";
        let parsed = parse_source(SourceId::new(1), source);

        let ranges = member_receiver_ranges(&parsed);

        let player_start = source.find("player.level").expect("field receiver");
        let call_receiver_start = source.find("player.grant").expect("method receiver");
        let level_start = player_start + "player.".len();
        let grant_start = call_receiver_start + "player.".len();
        assert_eq!(
            ranges.get(&(level_start, level_start + "level".len())),
            Some(&TextRange::new(player_start, player_start + "player".len()))
        );
        assert_eq!(
            ranges.get(&(grant_start, grant_start + "grant".len())),
            Some(&TextRange::new(
                call_receiver_start,
                call_receiver_start + "player".len()
            ))
        );
    }
}
