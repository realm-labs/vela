use vela_common::Span;
use vela_syntax::ast::{
    Argument, Block, ElseBranch, Expr, ExprKind, IfExpr, InterpolatedStringPart, ItemKind,
    MapEntry, MatchArm, SourceFile, Stmt, StmtKind,
};

use crate::TextRange;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct PathCallSite {
    pub(crate) path: Vec<String>,
    pub(crate) segment_range: TextRange,
}

pub(crate) fn path_call_sites(source: &SourceFile, text: &str) -> Vec<PathCallSite> {
    let mut collector = PathCallCollector {
        text,
        sites: Vec::new(),
    };
    collector.collect_source_file(source);
    collector.sites
}

struct PathCallCollector<'a> {
    text: &'a str,
    sites: Vec<PathCallSite>,
}

impl PathCallCollector<'_> {
    fn collect_source_file(&mut self, source: &SourceFile) {
        for item in &source.items {
            match &item.kind {
                ItemKind::Use(_) | ItemKind::Global(_) => {}
                ItemKind::Const(item) => self.collect_expr(&item.value),
                ItemKind::Struct(item) => {
                    for field in &item.fields {
                        if let Some(default) = &field.default_value {
                            self.collect_expr(default);
                        }
                    }
                }
                ItemKind::Enum(item) => {
                    for variant in &item.variants {
                        match &variant.fields {
                            vela_syntax::ast::EnumVariantFields::Unit => {}
                            vela_syntax::ast::EnumVariantFields::Tuple(params) => {
                                for param in params {
                                    if let Some(default) = &param.default_value {
                                        self.collect_expr(default);
                                    }
                                }
                            }
                            vela_syntax::ast::EnumVariantFields::Record(fields) => {
                                for field in fields {
                                    if let Some(default) = &field.default_value {
                                        self.collect_expr(default);
                                    }
                                }
                            }
                        }
                    }
                }
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
            StmtKind::Let { value, .. } | StmtKind::Return(value) => {
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
            ExprKind::Field { base, .. } => self.collect_expr(base),
            ExprKind::Call { callee, args } => {
                self.record_path_call(callee);
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
                    if let Some(value) = &field.value {
                        self.collect_expr(value);
                    }
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

    fn record_path_call(&mut self, callee: &Expr) {
        let ExprKind::Path(path) = &callee.kind else {
            return;
        };
        let Some(last_segment) = path.last() else {
            return;
        };
        let Some(segment_range) = last_segment_range(self.text, callee.span, last_segment) else {
            return;
        };
        self.sites.push(PathCallSite {
            path: path.clone(),
            segment_range,
        });
    }
}

fn last_segment_range(text: &str, span: Span, segment: &str) -> Option<TextRange> {
    let range = span_text_range(span)?;
    let slice = text.get(range.start..range.end)?;
    slice.rmatch_indices(segment).find_map(|(offset, matched)| {
        let start = range.start + offset;
        let end = start + matched.len();
        is_identifier_boundary(text, start, end).then(|| TextRange::new(start, end))
    })
}

fn span_text_range(span: Span) -> Option<TextRange> {
    let start = usize::try_from(span.start).ok()?;
    let end = usize::try_from(span.end).ok()?;
    Some(TextRange::new(start, end))
}

fn is_identifier_boundary(text: &str, start: usize, end: usize) -> bool {
    let before = text[..start].chars().next_back();
    let after = text[end..].chars().next();
    before.is_none_or(|ch| !is_identifier_continue(ch))
        && after.is_none_or(|ch| !is_identifier_continue(ch))
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}
