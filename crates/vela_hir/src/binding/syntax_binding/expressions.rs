use vela_syntax::ast::{
    AstNode, SyntaxArgument, SyntaxElseBranch, SyntaxExpression, SyntaxExpressionKind,
    SyntaxInterpolatedStringPart, SyntaxMapEntry, SyntaxRecordExprField,
};

use crate::binding::{LocalBindingKind, PathUsage};
use crate::body::{
    HirArgument, HirBodyOwner, HirBodyRoot, HirCall, HirElseBranch, HirExprKind, HirField, HirIf,
    HirIndex, HirLiteral, HirMapEntry, HirMatch, HirMatchArmBody, HirPathKind, HirPathOwner,
    HirRecordField, HirScopeKind, HirSourceOrigin,
};
use crate::ids::HirExprId;

use super::{
    SyntaxBindingLowerer, hir_assign_op, hir_binary_op, hir_literal, hir_path_kind_for_usage,
    hir_type_hint, hir_unary_op, last_segment_span, span_for,
};

impl SyntaxBindingLowerer<'_> {
    pub(super) fn bind_expr(&mut self, expr: &SyntaxExpression, usage: PathUsage) -> HirExprId {
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
            SyntaxExpressionKind::Await => {
                let expression = expr
                    .as_await()
                    .and_then(|expr| expr.expression())
                    .map(|expr| self.bind_expr(&expr, PathUsage::Value));
                HirExprKind::Await { expression }
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
        let logical_key = key.and_then(|key| self.logical_map_key(key));
        let value = entry
            .value()
            .map(|value| self.bind_expr(&value, PathUsage::Value));
        HirMapEntry {
            key,
            logical_key,
            value,
            origin: HirSourceOrigin {
                source: self.source,
                span: span_for(self.source, entry.syntax().text_range()),
            },
        }
    }

    fn logical_map_key(&self, expression: HirExprId) -> Option<String> {
        let body = self.bodies.get(&self.current_body())?;
        match &body.expression(expression)?.kind {
            HirExprKind::Literal(HirLiteral::String(value)) => Some(value.clone()),
            HirExprKind::Literal(HirLiteral::Char(value)) => Some(value.to_string()),
            HirExprKind::Literal(HirLiteral::Integer(value)) => Some(value.source_spelling()),
            HirExprKind::Literal(HirLiteral::Float(value)) => Some(value.source_spelling()),
            HirExprKind::Path(path) => body
                .paths
                .get(path)
                .filter(|path| path.kind == HirPathKind::Value)
                .map(|path| path.path.join("::"))
                .filter(|path| !path.is_empty()),
            HirExprKind::Literal(
                HirLiteral::Bool(_)
                | HirLiteral::Bytes(_)
                | HirLiteral::Interpolated { .. }
                | HirLiteral::Invalid { .. },
            )
            | HirExprKind::Paren { .. }
            | HirExprKind::Unit
            | HirExprKind::Tuple { .. }
            | HirExprKind::Unary { .. }
            | HirExprKind::Binary { .. }
            | HirExprKind::Assign { .. }
            | HirExprKind::Field(_)
            | HirExprKind::Call(_)
            | HirExprKind::Index(_)
            | HirExprKind::Try { .. }
            | HirExprKind::Await { .. }
            | HirExprKind::Array { .. }
            | HirExprKind::Map { .. }
            | HirExprKind::Record { .. }
            | HirExprKind::Lambda { .. }
            | HirExprKind::Block { .. }
            | HirExprKind::If(_)
            | HirExprKind::Match(_)
            | HirExprKind::Missing => None,
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
        let source_text = literal
            .token_text()
            .unwrap_or_else(|| literal.syntax().text().to_string());
        let Some(syntax_parts) = literal.interpolated_string_parts() else {
            return HirLiteral::Invalid { source_text };
        };
        let mut parts = Vec::with_capacity(syntax_parts.len());
        let mut has_expression = false;
        for part in syntax_parts {
            match part {
                SyntaxInterpolatedStringPart::Text(text) => {
                    parts.push(crate::body::HirInterpolatedStringPart::Text(text));
                }
                SyntaxInterpolatedStringPart::Expression(expression) => {
                    parts.push(crate::body::HirInterpolatedStringPart::Expr(
                        self.bind_expr(&expression, PathUsage::Value),
                    ));
                    has_expression = true;
                }
            }
        }
        if !has_expression {
            return HirLiteral::Invalid { source_text };
        }
        HirLiteral::Interpolated { parts }
    }

    pub(super) fn bind_if(&mut self, if_expr: &vela_syntax::ast::SyntaxIfExpr) -> HirIf {
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

    pub(super) fn bind_match(
        &mut self,
        match_expr: &vela_syntax::ast::SyntaxMatchExpr,
    ) -> HirMatch {
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
}
