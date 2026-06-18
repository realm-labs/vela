use std::collections::BTreeMap;

use vela_analysis::type_fact::TypeFact;
use vela_hir::{binding::LocalBindingKind, ids::HirLocalId, module_graph::ModuleGraph};
use vela_syntax::ast::{
    Block, ElseBranch, Expr, ExprKind, IfExpr, InterpolatedStringPart, ItemKind, MapEntry,
    MatchArm, MatchExpr, RecordField, SourceFile, Stmt, StmtKind,
};

pub(super) fn collect(graph: &ModuleGraph, parsed: &SourceFile) -> BTreeMap<HirLocalId, TypeFact> {
    let mut collector = LocalRecordFactCollector {
        graph,
        facts: BTreeMap::new(),
    };
    collector.collect_source_file(parsed);
    collector.facts
}

struct LocalRecordFactCollector<'a> {
    graph: &'a ModuleGraph,
    facts: BTreeMap<HirLocalId, TypeFact>,
}

impl LocalRecordFactCollector<'_> {
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
            StmtKind::Let { name, value, .. } => {
                if let Some(value) = value {
                    self.collect_expr(value);
                    if let ExprKind::Record { path, .. } = &value.kind {
                        self.record_local_fact(statement, name, path);
                    }
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

    fn record_local_fact(&mut self, statement: &Stmt, name: &str, path: &[String]) {
        for declaration in self.graph.declarations() {
            if declaration.span.source != statement.span.source
                || !declaration.span.contains(statement.span.start)
            {
                continue;
            }
            let Some(bindings) = self.graph.bindings(declaration.id) else {
                continue;
            };
            let Some(local) = bindings.local_named_at(name, LocalBindingKind::Let, statement.span)
            else {
                continue;
            };
            self.facts.insert(local, TypeFact::record(path.join("::")));
            break;
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
                self.collect_expr(callee);
                for arg in args {
                    self.collect_expr(&arg.value);
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
            ExprKind::Match(match_expr) => self.collect_match(match_expr),
            ExprKind::Block(block) => self.collect_block(block),
        }
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

    fn collect_match(&mut self, match_expr: &MatchExpr) {
        self.collect_expr(&match_expr.scrutinee);
        for arm in &match_expr.arms {
            self.collect_match_arm(arm);
        }
    }

    fn collect_match_arm(&mut self, arm: &MatchArm) {
        if let Some(guard) = &arm.guard {
            self.collect_expr(guard);
        }
        self.collect_expr(&arm.body);
    }
}
