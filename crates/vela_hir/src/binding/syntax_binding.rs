use std::collections::BTreeMap;

use vela_common::{Diagnostic, SourceId, Span};
use vela_syntax::TextRange;
use vela_syntax::ast::{
    AstNode, SyntaxArgument, SyntaxBlock, SyntaxElseBranch, SyntaxExpression, SyntaxExpressionKind,
    SyntaxMapEntry, SyntaxParam, SyntaxPattern, SyntaxPatternKind, SyntaxRecordExprField,
    SyntaxRecordPatternField, SyntaxStatement, SyntaxStatementKind, SyntaxTypeHint,
};

use crate::binding::name_candidates::{NameCandidate, closest_name_candidate};
use crate::binding::{
    BindingMap, BindingResolution, ExprInfo, ImportBinding, LocalBinding, LocalBindingKind,
    PathUsage,
};
use crate::ids::{HirDeclId, HirExprId, HirLocalId};
use crate::type_hint::{HirTypeHint, ParamHint};

pub(crate) struct SyntaxFunctionBindingInput<'a> {
    pub source: SourceId,
    pub declaration: HirDeclId,
    pub params: &'a [ParamHint],
    pub default_params: Vec<SyntaxParam>,
    pub body: SyntaxBlock,
    pub module_declarations: Vec<(String, HirDeclId)>,
    pub qualified_declarations: Vec<(Vec<String>, HirDeclId)>,
    pub imports: Vec<ImportBinding>,
    pub next_expr_id: &'a mut u32,
    pub next_local_id: &'a mut u32,
}

pub(crate) fn bind_syntax_function(
    input: SyntaxFunctionBindingInput<'_>,
) -> (BindingMap, Vec<Diagnostic>) {
    SyntaxBindingLowerer::new(input).lower()
}

struct SyntaxBindingLowerer<'a> {
    source: SourceId,
    declaration: HirDeclId,
    module_declarations: Vec<(String, HirDeclId)>,
    qualified_declarations: Vec<(Vec<String>, HirDeclId)>,
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

