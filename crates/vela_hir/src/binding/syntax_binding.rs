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
use crate::body::{
    HirBody, HirBodyOwner, HirBodyRoot, HirExprKind, HirScope, HirScopeKind, HirSourceOrigin,
};
use crate::ids::{
    HirBlockId, HirBodyId, HirCaptureId, HirDeclId, HirExprId, HirLocalId, HirScopeId,
};
use crate::type_hint::{HirTypeHint, ParamHint};

mod body_records;

pub(crate) struct SyntaxFunctionBindingInput<'a> {
    pub source: SourceId,
    pub declaration: HirDeclId,
    pub params: &'a [ParamHint],
    pub default_params: Vec<SyntaxParam>,
    pub body: SyntaxBlock,
    pub module_declarations: Vec<(String, HirDeclId)>,
    pub qualified_declarations: Vec<(Vec<String>, HirDeclId)>,
    pub imports: Vec<ImportBinding>,
    pub body_id: HirBodyId,
    pub owner: HirBodyOwner,
    pub next_expr_id: &'a mut u32,
    pub next_local_id: &'a mut u32,
    pub next_body_id: &'a mut u32,
    pub next_block_id: &'a mut u32,
    pub next_scope_id: &'a mut u32,
    pub next_stmt_id: &'a mut u32,
    pub next_pattern_id: &'a mut u32,
    pub next_param_id: &'a mut u32,
    pub next_capture_id: &'a mut u32,
}

pub(crate) struct SyntaxExpressionBindingInput<'a> {
    pub source: SourceId,
    pub declaration: HirDeclId,
    pub expression: SyntaxExpression,
    pub module_declarations: Vec<(String, HirDeclId)>,
    pub qualified_declarations: Vec<(Vec<String>, HirDeclId)>,
    pub imports: Vec<ImportBinding>,
    pub body_id: HirBodyId,
    pub owner: HirBodyOwner,
    pub next_expr_id: &'a mut u32,
    pub next_local_id: &'a mut u32,
    pub next_body_id: &'a mut u32,
    pub next_block_id: &'a mut u32,
    pub next_scope_id: &'a mut u32,
    pub next_stmt_id: &'a mut u32,
    pub next_pattern_id: &'a mut u32,
    pub next_param_id: &'a mut u32,
    pub next_capture_id: &'a mut u32,
}

pub(crate) fn bind_syntax_function(
    input: SyntaxFunctionBindingInput<'_>,
) -> (BindingMap, Vec<HirBody>, Vec<Diagnostic>) {
    SyntaxBindingLowerer::new(input).lower()
}

pub(crate) fn bind_syntax_expression_body(
    input: SyntaxExpressionBindingInput<'_>,
) -> (BindingMap, Vec<HirBody>, Vec<Diagnostic>) {
    SyntaxBindingLowerer::new_expression(input).lower()
}

struct SyntaxBindingLowerer<'a> {
    source: SourceId,
    declaration: HirDeclId,
    module_declarations: Vec<(String, HirDeclId)>,
    qualified_declarations: Vec<(Vec<String>, HirDeclId)>,
    imports: Vec<ImportBinding>,
    next_expr_id: &'a mut u32,
    next_local_id: &'a mut u32,
    next_body_id: &'a mut u32,
    next_block_id: &'a mut u32,
    next_scope_id: &'a mut u32,
    next_stmt_id: &'a mut u32,
    next_pattern_id: &'a mut u32,
    next_param_id: &'a mut u32,
    next_capture_id: &'a mut u32,
    root_body: HirBodyId,
    scopes: Vec<ActiveScope>,
    body_stack: Vec<HirBodyId>,
    block_stack: Vec<HirBlockId>,
    locals: BTreeMap<HirLocalId, LocalBinding>,
    locals_by_name: BTreeMap<String, Vec<HirLocalId>>,
    local_bodies: BTreeMap<HirLocalId, HirBodyId>,
    expressions: BTreeMap<HirExprId, ExprInfo>,
    resolutions: BTreeMap<HirExprId, BindingResolution>,
    pattern_resolutions: BTreeMap<Vec<String>, BindingResolution>,
    bodies: BTreeMap<HirBodyId, HirBody>,
    capture_keys: BTreeMap<(HirBodyId, HirLocalId), HirCaptureId>,
    diagnostics: Vec<Diagnostic>,
}

