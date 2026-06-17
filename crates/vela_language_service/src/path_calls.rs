use vela_common::Span;
use vela_syntax::ast::{
    Argument, Block, ElseBranch, Expr, ExprKind, IfExpr, InterpolatedStringPart, ItemKind,
    MapEntry, MatchExpr, Pattern, SourceFile, Stmt, StmtKind,
};

use crate::TextRange;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct PathCallSite {
    pub(crate) path: Vec<String>,
    pub(crate) segment_range: TextRange,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct PathExpressionSite {
    pub(crate) path: Vec<String>,
    pub(crate) segment_range: TextRange,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct PatternPathSite {
    pub(crate) path: Vec<String>,
    pub(crate) segment_range: TextRange,
}

pub(crate) fn path_call_sites(source: &SourceFile, text: &str) -> Vec<PathCallSite> {
    let mut collector = PathCallCollector {
        text,
        call_sites: Vec::new(),
        expression_sites: Vec::new(),
        pattern_sites: Vec::new(),
    };
    collector.collect_source_file(source);
    collector.call_sites
}

pub(crate) fn path_expression_sites(source: &SourceFile, text: &str) -> Vec<PathExpressionSite> {
    let mut collector = PathCallCollector {
        text,
        call_sites: Vec::new(),
        expression_sites: Vec::new(),
        pattern_sites: Vec::new(),
    };
    collector.collect_source_file(source);
    collector.expression_sites
}

pub(crate) fn pattern_path_sites(source: &SourceFile, text: &str) -> Vec<PatternPathSite> {
    let mut collector = PathCallCollector {
        text,
        call_sites: Vec::new(),
        expression_sites: Vec::new(),
        pattern_sites: Vec::new(),
    };
    collector.collect_source_file(source);
    collector.pattern_sites
}

struct PathCallCollector<'a> {
    text: &'a str,
    call_sites: Vec<PathCallSite>,
    expression_sites: Vec<PathExpressionSite>,
    pattern_sites: Vec<PatternPathSite>,
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
                if let Some(index_pattern) = index_pattern {
                    self.collect_pattern(index_pattern, pattern_region);
                }
                self.collect_pattern(pattern, pattern_region);
                self.collect_expr(iterable);
                self.collect_block(body);
            }
            StmtKind::Expr(expr) => self.collect_expr(expr),
            StmtKind::Block(block) => self.collect_block(block),
        }
    }

    fn collect_expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Path(path) => self.record_path_expression(expr.span, path),
            ExprKind::Literal(_) | ExprKind::SelfValue | ExprKind::Error => {}
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
            ExprKind::Record { path, fields } => {
                self.record_path_expression(expr.span, path);
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
                self.collect_match(match_expr);
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

    fn collect_match(&mut self, match_expr: &MatchExpr) {
        let mut arm_start = usize::try_from(match_expr.scrutinee.span.end).unwrap_or_default();
        for arm in &match_expr.arms {
            let arm_end = arm
                .guard
                .as_ref()
                .map_or(arm.body.span.start, |guard| guard.span.start);
            let arm_end = usize::try_from(arm_end).unwrap_or_default();
            self.collect_pattern(&arm.pattern, TextRange::new(arm_start, arm_end));
            if let Some(guard) = &arm.guard {
                self.collect_expr(guard);
            }
            self.collect_expr(&arm.body);
            arm_start = usize::try_from(arm.body.span.end).unwrap_or(arm_start);
        }
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
        self.call_sites.push(PathCallSite {
            path: path.clone(),
            segment_range,
        });
    }

    fn record_path_expression(&mut self, span: Span, path: &[String]) {
        let Some(last_segment) = path.last() else {
            return;
        };
        let Some(segment_range) = last_segment_range(self.text, span, last_segment) else {
            return;
        };
        self.expression_sites.push(PathExpressionSite {
            path: path.to_vec(),
            segment_range,
        });
    }

    fn collect_pattern(&mut self, pattern: &Pattern, search_range: TextRange) {
        match pattern {
            Pattern::Path(path) => self.record_pattern_path(path, search_range),
            Pattern::TupleVariant { path, fields } => {
                self.record_pattern_path(path, search_range);
                for field in fields {
                    self.collect_pattern(field, search_range);
                }
            }
            Pattern::RecordVariant { path, fields } => {
                self.record_pattern_path(path, search_range);
                for field in fields {
                    if let Some(pattern) = &field.pattern {
                        let field_start =
                            usize::try_from(field.span.start).unwrap_or(search_range.start);
                        self.collect_pattern(
                            pattern,
                            TextRange::new(field_start, search_range.end),
                        );
                    }
                }
            }
            Pattern::Binding(_) | Pattern::Wildcard | Pattern::Literal(_) => {}
        }
    }

    fn record_pattern_path(&mut self, path: &[String], search_range: TextRange) {
        let Some(segment_range) = path_last_segment_range(self.text, search_range, path) else {
            return;
        };
        self.pattern_sites.push(PatternPathSite {
            path: path.to_vec(),
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

fn path_last_segment_range(text: &str, range: TextRange, path: &[String]) -> Option<TextRange> {
    let last_segment = path.last()?;
    let joined = path.join("::");
    let slice = text.get(range.start..range.end)?;
    slice.find(&joined).and_then(|offset| {
        let path_start = range.start + offset;
        let path_end = path_start + joined.len();
        is_identifier_boundary(text, path_start, path_end)
            .then(|| TextRange::new(path_end - last_segment.len(), path_end))
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

#[cfg(test)]
mod tests {
    use vela_common::SourceId;
    use vela_syntax::parser::parse_source;

    use super::*;

    #[test]
    fn path_expression_sites_include_paths_and_record_constructors() {
        let source = "\
fn main(state: QuestState) {
    let next = QuestState::Active
    let wrapped = QuestState::Active { count: 1 }
}";
        let parsed = parse_source(SourceId::new(1), source);

        let sites = path_expression_sites(&parsed, source);

        let plain_start =
            source.find("QuestState::Active").expect("path expression") + "QuestState::".len();
        let record_start = source
            .rfind("QuestState::Active")
            .expect("record constructor")
            + "QuestState::".len();
        assert!(sites.contains(&PathExpressionSite {
            path: vec!["QuestState".to_owned(), "Active".to_owned()],
            segment_range: TextRange::new(plain_start, plain_start + "Active".len()),
        }));
        assert!(sites.contains(&PathExpressionSite {
            path: vec!["QuestState".to_owned(), "Active".to_owned()],
            segment_range: TextRange::new(record_start, record_start + "Active".len()),
        }));
    }

    #[test]
    fn pattern_path_sites_include_match_and_for_patterns() {
        let source = "\
fn main(states) {
    for QuestState::Active { count } in states {}
    match state {
        QuestState::Done => 1
        QuestState::Active { count } => count
    }
}";
        let parsed = parse_source(SourceId::new(1), source);

        let sites = pattern_path_sites(&parsed, source);

        let for_start =
            source.find("QuestState::Active").expect("for pattern") + "QuestState::".len();
        let done_start =
            source.find("QuestState::Done").expect("match path pattern") + "QuestState::".len();
        let match_active_start = source
            .rfind("QuestState::Active")
            .expect("match record pattern")
            + "QuestState::".len();
        assert!(sites.contains(&PatternPathSite {
            path: vec!["QuestState".to_owned(), "Active".to_owned()],
            segment_range: TextRange::new(for_start, for_start + "Active".len()),
        }));
        assert!(sites.contains(&PatternPathSite {
            path: vec!["QuestState".to_owned(), "Done".to_owned()],
            segment_range: TextRange::new(done_start, done_start + "Done".len()),
        }));
        assert!(sites.contains(&PatternPathSite {
            path: vec!["QuestState".to_owned(), "Active".to_owned()],
            segment_range: TextRange::new(match_active_start, match_active_start + "Active".len()),
        }));
    }
}
