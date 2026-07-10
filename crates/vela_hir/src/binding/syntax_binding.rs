use std::collections::BTreeMap;

use vela_common::{Diagnostic, SourceId, Span};
use vela_syntax::SyntaxToken;
use vela_syntax::ast::{
    AstNode, SyntaxArgument, SyntaxBlock, SyntaxElseBranch, SyntaxExpression, SyntaxExpressionKind,
    SyntaxMapEntry, SyntaxParam, SyntaxPattern, SyntaxPatternKind, SyntaxRecordExprField,
    SyntaxRecordPatternField, SyntaxStatement, SyntaxStatementKind,
};

use crate::binding::{
    BindingMap, BindingResolution, ImportBinding, LocalBinding, LocalBindingKind, PathUsage,
};
use crate::body::{
    HirArgument, HirBody, HirBodyOwner, HirBodyRoot, HirCall, HirElseBranch, HirExprKind, HirField,
    HirIf, HirIndex, HirLiteral, HirMapEntry, HirMatch, HirMatchArmBody, HirPathKind, HirPathOwner,
    HirPatternKind, HirRecordField, HirRecordPatternField, HirScope, HirScopeKind, HirSourceOrigin,
    HirStmtKind,
};
use crate::ids::{
    HirBlockId, HirBodyId, HirCaptureId, HirDeclId, HirExprId, HirLocalId, HirPatternId,
    HirScopeId, HirStmtId,
};
use crate::type_hint::ParamHint;

mod body_records;
mod lowering_values;
mod resolution;
mod scopes;