impl<'a> SyntaxBindingLowerer<'a> {
    fn new(input: SyntaxFunctionBindingInput<'a>) -> Self {
        let mut lowerer = Self {
            source: input.source,
            declaration: input.declaration,
            module_declarations: input.module_declarations,
            qualified_declarations: input.qualified_declarations,
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
            lowerer.declare_parameter(
                param.name.clone(),
                LocalBindingKind::Parameter,
                param.type_hint.clone(),
                param.span,
            );
        }
        for param in input.default_params {
            if let Some(default_value) = param.default_value() {
                lowerer.bind_expr(&default_value, PathUsage::Value);
            }
        }
        lowerer.bind_block_without_new_scope(&input.body);
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

    fn bind_block(&mut self, block: &SyntaxBlock) {
        self.push_scope();
        self.bind_block_without_new_scope(block);
        self.pop_scope();
    }

    fn bind_block_without_new_scope(&mut self, block: &SyntaxBlock) {
        for statement in block.statements() {
            self.bind_statement(&statement);
        }
    }

    fn bind_statement(&mut self, statement: &SyntaxStatement) {
        match statement.statement_kind() {
            SyntaxStatementKind::Let => {
                let Some(statement) = statement.as_let() else {
                    return;
                };
                if let Some(value) = statement.initializer() {
                    self.bind_expr(&value, PathUsage::Value);
                }
                if let Some(name) = statement.name_text() {
                    self.declare_local(
                        name,
                        LocalBindingKind::Let,
                        statement
                            .type_hint()
                            .as_ref()
                            .map(|hint| hir_type_hint(self.source, hint)),
                        span_for(self.source, statement.syntax().text_range()),
                    );
                }
            }
            SyntaxStatementKind::Return => {
                if let Some(statement) = statement.as_return()
                    && let Some(value) = statement.expression()
                {
                    self.bind_expr(&value, PathUsage::Value);
                }
            }
            SyntaxStatementKind::Break | SyntaxStatementKind::Continue => {}
            SyntaxStatementKind::For => {
                let Some(statement) = statement.as_for() else {
                    return;
                };
                if let Some(iterable) = statement.iterable() {
                    self.bind_expr(&iterable, PathUsage::Value);
                }
                self.push_scope();
                let span = span_for(self.source, statement.syntax().text_range());
                let patterns = statement.patterns().collect::<Vec<_>>();
                if let [pattern] = patterns.as_slice() {
                    self.bind_pattern(pattern, span, LocalBindingKind::For);
                } else {
                    if let Some(index_pattern) = patterns.first() {
                        self.bind_pattern(index_pattern, span, LocalBindingKind::For);
                    }
                    if let Some(pattern) = patterns.last() {
                        self.bind_pattern(pattern, span, LocalBindingKind::For);
                    }
                }
                if let Some(body) = statement.body() {
                    self.bind_block_without_new_scope(&body);
                }
                self.pop_scope();
            }
            SyntaxStatementKind::If => {
                if let Some(statement) = statement.as_if() {
                    self.bind_if(&statement);
                }
            }
            SyntaxStatementKind::Match => {
                if let Some(statement) = statement.as_match() {
                    self.bind_match(&statement);
                }
            }
            SyntaxStatementKind::Block => {
                if let Some(block) = statement.as_block() {
                    self.bind_block(&block);
                }
            }
            SyntaxStatementKind::Expr => {
                if let Some(statement) = statement.as_expr()
                    && let Some(expr) = statement.expression()
                {
                    self.bind_expr(&expr, PathUsage::Value);
                }
            }
        }
    }

    fn bind_expr(&mut self, expr: &SyntaxExpression, usage: PathUsage) -> HirExprId {
        let id = self.next_expr(span_for(self.source, expr.syntax().text_range()));
        match expr.expression_kind() {
            SyntaxExpressionKind::Literal => {}
            SyntaxExpressionKind::Path => {
                let Some(path) = expr.as_path() else {
                    return id;
                };
                if path.is_self() {
                    return id;
                }
                self.bind_path(
                    id,
                    &path.path_segments(),
                    span_for(self.source, path.syntax().text_range()),
                    usage,
                );
            }
            SyntaxExpressionKind::Paren => {
                if let Some(expr) = expr.as_paren().and_then(|expr| expr.expression()) {
                    self.bind_expr(&expr, PathUsage::Value);
                }
            }
            SyntaxExpressionKind::Unary => {
                if let Some(expr) = expr.as_unary().and_then(|expr| expr.expression()) {
                    self.bind_expr(&expr, PathUsage::Value);
                }
            }
            SyntaxExpressionKind::Binary => {
                if let Some(expr) = expr.as_binary() {
                    if let Some(left) = expr.lhs() {
                        self.bind_expr(&left, PathUsage::Value);
                    }
                    if let Some(right) = expr.rhs() {
                        self.bind_expr(&right, PathUsage::Value);
                    }
                }
            }
            SyntaxExpressionKind::Assign => {
                if let Some(expr) = expr.as_assign() {
                    if let Some(target) = expr.target() {
                        self.bind_expr(&target, PathUsage::AssignmentTarget);
                    }
                    if let Some(value) = expr.value() {
                        self.bind_expr(&value, PathUsage::Value);
                    }
                }
            }
            SyntaxExpressionKind::Field => {
                if let Some(base) = expr.as_field().and_then(|expr| expr.receiver()) {
                    self.bind_expr(&base, PathUsage::FieldBase);
                }
            }
            SyntaxExpressionKind::Call => {
                if let Some(expr) = expr.as_call() {
                    if let Some(callee) = expr.callee() {
                        self.bind_expr(&callee, PathUsage::Callee);
                    }
                    for argument in expr.arguments() {
                        self.bind_argument(&argument);
                    }
                }
            }
            SyntaxExpressionKind::Index => {
                if let Some(expr) = expr.as_index() {
                    if let Some(base) = expr.receiver() {
                        self.bind_expr(&base, PathUsage::Value);
                    }
                    if let Some(index) = expr.index() {
                        self.bind_expr(&index, PathUsage::Value);
                    }
                }
            }
            SyntaxExpressionKind::Try => {
                if let Some(expr) = expr.as_try().and_then(|expr| expr.expression()) {
                    self.bind_expr(&expr, PathUsage::Value);
                }
            }
            SyntaxExpressionKind::Array => {
                if let Some(expr) = expr.as_array() {
                    for value in expr.expressions() {
                        self.bind_expr(&value, PathUsage::Value);
                    }
                }
            }
            SyntaxExpressionKind::Map => {
                if let Some(expr) = expr.as_map() {
                    for entry in expr.entries() {
                        self.bind_map_entry(&entry);
                    }
                }
            }
            SyntaxExpressionKind::Record => {
                if let Some(expr) = expr.as_record() {
                    self.bind_constructor_path(id, &expr.path_segments());
                    for field in expr.fields() {
                        self.bind_record_field(&field);
                    }
                }
            }
            SyntaxExpressionKind::Lambda => {
                if let Some(expr) = expr.as_lambda() {
                    self.push_scope();
                    if let Some(params) = expr.param_list() {
                        for param in params.params() {
                            if let Some(name) = param.name_text() {
                                self.declare_parameter(
                                    name,
                                    LocalBindingKind::LambdaParameter,
                                    param
                                        .type_hint()
                                        .as_ref()
                                        .map(|hint| hir_type_hint(self.source, hint)),
                                    span_for(self.source, param.syntax().text_range()),
                                );
                            }
                        }
                    }
                    if let Some(body) = expr.body() {
                        match body {
                            vela_syntax::ast::SyntaxLambdaBody::Expression(expr) => {
                                self.bind_expr(&expr, PathUsage::Value);
                            }
                            vela_syntax::ast::SyntaxLambdaBody::Block(block) => {
                                self.bind_block(&block);
                            }
                        }
                    }
                    self.pop_scope();
                }
            }
            SyntaxExpressionKind::Block => {
                if let Some(block) = expr.as_block() {
                    self.bind_block(&block);
                }
            }
            SyntaxExpressionKind::If => {
                if let Some(if_expr) = expr.as_if() {
                    self.bind_if(&if_expr);
                }
            }
            SyntaxExpressionKind::Match => {
                if let Some(match_expr) = expr.as_match() {
                    self.bind_match(&match_expr);
                }
            }
        }
        id
    }

    fn bind_argument(&mut self, argument: &SyntaxArgument) {
        if let Some(value) = argument.expression() {
            self.bind_expr(&value, PathUsage::Value);
        }
    }

    fn bind_map_entry(&mut self, entry: &SyntaxMapEntry) {
        if let Some(key) = entry.key()
            && !matches!(key.expression_kind(), SyntaxExpressionKind::Path)
        {
            self.bind_expr(&key, PathUsage::Value);
        }
        if let Some(value) = entry.value() {
            self.bind_expr(&value, PathUsage::Value);
        }
    }

    fn bind_record_field(&mut self, field: &SyntaxRecordExprField) {
        if let Some(value) = field.expression() {
            self.bind_expr(&value, PathUsage::Value);
            return;
        }
        let Some(name) = field.label_text() else {
            return;
        };
        let span = field
            .label_token()
            .map(|token| span_for(self.source, token.text_range()))
            .unwrap_or_else(|| span_for(self.source, field.syntax().text_range()));
        let id = self.next_expr(span);
        if let Some(resolution) = self.resolve_name(&name) {
            self.resolutions.insert(id, resolution);
        } else {
            self.diagnostics
                .push(self.unresolved_name_diagnostic(&name, span));
        }
    }

    fn bind_if(&mut self, if_expr: &vela_syntax::ast::SyntaxIfExpr) {
        if let Some(condition) = if_expr.condition() {
            self.bind_expr(&condition, PathUsage::Value);
        }
        if let Some(then_branch) = if_expr.then_block() {
            self.bind_block(&then_branch);
        }
        match if_expr.else_branch() {
            Some(SyntaxElseBranch::If(if_expr)) => self.bind_if(&if_expr),
            Some(SyntaxElseBranch::Block(block)) => self.bind_block(&block),
            None => {}
        }
    }

    fn bind_match(&mut self, match_expr: &vela_syntax::ast::SyntaxMatchExpr) {
        if let Some(scrutinee) = match_expr.scrutinee() {
            self.bind_expr(&scrutinee, PathUsage::Value);
        }
        for arm in match_expr.arms() {
            self.push_scope();
            if let Some(pattern) = arm.pattern() {
                let span = arm
                    .body()
                    .as_ref()
                    .map(|body| self.match_arm_body_span(body))
                    .unwrap_or_else(|| span_for(self.source, arm.syntax().text_range()));
                self.bind_pattern(&pattern, span, LocalBindingKind::Pattern);
            }
            if let Some(guard) = arm.guard() {
                self.bind_expr(&guard, PathUsage::Value);
            }
            if let Some(body) = arm.body() {
                match body {
                    vela_syntax::ast::SyntaxMatchArmBody::Expression(expr) => {
                        self.bind_expr(&expr, PathUsage::Value);
                    }
                    vela_syntax::ast::SyntaxMatchArmBody::Block(block) => {
                        self.bind_block(&block);
                    }
                }
            }
            self.pop_scope();
        }
    }

    fn match_arm_body_span(&self, body: &vela_syntax::ast::SyntaxMatchArmBody) -> Span {
        match body {
            vela_syntax::ast::SyntaxMatchArmBody::Expression(expr) => {
                span_for(self.source, expr.syntax().text_range())
            }
            vela_syntax::ast::SyntaxMatchArmBody::Block(block) => {
                span_for(self.source, block.syntax().text_range())
            }
        }
    }

    fn bind_pattern(&mut self, pattern: &SyntaxPattern, span: Span, kind: LocalBindingKind) {
        match pattern.pattern_kind() {
            Some(SyntaxPatternKind::Binding) => {
                if let Some(name) = pattern.binding_name() {
                    self.declare_local(name, kind, None, span);
                }
            }
            Some(SyntaxPatternKind::TupleVariant) => {
                let Some(pattern) = pattern.as_tuple_variant() else {
                    return;
                };
                self.bind_pattern_path(&pattern.path_segments());
                for field in pattern.patterns() {
                    self.bind_pattern(&field, span, kind);
                }
            }
            Some(SyntaxPatternKind::RecordVariant) => {
                let Some(pattern) = pattern.as_record_variant() else {
                    return;
                };
                self.bind_pattern_path(&pattern.path_segments());
                for field in pattern.fields() {
                    self.bind_record_pattern_field(&field, span, kind);
                }
            }
            Some(SyntaxPatternKind::Path) => {
                self.bind_pattern_path(&pattern.path_segments());
            }
            Some(SyntaxPatternKind::Wildcard | SyntaxPatternKind::Literal) | None => {}
        }
    }

    fn bind_record_pattern_field(
        &mut self,
        field: &SyntaxRecordPatternField,
        span: Span,
        kind: LocalBindingKind,
    ) {
        if let Some(pattern) = field.pattern() {
            self.bind_pattern(&pattern, span, kind);
        } else if let Some(name) = field.label_text() {
            self.declare_local(name, kind, None, span);
        }
    }

    fn bind_path(&mut self, id: HirExprId, path: &[String], span: Span, usage: PathUsage) {
        if path.len() > 1
            && matches!(usage, PathUsage::Callee)
            && let Some(resolution) = self.resolve_constructor_path(path)
        {
            self.resolutions.insert(id, resolution);
            return;
        }

        let [name] = path else {
            if let Some(name) = path.first()
                && let Some(BindingResolution::Local(local)) = self.resolve_name(name)
            {
                self.resolutions.insert(id, BindingResolution::Local(local));
            } else if let Some(resolution) = self.resolve_declaration_path(path) {
                self.resolutions.insert(id, resolution);
            }
            return;
        };

        if let Some(resolution) = self.resolve_name(name) {
            self.resolutions.insert(id, resolution);
            return;
        }

        if matches!(usage, PathUsage::Value | PathUsage::AssignmentTarget) {
            self.diagnostics
                .push(self.unresolved_name_diagnostic(name, span));
        }
    }

    fn bind_constructor_path(&mut self, id: HirExprId, path: &[String]) {
        if let Some(resolution) = self.resolve_constructor_path(path) {
            self.resolutions.insert(id, resolution);
        }
    }

    fn bind_pattern_path(&mut self, path: &[String]) {
        if let Some(resolution) = self.resolve_constructor_path(path) {
            self.pattern_resolutions.insert(path.to_vec(), resolution);
        }
    }

    fn resolve_constructor_path(&self, path: &[String]) -> Option<BindingResolution> {
        if let [name] = path {
            return self.resolve_declaration_name(name);
        }
        if let Some(name) = path.first()
            && let Some(resolution) = self.resolve_declaration_name(name)
        {
            return Some(resolution);
        }
        if let Some(declaration) = self.qualified_declaration(path) {
            return Some(BindingResolution::Declaration(declaration));
        }
        let (_, enum_path) = path.split_last()?;
        self.qualified_declaration(enum_path)
            .map(BindingResolution::Declaration)
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

    fn resolve_declaration_path(&self, path: &[String]) -> Option<BindingResolution> {
        let [name] = path else {
            if let Some(declaration) = self.qualified_declaration(path) {
                return Some(BindingResolution::Declaration(declaration));
            }
            return Some(BindingResolution::QualifiedPath(path.to_vec()));
        };
        self.resolve_declaration_name(name)
    }

    fn qualified_declaration(&self, path: &[String]) -> Option<HirDeclId> {
        self.qualified_declarations
            .iter()
            .find_map(|(declaration_path, declaration)| {
                (declaration_path == path).then_some(*declaration)
            })
    }

    fn unresolved_name_diagnostic(&self, name: &str, span: Span) -> Diagnostic {
        let mut diagnostic = Diagnostic::error(format!("unresolved name `{name}`"))
            .with_code("hir::unresolved_name")
            .with_span(span);

        let Some(candidate) = self.name_candidate(name) else {
            return diagnostic.with_label(span, "no similar names found");
        };

        diagnostic = diagnostic.with_label(span, format!("did you mean `{}`?", candidate.name));
        if let Some(candidate_span) = candidate.span
            && candidate_span != span
        {
            diagnostic = diagnostic.with_label(
                candidate_span,
                format!("candidate `{}` is declared here", candidate.name),
            );
        }
        diagnostic
    }

    fn name_candidate(&self, name: &str) -> Option<NameCandidate> {
        let mut candidates = self
            .scopes
            .iter()
            .rev()
            .flat_map(|scope| {
                scope.iter().filter_map(|(name, local)| {
                    self.locals
                        .get(local)
                        .map(|binding| NameCandidate::new(name.clone(), Some(binding.span)))
                })
            })
            .chain(
                self.module_declarations
                    .iter()
                    .map(|(name, _)| NameCandidate::new(name.clone(), None)),
            )
            .chain(
                self.imports
                    .iter()
                    .map(|import| NameCandidate::new(import.name.clone(), None)),
            )
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| left.name.cmp(&right.name));
        candidates.dedup_by(|left, right| left.name == right.name);

        closest_name_candidate(name, candidates)
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

    fn declare_parameter(
        &mut self,
        name: String,
        kind: LocalBindingKind,
        type_hint: Option<HirTypeHint>,
        span: Span,
    ) -> HirLocalId {
        if let Some(previous) = self
            .scopes
            .last()
            .and_then(|scope| scope.get(&name))
            .and_then(|local| self.locals.get(local))
        {
            self.diagnostics.push(
                Diagnostic::error(format!("duplicate parameter `{name}`"))
                    .with_code("hir::duplicate_parameter")
                    .with_span(span)
                    .with_label(previous.span, "previous parameter is here")
                    .with_label(span, "duplicate parameter is here"),
            );
        }
        self.declare_local(name, kind, type_hint, span)
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

fn hir_type_hint(source: SourceId, hint: &SyntaxTypeHint) -> HirTypeHint {
    HirTypeHint {
        path: hint.path_segments(),
        args: hint
            .type_arg_list()
            .into_iter()
            .flat_map(|args| args.type_hints())
            .map(|arg| hir_type_hint(source, &arg))
            .collect(),
        span: span_for(source, hint.syntax().text_range()),
    }
}

fn span_for(source: SourceId, range: TextRange) -> Span {
    Span::new(source, range.start().into(), range.end().into())
}
