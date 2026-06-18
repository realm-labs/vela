use std::collections::BTreeMap;

use vela_analysis::{
    expression::{ExprFactScope, type_fact_from_expr_with_registry},
    registry::RegistryFacts,
    type_fact::TypeFact,
};
use vela_common::SourceId;
use vela_hir::{module_graph::ModuleGraph, type_hint::HirTypeHint};
use vela_syntax::ast::{
    Argument, Block, ElseBranch, Expr, ExprKind, FunctionItem, IfExpr, ItemKind, MapEntry,
    MatchArm, Param, RecordField, SourceFile, Stmt, StmtKind, TypeHint,
};

use crate::callable_context::query_type_fact_from_hint;
use crate::{LanguageServiceDatabases, TextRange};

pub(crate) fn collect(
    graph: &ModuleGraph,
    parsed: &SourceFile,
    schema: &RegistryFacts,
) -> BTreeMap<(usize, usize), TypeFact> {
    let mut collector = ExpressionFactCollector {
        graph,
        schema,
        facts: BTreeMap::new(),
    };
    collector.collect_source_file(parsed);
    collector.facts
}

pub(crate) fn fact_for_range(
    databases: &LanguageServiceDatabases,
    source_id: SourceId,
    range: TextRange,
) -> Option<TypeFact> {
    let document_id =
        databases
            .source_db()
            .records()
            .iter()
            .find_map(|(document_id, source)| {
                (source.source_id() == source_id).then_some(document_id)
            })?;
    let parsed = databases.parse_db().parsed_source(document_id)?;
    collect(
        databases.hir_db().graph(),
        parsed,
        databases.schema_db().facts(),
    )
    .get(&text_range_key(range))
    .cloned()
}

struct ExpressionFactCollector<'a> {
    graph: &'a ModuleGraph,
    schema: &'a RegistryFacts,
    facts: BTreeMap<(usize, usize), TypeFact>,
}