struct ActiveScope {
    id: HirScopeId,
    locals: BTreeMap<String, HirLocalId>,
}

impl<'a> SyntaxBindingLowerer<'a> {
    fn new(input: SyntaxFunctionBindingInput<'a>) -> Self {
        let body_span = span_for(input.source, input.body.syntax().text_range());
        let root_scope = HirScopeId::new(*input.next_scope_id);
        *input.next_scope_id = input.next_scope_id.saturating_add(1);
        let mut root_body = HirBody::new(
            input.body_id,
            input.owner,
            HirSourceOrigin {
                source: input.source,
                span: body_span,
            },
        );
        root_body.root_scope = Some(root_scope);
        root_body.scopes.insert(
            root_scope,
            HirScope {
                id: root_scope,
                parent: None,
                origin: HirSourceOrigin {
                    source: input.source,
                    span: body_span,
                },
                kind: HirScopeKind::Body,
                locals: Vec::new(),
                children: Vec::new(),
            },
        );
        let mut lowerer = Self {
            source: input.source,
            declaration: input.declaration,
            module_declarations: input.module_declarations,
            qualified_declarations: input.qualified_declarations,
            imports: input.imports,
            next_expr_id: input.next_expr_id,
            next_local_id: input.next_local_id,
            next_body_id: input.next_body_id,
            next_block_id: input.next_block_id,
            next_scope_id: input.next_scope_id,
            next_stmt_id: input.next_stmt_id,
            next_pattern_id: input.next_pattern_id,
            next_param_id: input.next_param_id,
            next_capture_id: input.next_capture_id,
            root_body: input.body_id,
            scopes: vec![ActiveScope {
                id: root_scope,
                locals: BTreeMap::new(),
            }],
            body_stack: vec![input.body_id],
            block_stack: Vec::new(),
            locals: BTreeMap::new(),
            locals_by_name: BTreeMap::new(),
            local_bodies: BTreeMap::new(),
            expressions: BTreeMap::new(),
            resolutions: BTreeMap::new(),
            pattern_resolutions: BTreeMap::new(),
            bodies: BTreeMap::from([(input.body_id, root_body)]),
            capture_keys: BTreeMap::new(),
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
        for (param_index, param) in input.default_params.into_iter().enumerate() {
            if let Some(default_value) = param.default_value() {
                let Some(parameter) = lowerer
                    .body_mut(input.body_id)
                    .params
                    .get(param_index)
                    .map(|param| param.id)
                else {
                    continue;
                };
                let default_body = lowerer.next_body(
                    HirBodyOwner::ParameterDefault {
                        parent: input.body_id,
                        parameter,
                    },
                    span_for(input.source, default_value.syntax().text_range()),
                );
                lowerer.with_body(default_body, |lowerer| {
                    let value = lowerer.bind_expr(&default_value, PathUsage::Value);
                    lowerer.body_mut(default_body).root = HirBodyRoot::Expr(value);
                });
                if let Some(param) = lowerer
                    .body_mut(input.body_id)
                    .params
                    .iter_mut()
                    .find(|param| param.id == parameter)
                {
                    param.default_body = Some(default_body);
                }
            }
        }
        let root_block =
            lowerer.next_block(span_for(input.source, input.body.syntax().text_range()));
        lowerer.body_mut(input.body_id).root = HirBodyRoot::Block(root_block);
        lowerer.block_stack.push(root_block);
        lowerer.bind_block_without_new_scope(&input.body);
        lowerer.block_stack.pop();
        lowerer
    }

    fn new_expression(input: SyntaxExpressionBindingInput<'a>) -> Self {
        let body_span = span_for(input.source, input.expression.syntax().text_range());
        let root_scope = HirScopeId::new(*input.next_scope_id);
        *input.next_scope_id = input.next_scope_id.saturating_add(1);
        let mut root_body = HirBody::new(
            input.body_id,
            input.owner,
            HirSourceOrigin {
                source: input.source,
                span: body_span,
            },
        );
        root_body.root_scope = Some(root_scope);
        root_body.scopes.insert(
            root_scope,
            HirScope {
                id: root_scope,
                parent: None,
                origin: HirSourceOrigin {
                    source: input.source,
                    span: body_span,
                },
                kind: HirScopeKind::Body,
                locals: Vec::new(),
                children: Vec::new(),
            },
        );
        let mut lowerer = Self {
            source: input.source,
            declaration: input.declaration,
            module_declarations: input.module_declarations,
            qualified_declarations: input.qualified_declarations,
            imports: input.imports,
            next_expr_id: input.next_expr_id,
            next_local_id: input.next_local_id,
            next_body_id: input.next_body_id,
            next_block_id: input.next_block_id,
            next_scope_id: input.next_scope_id,
            next_stmt_id: input.next_stmt_id,
            next_pattern_id: input.next_pattern_id,
            next_param_id: input.next_param_id,
            next_capture_id: input.next_capture_id,
            root_body: input.body_id,
            scopes: vec![ActiveScope {
                id: root_scope,
                locals: BTreeMap::new(),
            }],
            body_stack: vec![input.body_id],
            block_stack: Vec::new(),
            locals: BTreeMap::new(),
            locals_by_name: BTreeMap::new(),
            local_bodies: BTreeMap::new(),
            expressions: BTreeMap::new(),
            resolutions: BTreeMap::new(),
            pattern_resolutions: BTreeMap::new(),
            bodies: BTreeMap::from([(input.body_id, root_body)]),
            capture_keys: BTreeMap::new(),
            diagnostics: Vec::new(),
        };
        let value = lowerer.bind_expr(&input.expression, PathUsage::Value);
        lowerer.body_mut(input.body_id).root = HirBodyRoot::Expr(value);
        lowerer
    }

    fn lower(self) -> (BindingMap, Vec<HirBody>, Vec<Diagnostic>) {
        (
            BindingMap {
                declaration: self.declaration,
                body: self.root_body,
                locals: self.locals,
                locals_by_name: self.locals_by_name,
                expressions: self.expressions,
                resolutions: self.resolutions,
                pattern_resolutions: self.pattern_resolutions,
            },
            self.bodies.into_values().collect(),
            self.diagnostics,
        )
    }

    fn bind_block(&mut self, block: &SyntaxBlock) {
        self.push_scope(
            HirScopeKind::Block,
            span_for(self.source, block.syntax().text_range()),
        );
        let block_id = self.next_block(span_for(self.source, block.syntax().text_range()));
        self.block_stack.push(block_id);
        self.bind_block_without_new_scope(block);
        self.block_stack.pop();
        self.pop_scope();
    }

    fn bind_block_without_new_scope(&mut self, block: &SyntaxBlock) {
        for statement in block.statements() {
            self.bind_statement(&statement);
        }
    }

    fn bind_statement(&mut self, statement: &SyntaxStatement) {
        self.next_stmt(
            span_for(self.source, statement.syntax().text_range()),
            statement.statement_kind().into(),
        );
        match statement.statement_kind() {
            SyntaxStatementKind::Let => {
                let Some(statement) = statement.as_let() else {
                    return;
                };
                if let Some(value) = statement.initializer() {
                    self.bind_expr(&value, PathUsage::Value);
                }
                if let Some(pattern) = statement.pattern() {
                    self.bind_pattern(
                        &pattern,
                        span_for(self.source, statement.syntax().text_range()),
                        LocalBindingKind::Let,
                    );
                } else if let Some(name) = statement.name_text() {
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
                let span = span_for(self.source, statement.syntax().text_range());
                self.push_scope(HirScopeKind::For, span);
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
        let id = self.next_expr(
            span_for(self.source, expr.syntax().text_range()),
            expr.expression_kind().into(),
        );
        match expr.expression_kind() {
            SyntaxExpressionKind::Literal => {}
            SyntaxExpressionKind::Path => {
                let Some(path) = expr.as_path() else {
                    return id;
                };
                if path.is_self() {
                    self.bind_self_path(id);
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
            SyntaxExpressionKind::Unit => {}
            SyntaxExpressionKind::Tuple => {
                if let Some(expr) = expr.as_tuple() {
                    for value in expr.expressions() {
                        self.bind_expr(&value, PathUsage::Value);
                    }
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
                    let parent_body = self.current_body();
                    let lambda_body = self.next_body(
                        HirBodyOwner::Lambda {
                            parent: parent_body,
                            expression: id,
                        },
                        span_for(self.source, expr.syntax().text_range()),
                    );
                    self.with_body(lambda_body, |lowerer| {
                        if let Some(params) = expr.param_list() {
                            for param in params.params() {
                                if let Some(name) = param.name_text() {
                                    lowerer.declare_parameter(
                                        name,
                                        LocalBindingKind::LambdaParameter,
                                        param
                                            .type_hint()
                                            .as_ref()
                                            .map(|hint| hir_type_hint(lowerer.source, hint)),
                                        span_for(lowerer.source, param.syntax().text_range()),
                                    );
                                }
                            }
                        }
                        if let Some(body) = expr.body() {
                            match body {
                                vela_syntax::ast::SyntaxLambdaBody::Expression(expr) => {
                                    let value = lowerer.bind_expr(&expr, PathUsage::Value);
                                    lowerer.body_mut(lambda_body).root = HirBodyRoot::Expr(value);
                                }
                                vela_syntax::ast::SyntaxLambdaBody::Block(block) => {
                                    let root_block = lowerer.next_block(span_for(
                                        lowerer.source,
                                        block.syntax().text_range(),
                                    ));
                                    lowerer.body_mut(lambda_body).root =
                                        HirBodyRoot::Block(root_block);
                                    lowerer.block_stack.push(root_block);
                                    lowerer.bind_block_without_new_scope(&block);
                                    lowerer.block_stack.pop();
                                }
                            }
                        }
                    });
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
        let id = self.next_expr(span, HirExprKind::Path);
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
            self.push_scope(
                HirScopeKind::MatchArm,
                span_for(self.source, arm.syntax().text_range()),
            );
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
        let pattern_id = self.next_pattern(
            span_for(self.source, pattern.syntax().text_range()),
            pattern.pattern_kind().into(),
        );
        match pattern.pattern_kind() {
            Some(SyntaxPatternKind::Binding) => {
                if let Some(name_token) = pattern.binding_name_token() {
                    let local = self.declare_pattern_local(
                        name_token.text().to_owned(),
                        kind,
                        span_for(self.source, name_token.text_range()),
                        span,
                    );
                    if let Some(pattern) = self
                        .body_mut(self.current_body())
                        .patterns
                        .get_mut(&pattern_id)
                    {
                        pattern.local = Some(local);
                    }
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
        } else if let Some(name_token) = field.shorthand_binding_name_token() {
            self.declare_pattern_local(
                name_token.text().to_owned(),
                kind,
                span_for(self.source, name_token.text_range()),
                span,
            );
        }
    }

    fn bind_path(&mut self, id: HirExprId, path: &[String], span: Span, usage: PathUsage) {
        if path.len() > 1
            && matches!(usage, PathUsage::Callee)
            && let Some(resolution) = self.resolve_constructor_path(path)
        {
            self.record_capture_for_resolution(id, &resolution);
            self.resolutions.insert(id, resolution);
            return;
        }

        let [name] = path else {
            if let Some(name) = path.first()
                && let Some(BindingResolution::Local(local)) = self.resolve_name(name)
            {
                let resolution = BindingResolution::Local(local);
                self.record_capture_for_resolution(id, &resolution);
                self.resolutions.insert(id, resolution);
            } else if let Some(resolution) = self.resolve_declaration_path(path) {
                self.record_capture_for_resolution(id, &resolution);
                self.resolutions.insert(id, resolution);
            }
            return;
        };

        if let Some(resolution) = self.resolve_name(name) {
            self.record_capture_for_resolution(id, &resolution);
            self.resolutions.insert(id, resolution);
            return;
        }

        if matches!(
            usage,
            PathUsage::Value | PathUsage::Callee | PathUsage::AssignmentTarget
        ) {
            self.record_unresolved_reference(id, name.clone(), span);
        }

        if matches!(usage, PathUsage::Value | PathUsage::AssignmentTarget) {
            self.diagnostics
                .push(self.unresolved_name_diagnostic(name, span));
        }
    }

    fn bind_self_path(&mut self, id: HirExprId) {
        let Some(resolution) = self.resolve_name("self") else {
            return;
        };
        self.record_capture_for_resolution(id, &resolution);
        self.record_self_use(id, &resolution);
        self.resolutions.insert(id, resolution);
    }

    fn bind_constructor_path(&mut self, id: HirExprId, path: &[String]) {
        if let Some(resolution) = self.resolve_constructor_path(path) {
            self.record_capture_for_resolution(id, &resolution);
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
            if let Some(local) = scope.locals.get(name) {
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
                scope.locals.iter().filter_map(|(name, local)| {
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
        self.declare_local_with_scope(name, kind, type_hint, span, None)
    }

    fn declare_pattern_local(
        &mut self,
        name: String,
        kind: LocalBindingKind,
        span: Span,
        scope_span: Span,
    ) -> HirLocalId {
        self.declare_local_with_scope(name, kind, None, span, Some(scope_span))
    }

    fn declare_local_with_scope(
        &mut self,
        name: String,
        kind: LocalBindingKind,
        type_hint: Option<HirTypeHint>,
        span: Span,
        scope_span: Option<Span>,
    ) -> HirLocalId {
        let id = self.next_local();
        self.scopes
            .last_mut()
            .expect("function binding always has a scope")
            .locals
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
                scope_span,
            },
        );
        let body = self.current_body();
        let scope = self
            .scopes
            .last()
            .expect("function binding always has a scope")
            .id;
        self.local_bodies.insert(id, body);
        self.body_mut(body).locals.push(id);
        if let Some(scope) = self.body_mut(body).scopes.get_mut(&scope) {
            scope.locals.push(id);
        }
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
            .and_then(|scope| scope.locals.get(&name))
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
        let local = self.declare_local(name, kind, type_hint, span);
        if self
            .locals
            .get(&local)
            .is_some_and(|binding| binding.name == "self")
        {
            self.body_mut(self.current_body()).self_binding = Some(local);
        }
        self.next_param(local, span);
        local
    }

    fn push_scope(&mut self, kind: HirScopeKind, span: Span) {
        let id = HirScopeId::new(*self.next_scope_id);
        *self.next_scope_id = self.next_scope_id.saturating_add(1);
        let body = self.current_body();
        let parent = self.scopes.last().map(|scope| scope.id);
        let source = self.source;
        self.body_mut(body).scopes.insert(
            id,
            HirScope {
                id,
                parent,
                origin: HirSourceOrigin { source, span },
                kind,
                locals: Vec::new(),
                children: Vec::new(),
            },
        );
        if let Some(parent) = parent
            && let Some(parent_scope) = self.body_mut(body).scopes.get_mut(&parent)
        {
            parent_scope.children.push(id);
        }
        self.scopes.push(ActiveScope {
            id,
            locals: BTreeMap::new(),
        });
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn next_local(&mut self) -> HirLocalId {
        let id = HirLocalId::new(*self.next_local_id);
        *self.next_local_id = self.next_local_id.saturating_add(1);
        id
    }
}

fn hir_type_hint(source: SourceId, hint: &SyntaxTypeHint) -> HirTypeHint {
    let span = span_for(source, hint.syntax().text_range());
    if hint.is_unit() {
        return HirTypeHint {
            path: vec![HirTypeHint::UNIT_PATH.to_owned()],
            args: Vec::new(),
            span,
        };
    }

    let tuple_elements = hint.tuple_element_hints().collect::<Vec<_>>();
    if hint.is_tuple() {
        return HirTypeHint {
            path: vec![HirTypeHint::UNIT_PATH.to_owned()],
            args: tuple_elements
                .iter()
                .map(|arg| hir_type_hint(source, arg))
                .collect(),
            span,
        };
    }

    if hint.l_paren_token().is_some() && tuple_elements.len() == 1 {
        return hir_type_hint(source, &tuple_elements[0]);
    }

    HirTypeHint {
        path: hint.path_segments(),
        args: hint
            .type_arg_list()
            .into_iter()
            .flat_map(|args| args.type_hints())
            .map(|arg| hir_type_hint(source, &arg))
            .collect(),
        span,
    }
}

fn span_for(source: SourceId, range: TextRange) -> Span {
    Span::new(source, range.start().into(), range.end().into())
}