use lowering_values::{
    hir_assign_op, hir_binary_op, hir_literal, hir_path_kind_for_usage, hir_type_hint,
    hir_unary_op, last_segment_span, span_for,
};

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
    pub next_match_arm_id: &'a mut u32,
    pub next_path_id: &'a mut u32,
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
    pub next_match_arm_id: &'a mut u32,
    pub next_path_id: &'a mut u32,
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
    next_match_arm_id: &'a mut u32,
    next_path_id: &'a mut u32,
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
            root_scope,
        );
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
            next_match_arm_id: input.next_match_arm_id,
            next_path_id: input.next_path_id,
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
            root_scope,
        );
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
            next_match_arm_id: input.next_match_arm_id,
            next_path_id: input.next_path_id,
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
                resolutions: self.resolutions,
                pattern_resolutions: self.pattern_resolutions,
            },
            self.bodies.into_values().collect(),
            self.diagnostics,
        )
    }

    fn bind_block(&mut self, block: &SyntaxBlock) -> HirBlockId {
        self.push_scope(
            HirScopeKind::Block,
            span_for(self.source, block.syntax().text_range()),
        );
        let block_id = self.next_block(span_for(self.source, block.syntax().text_range()));
        self.block_stack.push(block_id);
        self.bind_block_without_new_scope(block);
        self.block_stack.pop();
        self.pop_scope();
        block_id
    }

    fn bind_block_without_new_scope(&mut self, block: &SyntaxBlock) {
        for statement in block.statements() {
            self.bind_statement(&statement);
        }
    }

    fn bind_block_in_current_scope(&mut self, block: &SyntaxBlock) -> HirBlockId {
        let block_id = self.next_block(span_for(self.source, block.syntax().text_range()));
        self.block_stack.push(block_id);
        self.bind_block_without_new_scope(block);
        self.block_stack.pop();
        block_id
    }

    fn bind_statement(&mut self, statement: &SyntaxStatement) {
        let statement_id = self.next_stmt(span_for(self.source, statement.syntax().text_range()));
        self.bind_statement_inner(statement, statement_id);
    }

    fn bind_statement_inner(&mut self, statement: &SyntaxStatement, statement_id: HirStmtId) {
        match statement.statement_kind() {
            SyntaxStatementKind::Let => {
                let Some(statement) = statement.as_let() else {
                    return;
                };
                let initializer = statement
                    .initializer()
                    .map(|value| self.bind_expr(&value, PathUsage::Value));
                let pattern = if let Some(pattern) = statement.pattern() {
                    Some(self.bind_pattern(
                        &pattern,
                        span_for(self.source, statement.syntax().text_range()),
                        LocalBindingKind::Let,
                    ))
                } else if let Some(name_token) = statement.name_token() {
                    let pattern_id =
                        self.next_pattern(span_for(self.source, name_token.text_range()));
                    let local = self.declare_local_with_scope(
                        name_token.text().to_owned(),
                        LocalBindingKind::Let,
                        statement
                            .type_hint()
                            .as_ref()
                            .map(|hint| hir_type_hint(self.source, hint)),
                        span_for(self.source, name_token.text_range()),
                        Some(span_for(self.source, statement.syntax().text_range())),
                    );
                    self.finish_pattern(pattern_id, HirPatternKind::Binding { local: Some(local) });
                    Some(pattern_id)
                } else {
                    None
                };
                self.finish_stmt(
                    statement_id,
                    HirStmtKind::Let {
                        pattern,
                        type_hint: statement
                            .type_hint()
                            .as_ref()
                            .map(|hint| hir_type_hint(self.source, hint)),
                        initializer,
                    },
                );
            }
            SyntaxStatementKind::Return => {
                let value = statement
                    .as_return()
                    .and_then(|statement| statement.expression())
                    .map(|value| self.bind_expr(&value, PathUsage::Value));
                self.finish_stmt(statement_id, HirStmtKind::Return { value });
            }
            SyntaxStatementKind::Break => self.finish_stmt(statement_id, HirStmtKind::Break),
            SyntaxStatementKind::Continue => {
                self.finish_stmt(statement_id, HirStmtKind::Continue);
            }
            SyntaxStatementKind::For => {
                let Some(statement) = statement.as_for() else {
                    return;
                };
                let iterable = statement
                    .iterable()
                    .map(|iterable| self.bind_expr(&iterable, PathUsage::Value));
                let span = span_for(self.source, statement.syntax().text_range());
                self.push_scope(HirScopeKind::For, span);
                let syntax_patterns = statement.patterns().collect::<Vec<_>>();
                let mut patterns = Vec::new();
                if let [pattern] = syntax_patterns.as_slice() {
                    patterns.push(self.bind_pattern(pattern, span, LocalBindingKind::For));
                } else {
                    if let Some(index_pattern) = syntax_patterns.first() {
                        patterns.push(self.bind_pattern(
                            index_pattern,
                            span,
                            LocalBindingKind::For,
                        ));
                    }
                    if let Some(pattern) = syntax_patterns.last() {
                        patterns.push(self.bind_pattern(pattern, span, LocalBindingKind::For));
                    }
                }
                let body = statement
                    .body()
                    .map(|body| self.bind_block_in_current_scope(&body));
                self.pop_scope();
                self.finish_stmt(
                    statement_id,
                    HirStmtKind::For {
                        patterns,
                        iterable,
                        body,
                    },
                );
            }
            SyntaxStatementKind::If => {
                if let Some(statement) = statement.as_if() {
                    let value = self.bind_if(&statement);
                    self.finish_stmt(statement_id, HirStmtKind::If(value));
                }
            }
            SyntaxStatementKind::Match => {
                if let Some(statement) = statement.as_match() {
                    let value = self.bind_match(&statement);
                    self.finish_stmt(statement_id, HirStmtKind::Match(value));
                }
            }
            SyntaxStatementKind::Block => {
                if let Some(block) = statement.as_block() {
                    let block = self.bind_block(&block);
                    self.finish_stmt(statement_id, HirStmtKind::Block(block));
                }
            }
            SyntaxStatementKind::Expr => {
                let statement = statement.as_expr();
                let expression = statement
                    .as_ref()
                    .and_then(|statement| statement.expression())
                    .map(|expr| self.bind_expr(&expr, PathUsage::Value));
                let terminated =
                    statement.is_some_and(|statement| statement.semicolon_token().is_some());
                self.finish_stmt(
                    statement_id,
                    HirStmtKind::Expr {
                        expression,
                        terminated,
                    },
                );
            }
        }
    }

    fn bind_expr(&mut self, expr: &SyntaxExpression, usage: PathUsage) -> HirExprId {
        if matches!(expr.expression_kind(), SyntaxExpressionKind::Binary) {
            return self.bind_binary_expr(expr);
        }
        let span = span_for(self.source, expr.syntax().text_range());
        let id = self.next_expr(span);
        let kind = match expr.expression_kind() {
            SyntaxExpressionKind::Literal => HirExprKind::Literal(self.bind_literal(expr)),
            SyntaxExpressionKind::Path => {
                let Some(path) = expr.as_path() else {
                    self.finish_expr(
                        id,
                        HirExprKind::Literal(HirLiteral::Invalid {
                            source_text: expr.syntax().text().to_string(),
                        }),
                    );
                    return id;
                };
                let path_segments = path.path_segments();
                let segment_span =
                    last_segment_span(self.source, path.path_tokens()).unwrap_or(span);
                let path_id = self.next_path(
                    HirPathOwner::Expression(id),
                    hir_path_kind_for_usage(usage),
                    if path.is_self() {
                        vec!["self".to_owned()]
                    } else {
                        path_segments.clone()
                    },
                    span,
                    segment_span,
                );
                if path.is_self() {
                    self.bind_self_path(id);
                } else {
                    self.bind_path(id, &path_segments, span, usage);
                }
                HirExprKind::Path(path_id)
            }
            SyntaxExpressionKind::Paren => {
                let expression = expr
                    .as_paren()
                    .and_then(|expr| expr.expression())
                    .map(|expr| self.bind_expr(&expr, PathUsage::Value));
                HirExprKind::Paren { expression }
            }
            SyntaxExpressionKind::Unit => HirExprKind::Unit,
            SyntaxExpressionKind::Tuple => {
                let elements = expr
                    .as_tuple()
                    .into_iter()
                    .flat_map(|expr| expr.expressions())
                    .map(|value| self.bind_expr(&value, PathUsage::Value))
                    .collect();
                HirExprKind::Tuple { elements }
            }
            SyntaxExpressionKind::Unary => {
                let unary = expr.as_unary();
                let op = unary
                    .as_ref()
                    .and_then(|expr| expr.operator())
                    .map(hir_unary_op);
                let operand = unary
                    .and_then(|expr| expr.expression())
                    .map(|expr| self.bind_expr(&expr, PathUsage::Value));
                HirExprKind::Unary { op, operand }
            }
            SyntaxExpressionKind::Binary => {
                unreachable!("binary expressions use iterative HIR lowering")
            }
            SyntaxExpressionKind::Assign => {
                let assign = expr.as_assign();
                let op = assign
                    .as_ref()
                    .and_then(|expr| expr.operator())
                    .map(hir_assign_op);
                let target = assign
                    .as_ref()
                    .and_then(|expr| expr.target())
                    .map(|expr| self.bind_expr(&expr, PathUsage::AssignmentTarget));
                let value = assign
                    .and_then(|expr| expr.value())
                    .map(|expr| self.bind_expr(&expr, PathUsage::Value));
                HirExprKind::Assign { op, target, value }
            }
            SyntaxExpressionKind::Field => {
                let field = expr.as_field();
                let receiver = field
                    .as_ref()
                    .and_then(|field| field.receiver())
                    .map(|base| self.bind_expr(&base, PathUsage::FieldBase))
                    .unwrap_or_else(|| self.missing_expr(span));
                let member_token = field
                    .and_then(|field| field.name_token().or_else(|| field.tuple_index_token()));
                let name = member_token
                    .as_ref()
                    .map_or_else(String::new, |token| token.text().to_owned());
                let member_origin = HirSourceOrigin {
                    source: self.source,
                    span: member_token
                        .map_or(span, |token| span_for(self.source, token.text_range())),
                };
                HirExprKind::Field(HirField {
                    expression: id,
                    receiver,
                    name,
                    member_origin,
                })
            }
            SyntaxExpressionKind::Call => {
                let call = expr.as_call();
                let callee = call
                    .as_ref()
                    .and_then(|expr| expr.callee())
                    .map(|callee| self.bind_expr(&callee, PathUsage::Callee))
                    .unwrap_or_else(|| self.missing_expr(span));
                let arguments = call
                    .into_iter()
                    .flat_map(|expr| expr.arguments())
                    .map(|argument| self.bind_argument(&argument))
                    .collect();
                HirExprKind::Call(HirCall {
                    expression: id,
                    callee,
                    arguments,
                })
            }
            SyntaxExpressionKind::Index => {
                let index = expr.as_index();
                let receiver = index
                    .as_ref()
                    .and_then(|expr| expr.receiver())
                    .map(|base| self.bind_expr(&base, PathUsage::Value))
                    .unwrap_or_else(|| self.missing_expr(span));
                let index = index
                    .and_then(|expr| expr.index())
                    .map(|index| self.bind_expr(&index, PathUsage::Value))
                    .unwrap_or_else(|| self.missing_expr(span));
                HirExprKind::Index(HirIndex {
                    expression: id,
                    receiver,
                    index,
                })
            }
            SyntaxExpressionKind::Try => {
                let expression = expr
                    .as_try()
                    .and_then(|expr| expr.expression())
                    .map(|expr| self.bind_expr(&expr, PathUsage::Value));
                HirExprKind::Try { expression }
            }
            SyntaxExpressionKind::Array => {
                let elements = expr
                    .as_array()
                    .into_iter()
                    .flat_map(|expr| expr.expressions())
                    .map(|value| self.bind_expr(&value, PathUsage::Value))
                    .collect();
                HirExprKind::Array { elements }
            }
            SyntaxExpressionKind::Map => {
                let entries = expr
                    .as_map()
                    .into_iter()
                    .flat_map(|expr| expr.entries())
                    .map(|entry| self.bind_map_entry(&entry))
                    .collect();
                HirExprKind::Map { entries }
            }
            SyntaxExpressionKind::Record => {
                let record = expr.as_record();
                let constructor = record.as_ref().map(|record| {
                    let path = record.path_segments();
                    let segment_span =
                        last_segment_span(self.source, record.path_tokens()).unwrap_or(span);
                    let path_id = self.next_path(
                        HirPathOwner::Expression(id),
                        HirPathKind::Constructor,
                        path.clone(),
                        span,
                        segment_span,
                    );
                    self.bind_constructor_path(id, &path);
                    path_id
                });
                let fields = record
                    .into_iter()
                    .flat_map(|record| record.fields())
                    .map(|field| self.bind_record_field(&field))
                    .collect();
                HirExprKind::Record {
                    constructor,
                    fields,
                }
            }
            SyntaxExpressionKind::Lambda => {
                let Some(lambda) = expr.as_lambda() else {
                    self.finish_expr(
                        id,
                        HirExprKind::Literal(HirLiteral::Invalid {
                            source_text: expr.syntax().text().to_string(),
                        }),
                    );
                    return id;
                };
                let parent_body = self.current_body();
                let lambda_body = self.next_body(
                    HirBodyOwner::Lambda {
                        parent: parent_body,
                        expression: id,
                    },
                    span,
                );
                self.with_body(lambda_body, |lowerer| {
                    if let Some(params) = lambda.param_list() {
                        for param in params.params() {
                            if let Some(name_token) = param.name_token() {
                                lowerer.declare_parameter(
                                    name_token.text().to_owned(),
                                    LocalBindingKind::LambdaParameter,
                                    param
                                        .type_hint()
                                        .as_ref()
                                        .map(|hint| hir_type_hint(lowerer.source, hint)),
                                    span_for(lowerer.source, name_token.text_range()),
                                );
                            }
                        }
                    }
                    if let Some(body) = lambda.body() {
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
                                lowerer.body_mut(lambda_body).root = HirBodyRoot::Block(root_block);
                                lowerer.block_stack.push(root_block);
                                lowerer.bind_block_without_new_scope(&block);
                                lowerer.block_stack.pop();
                            }
                        }
                    }
                });
                HirExprKind::Lambda { body: lambda_body }
            }
            SyntaxExpressionKind::Block => {
                let block = expr
                    .as_block()
                    .map(|block| self.bind_block(&block))
                    .unwrap_or_else(|| self.next_block(span));
                HirExprKind::Block { block }
            }
            SyntaxExpressionKind::If => HirExprKind::If(expr.as_if().map_or(
                HirIf {
                    condition: None,
                    then_block: None,
                    else_branch: None,
                },
                |if_expr| self.bind_if(&if_expr),
            )),
            SyntaxExpressionKind::Match => HirExprKind::Match(expr.as_match().map_or(
                HirMatch {
                    scrutinee: None,
                    arms: Vec::new(),
                },
                |match_expr| self.bind_match(&match_expr),
            )),
        };
        self.finish_expr(id, kind);
        id
    }

    fn bind_binary_expr(&mut self, expr: &SyntaxExpression) -> HirExprId {
        let mut pending = Vec::new();
        let mut current = expr.clone();

        loop {
            let span = span_for(self.source, current.syntax().text_range());
            let id = self.next_expr(span);
            let binary = current
                .as_binary()
                .expect("binary expression kind should expose binary syntax");
            let op = binary.operator().map(hir_binary_op);
            let lhs = binary.lhs();
            let rhs = binary.rhs();

            if lhs
                .as_ref()
                .is_some_and(|lhs| matches!(lhs.expression_kind(), SyntaxExpressionKind::Binary))
            {
                pending.push((id, op, rhs));
                current = lhs.expect("binary left operand checked above");
                continue;
            }

            let lhs = lhs.map(|lhs| self.bind_expr(&lhs, PathUsage::Value));
            let rhs = rhs.map(|rhs| self.bind_expr(&rhs, PathUsage::Value));
            self.finish_expr(id, HirExprKind::Binary { op, lhs, rhs });

            let mut completed = id;
            while let Some((id, op, rhs)) = pending.pop() {
                let rhs = rhs.map(|rhs| self.bind_expr(&rhs, PathUsage::Value));
                self.finish_expr(
                    id,
                    HirExprKind::Binary {
                        op,
                        lhs: Some(completed),
                        rhs,
                    },
                );
                completed = id;
            }
            return completed;
        }
    }

    fn bind_argument(&mut self, argument: &SyntaxArgument) -> HirArgument {
        let name_token = argument.name_token();
        HirArgument {
            name: name_token.as_ref().map(|token| token.text().to_owned()),
            name_origin: name_token.map(|token| HirSourceOrigin {
                source: self.source,
                span: span_for(self.source, token.text_range()),
            }),
            value: argument
                .expression()
                .map(|value| self.bind_expr(&value, PathUsage::Value)),
            origin: HirSourceOrigin {
                source: self.source,
                span: span_for(self.source, argument.syntax().text_range()),
            },
        }
    }

    fn bind_map_entry(&mut self, entry: &SyntaxMapEntry) -> HirMapEntry {
        let key = entry.key().map(|key| {
            if matches!(key.expression_kind(), SyntaxExpressionKind::Path) {
                self.bind_bare_map_key(&key)
            } else {
                self.bind_expr(&key, PathUsage::Value)
            }
        });
        let value = entry
            .value()
            .map(|value| self.bind_expr(&value, PathUsage::Value));
        HirMapEntry {
            key,
            value,
            origin: HirSourceOrigin {
                source: self.source,
                span: span_for(self.source, entry.syntax().text_range()),
            },
        }
    }

    fn bind_bare_map_key(&mut self, key: &SyntaxExpression) -> HirExprId {
        let span = span_for(self.source, key.syntax().text_range());
        let id = self.next_expr(span);
        let path = key.as_path();
        let path_segments = path
            .as_ref()
            .map_or_else(Vec::new, |path| path.path_segments());
        let segment_span = path
            .as_ref()
            .and_then(|path| last_segment_span(self.source, path.path_tokens()))
            .unwrap_or(span);
        let path_id = self.next_path(
            HirPathOwner::Expression(id),
            HirPathKind::Value,
            path_segments,
            span,
            segment_span,
        );
        self.finish_expr(id, HirExprKind::Path(path_id));
        id
    }

    fn bind_record_field(&mut self, field: &SyntaxRecordExprField) -> HirRecordField {
        let name_token = field.label_token();
        let name = name_token
            .as_ref()
            .map_or_else(String::new, |token| token.text().to_owned());
        let name_span = name_token.as_ref().map_or_else(
            || span_for(self.source, field.syntax().text_range()),
            |token| span_for(self.source, token.text_range()),
        );
        let shorthand = field.is_shorthand();
        let value = if let Some(value) = field.expression() {
            Some(self.bind_expr(&value, PathUsage::Value))
        } else if shorthand {
            let id = self.next_expr(name_span);
            let path_id = self.next_path(
                HirPathOwner::Expression(id),
                HirPathKind::Value,
                vec![name.clone()],
                name_span,
                name_span,
            );
            if let Some(resolution) = self.resolve_name(&name) {
                self.record_capture_for_resolution(id, &resolution);
                self.resolutions.insert(id, resolution);
            } else {
                self.record_unresolved_reference(id, name.clone(), name_span);
                self.diagnostics
                    .push(self.unresolved_name_diagnostic(&name, name_span));
            }
            self.finish_expr(id, HirExprKind::Path(path_id));
            Some(id)
        } else {
            None
        };
        HirRecordField {
            name,
            name_origin: HirSourceOrigin {
                source: self.source,
                span: name_span,
            },
            value,
            shorthand,
        }
    }

    fn bind_literal(&mut self, expr: &SyntaxExpression) -> HirLiteral {
        let Some(literal) = expr.as_literal() else {
            return HirLiteral::Invalid {
                source_text: expr.syntax().text().to_string(),
            };
        };
        if let Some(value) = literal.literal() {
            return hir_literal(value);
        }
        let expressions = literal
            .interpolation_expressions()
            .map(|expression| self.bind_expr(&expression, PathUsage::Value))
            .collect::<Vec<_>>();
        let source_text = literal
            .token_text()
            .unwrap_or_else(|| literal.syntax().text().to_string());
        if expressions.is_empty() {
            HirLiteral::Invalid { source_text }
        } else {
            HirLiteral::Interpolated {
                source_text,
                expressions,
            }
        }
    }

    fn bind_if(&mut self, if_expr: &vela_syntax::ast::SyntaxIfExpr) -> HirIf {
        let condition = if_expr
            .condition()
            .map(|condition| self.bind_expr(&condition, PathUsage::Value));
        let then_block = if_expr
            .then_block()
            .map(|then_branch| self.bind_block(&then_branch));
        let else_branch = match if_expr.else_branch() {
            Some(SyntaxElseBranch::If(if_expr)) => {
                Some(HirElseBranch::If(Box::new(self.bind_if(&if_expr))))
            }
            Some(SyntaxElseBranch::Block(block)) => {
                Some(HirElseBranch::Block(self.bind_block(&block)))
            }
            None => None,
        };
        HirIf {
            condition,
            then_block,
            else_branch,
        }
    }

    fn bind_match(&mut self, match_expr: &vela_syntax::ast::SyntaxMatchExpr) -> HirMatch {
        let scrutinee = match_expr
            .scrutinee()
            .map(|scrutinee| self.bind_expr(&scrutinee, PathUsage::Value));
        let mut arms = Vec::new();
        for arm in match_expr.arms() {
            self.push_scope(
                HirScopeKind::MatchArm,
                span_for(self.source, arm.syntax().text_range()),
            );
            let scope = self.current_scope();
            let pattern = arm.pattern().map(|pattern| {
                let span = arm
                    .body()
                    .as_ref()
                    .map(|body| self.match_arm_body_span(body))
                    .unwrap_or_else(|| span_for(self.source, arm.syntax().text_range()));
                self.bind_pattern(&pattern, span, LocalBindingKind::Pattern)
            });
            let guard = arm
                .guard()
                .map(|guard| self.bind_expr(&guard, PathUsage::Value));
            let body = arm.body().map(|body| match body {
                vela_syntax::ast::SyntaxMatchArmBody::Expression(expr) => {
                    HirMatchArmBody::Expr(self.bind_expr(&expr, PathUsage::Value))
                }
                vela_syntax::ast::SyntaxMatchArmBody::Block(block) => {
                    HirMatchArmBody::Block(self.bind_block(&block))
                }
            });
            let arm_id = self.next_match_arm(
                span_for(self.source, arm.syntax().text_range()),
                scope,
                pattern,
                guard,
                body,
            );
            arms.push(arm_id);
            self.pop_scope();
        }
        HirMatch { scrutinee, arms }
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

    fn bind_pattern(
        &mut self,
        pattern: &SyntaxPattern,
        span: Span,
        kind: LocalBindingKind,
    ) -> HirPatternId {
        let pattern_span = span_for(self.source, pattern.syntax().text_range());
        let pattern_id = self.next_pattern(pattern_span);
        let payload = match pattern.pattern_kind() {
            Some(SyntaxPatternKind::Binding) => {
                let local = pattern.binding_name_token().map(|name_token| {
                    self.declare_pattern_local(
                        name_token.text().to_owned(),
                        kind,
                        span_for(self.source, name_token.text_range()),
                        span,
                    )
                });
                HirPatternKind::Binding { local }
            }
            Some(SyntaxPatternKind::TupleVariant) => {
                let Some(pattern) = pattern.as_tuple_variant() else {
                    self.finish_pattern(pattern_id, HirPatternKind::Missing);
                    return pattern_id;
                };
                let path = self.record_pattern_path_from_tokens(
                    pattern_id,
                    pattern.path_segments(),
                    pattern_span,
                    pattern.path_tokens(),
                );
                self.bind_pattern_path(&pattern.path_segments());
                let fields = pattern
                    .patterns()
                    .map(|field| self.bind_pattern(&field, span, kind))
                    .collect();
                HirPatternKind::TupleVariant { path, fields }
            }
            Some(SyntaxPatternKind::RecordVariant) => {
                let Some(pattern) = pattern.as_record_variant() else {
                    self.finish_pattern(pattern_id, HirPatternKind::Missing);
                    return pattern_id;
                };
                let path = self.record_pattern_path_from_tokens(
                    pattern_id,
                    pattern.path_segments(),
                    pattern_span,
                    pattern.path_tokens(),
                );
                self.bind_pattern_path(&pattern.path_segments());
                let fields = pattern
                    .fields()
                    .map(|field| self.bind_record_pattern_field(&field, span, kind))
                    .collect();
                HirPatternKind::RecordVariant { path, fields }
            }
            Some(SyntaxPatternKind::Path) => {
                let path_segments = pattern.path_segments();
                let path = self.record_pattern_path_from_tokens(
                    pattern_id,
                    path_segments.clone(),
                    pattern_span,
                    pattern.path_tokens(),
                );
                self.bind_pattern_path(&path_segments);
                HirPatternKind::Path { path }
            }
            Some(SyntaxPatternKind::Wildcard) => HirPatternKind::Wildcard,
            Some(SyntaxPatternKind::Literal) => {
                HirPatternKind::Literal(pattern.literal().map(hir_literal))
            }
            None => HirPatternKind::Missing,
        };
        self.finish_pattern(pattern_id, payload);
        pattern_id
    }

    fn bind_record_pattern_field(
        &mut self,
        field: &SyntaxRecordPatternField,
        span: Span,
        kind: LocalBindingKind,
    ) -> HirRecordPatternField {
        let name_token = field.label_token();
        let name = name_token
            .as_ref()
            .map_or_else(String::new, |token| token.text().to_owned());
        let name_span = name_token.as_ref().map_or_else(
            || span_for(self.source, field.syntax().text_range()),
            |token| span_for(self.source, token.text_range()),
        );
        let shorthand = field.is_shorthand();
        if let Some(pattern) = field.pattern() {
            return HirRecordPatternField {
                name,
                name_origin: HirSourceOrigin {
                    source: self.source,
                    span: name_span,
                },
                pattern: Some(self.bind_pattern(&pattern, span, kind)),
                shorthand,
            };
        } else if let Some(name_token) = field.shorthand_binding_name_token() {
            let pattern_id = self.next_pattern(span_for(self.source, name_token.text_range()));
            let local = self.declare_pattern_local(
                name_token.text().to_owned(),
                kind,
                span_for(self.source, name_token.text_range()),
                span,
            );
            self.finish_pattern(pattern_id, HirPatternKind::Binding { local: Some(local) });
            return HirRecordPatternField {
                name,
                name_origin: HirSourceOrigin {
                    source: self.source,
                    span: name_span,
                },
                pattern: Some(pattern_id),
                shorthand,
            };
        }
        HirRecordPatternField {
            name,
            name_origin: HirSourceOrigin {
                source: self.source,
                span: name_span,
            },
            pattern: None,
            shorthand,
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

    fn record_pattern_path_from_tokens(
        &mut self,
        pattern: HirPatternId,
        path: Vec<String>,
        origin_span: Span,
        tokens: Vec<SyntaxToken>,
    ) -> Option<crate::ids::HirPathId> {
        if path.is_empty() {
            return None;
        }
        let segment_span = last_segment_span(self.source, tokens).unwrap_or(origin_span);
        Some(self.next_path(
            HirPathOwner::Pattern(pattern),
            HirPathKind::Pattern,
            path,
            origin_span,
            segment_span,
        ))
    }
}