impl ExpressionFactCollector<'_> {
    fn collect_source_file(&mut self, parsed: &SourceFile) {
        for item in &parsed.items {
            match &item.kind {
                ItemKind::Const(item) => {
                    let mut scope = ExprFactScope::new();
                    self.collect_expr(&item.value, &mut scope);
                }
                ItemKind::Function(item) => self.collect_function(item),
                ItemKind::Trait(item) => {
                    for method in &item.methods {
                        if let Some(body) = &method.default_body {
                            let mut scope = self.param_scope(&method.params);
                            self.collect_block(body, &mut scope);
                        }
                    }
                }
                ItemKind::Impl(item) => {
                    for method in &item.methods {
                        self.collect_function(&method.function);
                    }
                }
                ItemKind::Use(_)
                | ItemKind::Global(_)
                | ItemKind::Struct(_)
                | ItemKind::Enum(_) => {}
            }
        }
    }

    fn collect_function(&mut self, item: &FunctionItem) {
        let mut scope = self.param_scope(&item.params);
        self.collect_block(&item.body, &mut scope);
    }

    fn param_scope(&self, params: &[Param]) -> ExprFactScope {
        let mut scope = ExprFactScope::new();
        for param in params {
            if let Some(type_hint) = &param.type_hint {
                scope.insert_path([param.name.clone()], self.type_fact_from_hint(type_hint));
            }
            if let Some(default) = &param.default_value {
                let fact = type_fact_from_expr_with_registry(default, &scope, self.schema);
                if !matches!(fact, TypeFact::Unknown) {
                    scope.insert_path([param.name.clone()], fact);
                }
            }
        }
        scope
    }

    fn collect_block(&mut self, block: &Block, scope: &mut ExprFactScope) {
        for statement in &block.statements {
            self.collect_stmt(statement, scope);
        }
    }

    fn collect_stmt(&mut self, statement: &Stmt, scope: &mut ExprFactScope) {
        match &statement.kind {
            StmtKind::Let {
                name,
                type_hint,
                value,
            } => {
                if let Some(value) = value {
                    self.collect_expr(value, scope);
                    let fact = type_fact_from_expr_with_registry(value, scope, self.schema);
                    if !matches!(fact, TypeFact::Unknown) {
                        scope.insert_path([name.clone()], fact);
                    }
                } else if let Some(type_hint) = type_hint {
                    scope.insert_path([name.clone()], self.type_fact_from_hint(type_hint));
                }
            }
            StmtKind::Return(value) => {
                if let Some(expr) = value {
                    self.collect_expr(expr, scope);
                }
            }
            StmtKind::For { iterable, body, .. } => {
                self.collect_expr(iterable, scope);
                let mut body_scope = scope.clone();
                self.collect_block(body, &mut body_scope);
            }
            StmtKind::Expr(expr) => self.collect_expr(expr, scope),
            StmtKind::Block(block) => {
                let mut block_scope = scope.clone();
                self.collect_block(block, &mut block_scope);
            }
            StmtKind::Break | StmtKind::Continue => {}
        }
    }

    fn collect_expr(&mut self, expr: &Expr, scope: &mut ExprFactScope) {
        match &expr.kind {
            ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => {
                self.collect_expr(expr, scope);
            }
            ExprKind::Binary { left, right, .. }
            | ExprKind::Assign {
                target: left,
                value: right,
                ..
            } => {
                self.collect_expr(left, scope);
                self.collect_expr(right, scope);
            }
            ExprKind::Field { base, .. } => self.collect_expr(base, scope),
            ExprKind::Call { callee, args } => {
                self.collect_expr(callee, scope);
                for arg in args {
                    self.collect_argument(arg, scope);
                }
            }
            ExprKind::Index { base, index } => {
                self.collect_expr(base, scope);
                self.collect_expr(index, scope);
            }
            ExprKind::Array(items) => {
                for item in items {
                    self.collect_expr(item, scope);
                }
            }
            ExprKind::Map(entries) => {
                for entry in entries {
                    self.collect_map_entry(entry, scope);
                }
            }
            ExprKind::Record { fields, .. } => {
                for field in fields {
                    self.collect_record_field(field, scope);
                }
            }
            ExprKind::Lambda { params, body } => {
                let mut lambda_scope = scope.clone();
                for param in params {
                    if let Some(type_hint) = &param.type_hint {
                        lambda_scope
                            .insert_path([param.name.clone()], self.type_fact_from_hint(type_hint));
                    }
                    if let Some(default) = &param.default_value {
                        self.collect_expr(default, &mut lambda_scope);
                    }
                }
                self.collect_expr(body, &mut lambda_scope);
            }
            ExprKind::If(if_expr) => self.collect_if(if_expr, scope),
            ExprKind::Match(match_expr) => {
                self.collect_expr(&match_expr.scrutinee, scope);
                for arm in &match_expr.arms {
                    self.collect_match_arm(arm, scope);
                }
            }
            ExprKind::Block(block) => {
                let mut block_scope = scope.clone();
                self.collect_block(block, &mut block_scope);
            }
            ExprKind::InterpolatedString(parts) => {
                for part in parts {
                    if let vela_syntax::ast::InterpolatedStringPart::Expr(expr) = part {
                        self.collect_expr(expr, scope);
                    }
                }
            }
            ExprKind::Literal(_) | ExprKind::Path(_) | ExprKind::SelfValue | ExprKind::Error => {}
        }

        let fact = type_fact_from_expr_with_registry(expr, scope, self.schema);
        if !matches!(fact, TypeFact::Unknown) {
            self.facts.insert(span_key(expr.span), fact);
        }
    }

    fn collect_argument(&mut self, argument: &Argument, scope: &mut ExprFactScope) {
        self.collect_expr(&argument.value, scope);
    }

    fn collect_map_entry(&mut self, entry: &MapEntry, scope: &mut ExprFactScope) {
        self.collect_expr(&entry.key, scope);
        self.collect_expr(&entry.value, scope);
    }

    fn collect_record_field(&mut self, field: &RecordField, scope: &mut ExprFactScope) {
        if let Some(value) = &field.value {
            self.collect_expr(value, scope);
        }
    }

    fn collect_if(&mut self, if_expr: &IfExpr, scope: &mut ExprFactScope) {
        self.collect_expr(&if_expr.condition, scope);
        let mut then_scope = scope.clone();
        self.collect_block(&if_expr.then_branch, &mut then_scope);
        if let Some(branch) = &if_expr.else_branch {
            match branch {
                ElseBranch::If(if_expr) => {
                    let mut else_scope = scope.clone();
                    self.collect_if(if_expr, &mut else_scope);
                }
                ElseBranch::Block(block) => {
                    let mut else_scope = scope.clone();
                    self.collect_block(block, &mut else_scope);
                }
            }
        }
    }

    fn collect_match_arm(&mut self, arm: &MatchArm, scope: &ExprFactScope) {
        let mut arm_scope = scope.clone();
        if let Some(guard) = &arm.guard {
            self.collect_expr(guard, &mut arm_scope);
        }
        self.collect_expr(&arm.body, &mut arm_scope);
    }

    fn type_fact_from_hint(&self, hint: &TypeHint) -> TypeFact {
        query_type_fact_from_hint(self.graph, &HirTypeHint::from_syntax(hint), self.schema)
    }
}

fn text_range_key(range: TextRange) -> (usize, usize) {
    (range.start, range.end)
}

fn span_key(span: vela_common::Span) -> (usize, usize) {
    (span.start as usize, span.end as usize)
}
