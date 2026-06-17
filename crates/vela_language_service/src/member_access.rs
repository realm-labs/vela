use std::collections::BTreeMap;

use vela_syntax::ast::{
    Argument, Block, ElseBranch, Expr, ExprKind, IfExpr, InterpolatedStringPart, ItemKind,
    MapEntry, MatchArm, RecordField, SourceFile, Stmt, StmtKind,
};

use crate::TextRange;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct MemberAccessSite {
    pub(crate) member: String,
    pub(crate) member_range: TextRange,
    pub(crate) receiver_range: TextRange,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct MemberCallSite {
    pub(crate) member: String,
    pub(crate) member_range: TextRange,
    pub(crate) receiver_range: TextRange,
}

#[derive(Default)]
struct MemberAccessCollector {
    receiver_ranges: BTreeMap<(usize, usize), TextRange>,
    access_sites: Vec<MemberAccessSite>,
    call_sites: Vec<MemberCallSite>,
}

pub(crate) fn member_receiver_ranges(parsed: &SourceFile) -> BTreeMap<(usize, usize), TextRange> {
    let mut collector = MemberAccessCollector::default();
    collector.collect_source_file(parsed);
    collector.receiver_ranges
}

pub(crate) fn member_call_sites(parsed: &SourceFile) -> Vec<MemberCallSite> {
    let mut collector = MemberAccessCollector::default();
    collector.collect_source_file(parsed);
    collector.call_sites
}

pub(crate) fn member_access_sites(parsed: &SourceFile) -> Vec<MemberAccessSite> {
    let mut collector = MemberAccessCollector::default();
    collector.collect_source_file(parsed);
    collector.access_sites
}

impl MemberAccessCollector {
    fn collect_source_file(&mut self, parsed: &SourceFile) {
        for item in &parsed.items {
            match &item.kind {
                ItemKind::Use(_)
                | ItemKind::Global(_)
                | ItemKind::Struct(_)
                | ItemKind::Enum(_) => {}
                ItemKind::Const(item) => self.collect_expr(&item.value),
                ItemKind::Function(item) => {
                    for param in &item.params {
                        if let Some(default) = &param.default_value {
                            self.collect_expr(default);
                        }
                    }
                    self.collect_block(&item.body);
                }
                ItemKind::Trait(item) => {
                    for method in &item.methods {
                        for param in &method.params {
                            if let Some(default) = &param.default_value {
                                self.collect_expr(default);
                            }
                        }
                        if let Some(body) = &method.default_body {
                            self.collect_block(body);
                        }
                    }
                }
                ItemKind::Impl(item) => {
                    for method in &item.methods {
                        for param in &method.function.params {
                            if let Some(default) = &param.default_value {
                                self.collect_expr(default);
                            }
                        }
                        self.collect_block(&method.function.body);
                    }
                }
            }
        }
    }

    fn collect_block(&mut self, block: &Block) {
        for statement in &block.statements {
            self.collect_statement(statement);
        }
    }

    fn collect_statement(&mut self, statement: &Stmt) {
        match &statement.kind {
            StmtKind::Let { value, .. } => {
                if let Some(value) = value {
                    self.collect_expr(value);
                }
            }
            StmtKind::Return(value) => {
                if let Some(value) = value {
                    self.collect_expr(value);
                }
            }
            StmtKind::Break | StmtKind::Continue => {}
            StmtKind::For { iterable, body, .. } => {
                self.collect_expr(iterable);
                self.collect_block(body);
            }
            StmtKind::Expr(expr) => self.collect_expr(expr),
            StmtKind::Block(block) => self.collect_block(block),
        }
    }

    fn collect_expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Literal(_) | ExprKind::Path(_) | ExprKind::SelfValue | ExprKind::Error => {}
            ExprKind::InterpolatedString(parts) => {
                for part in parts {
                    if let InterpolatedStringPart::Expr(expr) = part {
                        self.collect_expr(expr);
                    }
                }
            }
            ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => self.collect_expr(expr),
            ExprKind::Binary { left, right, .. } => {
                self.collect_expr(left);
                self.collect_expr(right);
            }
            ExprKind::Assign { target, value, .. } => {
                self.collect_expr(target);
                self.collect_expr(value);
            }
            ExprKind::Field { base, name } => {
                self.collect_expr(base);
                self.record_member_access(expr, base, name);
            }
            ExprKind::Call { callee, args } => {
                self.record_call_site(callee);
                self.collect_expr(callee);
                for arg in args {
                    self.collect_argument(arg);
                }
            }
            ExprKind::Index { base, index } => {
                self.collect_expr(base);
                self.collect_expr(index);
            }
            ExprKind::Array(values) => {
                for value in values {
                    self.collect_expr(value);
                }
            }
            ExprKind::Map(entries) => {
                for entry in entries {
                    self.collect_map_entry(entry);
                }
            }
            ExprKind::Record { fields, .. } => {
                for field in fields {
                    self.collect_record_field(field);
                }
            }
            ExprKind::Lambda { params, body } => {
                for param in params {
                    if let Some(default) = &param.default_value {
                        self.collect_expr(default);
                    }
                }
                self.collect_expr(body);
            }
            ExprKind::If(if_expr) => self.collect_if(if_expr),
            ExprKind::Match(match_expr) => {
                self.collect_expr(&match_expr.scrutinee);
                for arm in &match_expr.arms {
                    self.collect_match_arm(arm);
                }
            }
            ExprKind::Block(block) => self.collect_block(block),
        }
    }

    fn collect_argument(&mut self, argument: &Argument) {
        self.collect_expr(&argument.value);
    }

    fn collect_map_entry(&mut self, entry: &MapEntry) {
        self.collect_expr(&entry.key);
        self.collect_expr(&entry.value);
    }

    fn collect_record_field(&mut self, field: &RecordField) {
        if let Some(value) = &field.value {
            self.collect_expr(value);
        }
    }

    fn collect_if(&mut self, if_expr: &IfExpr) {
        self.collect_expr(&if_expr.condition);
        self.collect_block(&if_expr.then_branch);
        if let Some(branch) = &if_expr.else_branch {
            match branch {
                ElseBranch::If(if_expr) => self.collect_if(if_expr),
                ElseBranch::Block(block) => self.collect_block(block),
            }
        }
    }

    fn collect_match_arm(&mut self, arm: &MatchArm) {
        if let Some(guard) = &arm.guard {
            self.collect_expr(guard);
        }
        self.collect_expr(&arm.body);
    }

    fn record_member_access(&mut self, expr: &Expr, base: &Expr, name: &str) {
        let Some(receiver) = span_range(base.span) else {
            return;
        };
        let Some(member) = member_range(expr, name) else {
            return;
        };
        self.receiver_ranges
            .insert((member.start, member.end), receiver);
        self.access_sites.push(MemberAccessSite {
            member: name.to_owned(),
            member_range: member,
            receiver_range: receiver,
        });
    }

    fn record_call_site(&mut self, callee: &Expr) {
        let ExprKind::Field { base, name } = &callee.kind else {
            return;
        };
        let Some(receiver_range) = span_range(base.span) else {
            return;
        };
        let Some(member_range) = member_range(callee, name) else {
            return;
        };
        self.call_sites.push(MemberCallSite {
            member: name.clone(),
            member_range,
            receiver_range,
        });
    }
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

    #[test]
    fn member_call_sites_come_from_call_callee_spans() {
        let source = "\
fn main(player: Player) {
    player.grant(player.level)
}";
        let parsed = parse_source(SourceId::new(1), source);

        let calls = member_call_sites(&parsed);

        let call_receiver_start = source.find("player.grant").expect("method receiver");
        let grant_start = call_receiver_start + "player.".len();
        assert_eq!(
            calls,
            vec![MemberCallSite {
                member: "grant".to_owned(),
                member_range: TextRange::new(grant_start, grant_start + "grant".len()),
                receiver_range: TextRange::new(
                    call_receiver_start,
                    call_receiver_start + "player".len()
                ),
            }]
        );
    }

    #[test]
    fn member_access_sites_include_field_and_method_members() {
        let source = "\
fn main(player: Player) {
    player.grant(player.level)
}";
        let parsed = parse_source(SourceId::new(1), source);

        let sites = member_access_sites(&parsed);

        let grant_receiver_start = source.find("player.grant").expect("method receiver");
        let grant_start = grant_receiver_start + "player.".len();
        let level_receiver_start = source.find("player.level").expect("field receiver");
        let level_start = level_receiver_start + "player.".len();
        assert_eq!(
            sites,
            vec![
                MemberAccessSite {
                    member: "grant".to_owned(),
                    member_range: TextRange::new(grant_start, grant_start + "grant".len()),
                    receiver_range: TextRange::new(
                        grant_receiver_start,
                        grant_receiver_start + "player".len()
                    ),
                },
                MemberAccessSite {
                    member: "level".to_owned(),
                    member_range: TextRange::new(level_start, level_start + "level".len()),
                    receiver_range: TextRange::new(
                        level_receiver_start,
                        level_receiver_start + "player".len()
                    ),
                },
            ]
        );
    }
}
