use std::collections::BTreeMap;

use vela_common::{Diagnostic, Span};
use vela_syntax::{
    Argument, Block, ElseBranch, Expr, ExprKind, IfExpr, MapEntry, MatchArm, MatchExpr, Param,
    Pattern, RecordField, RecordPatternField, Stmt, StmtKind,
};

use crate::{HirDeclId, HirExprId, HirLocalId, HirTypeHint};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalBinding {
    pub id: HirLocalId,
    pub name: String,
    pub kind: LocalBindingKind,
    pub type_hint: Option<HirTypeHint>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalBindingKind {
    Parameter,
    Let,
    For,
    LambdaParameter,
    Pattern,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExprInfo {
    pub id: HirExprId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BindingResolution {
    Local(HirLocalId),
    Declaration(HirDeclId),
    Import(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ImportBinding {
    pub name: String,
    pub declaration: Option<HirDeclId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BindingMap {
    pub declaration: HirDeclId,
    locals: BTreeMap<HirLocalId, LocalBinding>,
    locals_by_name: BTreeMap<String, Vec<HirLocalId>>,
    expressions: BTreeMap<HirExprId, ExprInfo>,
    resolutions: BTreeMap<HirExprId, BindingResolution>,
    pattern_resolutions: BTreeMap<Vec<String>, BindingResolution>,
}

impl BindingMap {
    #[must_use]
    pub fn local(&self, local: HirLocalId) -> Option<&LocalBinding> {
        self.locals.get(&local)
    }

    pub fn locals(&self) -> impl Iterator<Item = &LocalBinding> {
        self.locals.values()
    }

    #[must_use]
    pub fn locals_named(&self, name: &str) -> &[HirLocalId] {
        self.locals_by_name
            .get(name)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    #[must_use]
    pub fn local_named_at(
        &self,
        name: &str,
        kind: LocalBindingKind,
        span: Span,
    ) -> Option<HirLocalId> {
        self.locals_named(name).iter().copied().find(|local| {
            self.local(*local)
                .is_some_and(|binding| binding.kind == kind && binding.span == span)
        })
    }

    #[must_use]
    pub fn expression(&self, expression: HirExprId) -> Option<&ExprInfo> {
        self.expressions.get(&expression)
    }

    #[must_use]
    pub fn expression_count(&self) -> usize {
        self.expressions.len()
    }

    #[must_use]
    pub fn resolution(&self, expression: HirExprId) -> Option<&BindingResolution> {
        self.resolutions.get(&expression)
    }

    pub fn resolutions(&self) -> impl Iterator<Item = (HirExprId, &BindingResolution)> {
        self.resolutions
            .iter()
            .map(|(expression, resolution)| (*expression, resolution))
    }

    #[must_use]
    pub fn resolution_at_span(&self, span: Span) -> Option<&BindingResolution> {
        let expression = self
            .expressions
            .iter()
            .find_map(|(id, expression)| (expression.span == span).then_some(*id))?;
        self.resolution(expression)
    }

    #[must_use]
    pub fn pattern_resolution(&self, path: &[String]) -> Option<&BindingResolution> {
        self.pattern_resolutions.get(path)
    }

    pub fn pattern_resolutions(&self) -> impl Iterator<Item = (&[String], &BindingResolution)> {
        self.pattern_resolutions
            .iter()
            .map(|(path, resolution)| (path.as_slice(), resolution))
    }

    pub(crate) fn resolve_import_declarations(&mut self, imports: &BTreeMap<String, HirDeclId>) {
        for resolution in self.resolutions.values_mut() {
            if let BindingResolution::Import(name) = resolution
                && let Some(declaration) = imports.get(name).copied()
            {
                *resolution = BindingResolution::Declaration(declaration);
            }
        }
        for resolution in self.pattern_resolutions.values_mut() {
            if let BindingResolution::Import(name) = resolution
                && let Some(declaration) = imports.get(name).copied()
            {
                *resolution = BindingResolution::Declaration(declaration);
            }
        }
    }
}

pub(crate) struct FunctionBindingInput<'a> {
    pub declaration: HirDeclId,
    pub params: &'a [Param],
    pub body: &'a Block,
    pub module_declarations: Vec<(String, HirDeclId)>,
    pub imports: Vec<ImportBinding>,
    pub next_expr_id: &'a mut u32,
    pub next_local_id: &'a mut u32,
}

pub(crate) fn bind_function(input: FunctionBindingInput<'_>) -> (BindingMap, Vec<Diagnostic>) {
    BindingLowerer::new(input).lower()
}

struct BindingLowerer<'a> {
    declaration: HirDeclId,
    module_declarations: Vec<(String, HirDeclId)>,
    imports: Vec<ImportBinding>,
    next_expr_id: &'a mut u32,
    next_local_id: &'a mut u32,
    scopes: Vec<BTreeMap<String, HirLocalId>>,
    locals: BTreeMap<HirLocalId, LocalBinding>,
    locals_by_name: BTreeMap<String, Vec<HirLocalId>>,
    expressions: BTreeMap<HirExprId, ExprInfo>,
    resolutions: BTreeMap<HirExprId, BindingResolution>,
    pattern_resolutions: BTreeMap<Vec<String>, BindingResolution>,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PathUsage {
    Value,
    Callee,
    FieldBase,
    AssignmentTarget,
}

impl<'a> BindingLowerer<'a> {
    fn new(input: FunctionBindingInput<'a>) -> Self {
        let mut lowerer = Self {
            declaration: input.declaration,
            module_declarations: input.module_declarations,
            imports: input.imports,
            next_expr_id: input.next_expr_id,
            next_local_id: input.next_local_id,
            scopes: vec![BTreeMap::new()],
            locals: BTreeMap::new(),
            locals_by_name: BTreeMap::new(),
            expressions: BTreeMap::new(),
            resolutions: BTreeMap::new(),
            pattern_resolutions: BTreeMap::new(),
            diagnostics: Vec::new(),
        };

        for param in input.params {
            lowerer.declare_local(
                param.name.clone(),
                LocalBindingKind::Parameter,
                param.type_hint.as_ref().map(HirTypeHint::from_syntax),
                input.body.span,
            );
        }
        lowerer.bind_block_without_new_scope(input.body);
        lowerer
    }

    fn lower(self) -> (BindingMap, Vec<Diagnostic>) {
        (
            BindingMap {
                declaration: self.declaration,
                locals: self.locals,
                locals_by_name: self.locals_by_name,
                expressions: self.expressions,
                resolutions: self.resolutions,
                pattern_resolutions: self.pattern_resolutions,
            },
            self.diagnostics,
        )
    }

    fn bind_block(&mut self, block: &Block) {
        self.push_scope();
        self.bind_block_without_new_scope(block);
        self.pop_scope();
    }

    fn bind_block_without_new_scope(&mut self, block: &Block) {
        for statement in &block.statements {
            self.bind_statement(statement);
        }
    }

    fn bind_statement(&mut self, statement: &Stmt) {
        match &statement.kind {
            StmtKind::Let {
                name,
                type_hint,
                value,
            } => {
                if let Some(value) = value {
                    self.bind_expr(value, PathUsage::Value);
                }
                self.declare_local(
                    name.clone(),
                    LocalBindingKind::Let,
                    type_hint.as_ref().map(HirTypeHint::from_syntax),
                    statement.span,
                );
            }
            StmtKind::Return(value) => {
                if let Some(value) = value {
                    self.bind_expr(value, PathUsage::Value);
                }
            }
            StmtKind::Break | StmtKind::Continue => {}
            StmtKind::For {
                binding,
                iterable,
                body,
            } => {
                self.bind_expr(iterable, PathUsage::Value);
                self.push_scope();
                self.declare_local(binding.clone(), LocalBindingKind::For, None, statement.span);
                self.bind_block_without_new_scope(body);
                self.pop_scope();
            }
            StmtKind::Expr(expr) => {
                self.bind_expr(expr, PathUsage::Value);
            }
            StmtKind::Block(block) => {
                self.bind_block(block);
            }
        }
    }

    fn bind_expr(&mut self, expr: &Expr, usage: PathUsage) -> HirExprId {
        let id = self.next_expr(expr.span);
        match &expr.kind {
            ExprKind::Literal(_) | ExprKind::SelfValue | ExprKind::Error => {}
            ExprKind::Path(path) => {
                self.bind_path(id, path, expr.span, usage);
            }
            ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => {
                self.bind_expr(expr, PathUsage::Value);
            }
            ExprKind::Binary { left, right, .. } => {
                self.bind_expr(left, PathUsage::Value);
                self.bind_expr(right, PathUsage::Value);
            }
            ExprKind::Assign { target, value, .. } => {
                self.bind_expr(target, PathUsage::AssignmentTarget);
                self.bind_expr(value, PathUsage::Value);
            }
            ExprKind::Field { base, .. } => {
                self.bind_expr(base, PathUsage::FieldBase);
            }
            ExprKind::Call { callee, args } => {
                self.bind_expr(callee, PathUsage::Callee);
                for arg in args {
                    self.bind_argument(arg);
                }
            }
            ExprKind::Index { base, index } => {
                self.bind_expr(base, PathUsage::Value);
                self.bind_expr(index, PathUsage::Value);
            }
            ExprKind::Array(values) => {
                for value in values {
                    self.bind_expr(value, PathUsage::Value);
                }
            }
            ExprKind::Map(entries) => {
                for entry in entries {
                    self.bind_map_entry(entry);
                }
            }
            ExprKind::Record { path, fields } => {
                self.bind_constructor_path(id, path);
                for field in fields {
                    self.bind_record_field(field);
                }
            }
            ExprKind::Lambda { params, body } => {
                self.push_scope();
                for param in params {
                    self.declare_local(
                        param.name.clone(),
                        LocalBindingKind::LambdaParameter,
                        param.type_hint.as_ref().map(HirTypeHint::from_syntax),
                        expr.span,
                    );
                }
                self.bind_expr(body, PathUsage::Value);
                self.pop_scope();
            }
            ExprKind::If(if_expr) => {
                self.bind_if(if_expr);
            }
            ExprKind::Match(match_expr) => {
                self.bind_match(match_expr);
            }
            ExprKind::Block(block) => {
                self.bind_block(block);
            }
        }
        id
    }

    fn bind_argument(&mut self, argument: &Argument) {
        self.bind_expr(&argument.value, PathUsage::Value);
    }

    fn bind_map_entry(&mut self, entry: &MapEntry) {
        if !matches!(entry.key.kind, ExprKind::Path(_)) {
            self.bind_expr(&entry.key, PathUsage::Value);
        }
        self.bind_expr(&entry.value, PathUsage::Value);
    }

    fn bind_record_field(&mut self, field: &RecordField) {
        if let Some(value) = &field.value {
            self.bind_expr(value, PathUsage::Value);
        } else {
            let id = self.next_expr(field.span);
            if let Some(resolution) = self.resolve_name(&field.name) {
                self.resolutions.insert(id, resolution);
            } else {
                self.diagnostics.push(
                    Diagnostic::error(format!("unresolved name `{}`", field.name))
                        .with_code("hir::unresolved_name")
                        .with_span(field.span)
                        .with_label(field.span, self.name_candidate_label(&field.name)),
                );
            }
        }
    }

    fn bind_if(&mut self, if_expr: &IfExpr) {
        self.bind_expr(&if_expr.condition, PathUsage::Value);
        self.bind_block(&if_expr.then_branch);
        match &if_expr.else_branch {
            Some(ElseBranch::If(if_expr)) => self.bind_if(if_expr),
            Some(ElseBranch::Block(block)) => self.bind_block(block),
            None => {}
        }
    }

    fn bind_match(&mut self, match_expr: &MatchExpr) {
        self.bind_expr(&match_expr.scrutinee, PathUsage::Value);
        for arm in &match_expr.arms {
            self.bind_match_arm(arm);
        }
    }

    fn bind_match_arm(&mut self, arm: &MatchArm) {
        self.push_scope();
        self.bind_pattern(&arm.pattern, arm.body.span);
        if let Some(guard) = &arm.guard {
            self.bind_expr(guard, PathUsage::Value);
        }
        self.bind_expr(&arm.body, PathUsage::Value);
        self.pop_scope();
    }

    fn bind_pattern(&mut self, pattern: &Pattern, span: Span) {
        match pattern {
            Pattern::Binding(name) => {
                self.declare_local(name.clone(), LocalBindingKind::Pattern, None, span);
            }
            Pattern::TupleVariant { path, fields } => {
                self.bind_pattern_path(path);
                for field in fields {
                    self.bind_pattern(field, span);
                }
            }
            Pattern::RecordVariant { path, fields } => {
                self.bind_pattern_path(path);
                for field in fields {
                    self.bind_record_pattern_field(field, span);
                }
            }
            Pattern::Path(path) => {
                self.bind_pattern_path(path);
            }
            Pattern::Wildcard | Pattern::Literal(_) => {}
        }
    }

    fn bind_record_pattern_field(&mut self, field: &RecordPatternField, span: Span) {
        match &field.pattern {
            Some(pattern) => self.bind_pattern(pattern, span),
            None => {
                self.declare_local(field.name.clone(), LocalBindingKind::Pattern, None, span);
            }
        }
    }

    fn bind_path(&mut self, id: HirExprId, path: &[String], span: Span, usage: PathUsage) {
        let [name] = path else {
            if let Some(name) = path.first()
                && let Some(BindingResolution::Local(local)) = self.resolve_name(name)
            {
                self.resolutions.insert(id, BindingResolution::Local(local));
            }
            return;
        };

        if let Some(resolution) = self.resolve_name(name) {
            self.resolutions.insert(id, resolution);
            return;
        }

        if matches!(usage, PathUsage::Value | PathUsage::AssignmentTarget) {
            self.diagnostics.push(
                Diagnostic::error(format!("unresolved name `{name}`"))
                    .with_code("hir::unresolved_name")
                    .with_span(span)
                    .with_label(span, self.name_candidate_label(name)),
            );
        }
    }

    fn bind_constructor_path(&mut self, id: HirExprId, path: &[String]) {
        let Some(name) = path.first() else {
            return;
        };
        if let Some(resolution) = self.resolve_declaration_name(name) {
            self.resolutions.insert(id, resolution);
        }
    }

    fn bind_pattern_path(&mut self, path: &[String]) {
        let Some(name) = path.first() else {
            return;
        };
        if let Some(resolution) = self.resolve_declaration_name(name) {
            self.pattern_resolutions.insert(path.to_vec(), resolution);
        }
    }

    fn resolve_name(&self, name: &str) -> Option<BindingResolution> {
        for scope in self.scopes.iter().rev() {
            if let Some(local) = scope.get(name) {
                return Some(BindingResolution::Local(*local));
            }
        }
        self.resolve_declaration_name(name)
    }

    fn resolve_declaration_name(&self, name: &str) -> Option<BindingResolution> {
        if let Some((_, declaration)) = self
            .module_declarations
            .iter()
            .find(|(declaration_name, _)| declaration_name == name)
        {
            return Some(BindingResolution::Declaration(*declaration));
        }
        self.imports.iter().find_map(|import| {
            if import.name != name {
                return None;
            }
            Some(match import.declaration {
                Some(declaration) => BindingResolution::Declaration(declaration),
                None => BindingResolution::Import(import.name.clone()),
            })
        })
    }

    fn name_candidate_label(&self, name: &str) -> String {
        let mut candidates = self
            .scopes
            .iter()
            .flat_map(|scope| scope.keys().map(String::as_str))
            .chain(
                self.module_declarations
                    .iter()
                    .map(|(name, _)| name.as_str()),
            )
            .chain(self.imports.iter().map(|import| import.name.as_str()))
            .collect::<Vec<_>>();
        candidates.sort_unstable();
        candidates.dedup();

        if let Some(candidate) = closest_name(name, candidates) {
            format!("did you mean `{candidate}`?")
        } else {
            "no similar names found".to_owned()
        }
    }

    fn declare_local(
        &mut self,
        name: String,
        kind: LocalBindingKind,
        type_hint: Option<HirTypeHint>,
        span: Span,
    ) -> HirLocalId {
        let id = self.next_local();
        self.scopes
            .last_mut()
            .expect("function binding always has a scope")
            .insert(name.clone(), id);
        self.locals_by_name
            .entry(name.clone())
            .or_default()
            .push(id);
        self.locals.insert(
            id,
            LocalBinding {
                id,
                name,
                kind,
                type_hint,
                span,
            },
        );
        id
    }

    fn push_scope(&mut self) {
        self.scopes.push(BTreeMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn next_expr(&mut self, span: Span) -> HirExprId {
        let id = HirExprId::new(*self.next_expr_id);
        *self.next_expr_id = self.next_expr_id.saturating_add(1);
        self.expressions.insert(id, ExprInfo { id, span });
        id
    }

    fn next_local(&mut self) -> HirLocalId {
        let id = HirLocalId::new(*self.next_local_id);
        *self.next_local_id = self.next_local_id.saturating_add(1);
        id
    }
}

fn closest_name(
    wanted: &str,
    candidates: impl IntoIterator<Item = impl AsRef<str>>,
) -> Option<String> {
    candidates
        .into_iter()
        .map(|candidate| candidate.as_ref().to_owned())
        .min_by_key(|candidate| candidate_distance(wanted, candidate))
        .filter(|candidate| candidate_distance(wanted, candidate) <= 3)
}

fn candidate_distance(wanted: &str, candidate: &str) -> usize {
    if wanted.contains(candidate) || candidate.contains(wanted) {
        return 0;
    }
    levenshtein(wanted, candidate)
}

fn levenshtein(lhs: &str, rhs: &str) -> usize {
    let mut previous = (0..=rhs.chars().count()).collect::<Vec<_>>();
    let mut current = vec![0; previous.len()];

    for (lhs_index, lhs_char) in lhs.chars().enumerate() {
        current[0] = lhs_index + 1;
        for (rhs_index, rhs_char) in rhs.chars().enumerate() {
            let cost = usize::from(lhs_char != rhs_char);
            current[rhs_index + 1] = (previous[rhs_index + 1] + 1)
                .min(current[rhs_index] + 1)
                .min(previous[rhs_index] + cost);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[rhs.chars().count()]
}
