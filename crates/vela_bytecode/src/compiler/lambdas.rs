use std::collections::{BTreeMap, HashMap};

use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingMap, BindingResolution};
use vela_hir::ids::HirLocalId;
use vela_syntax::ast::{
    AstNode, Param, SyntaxBlock, SyntaxElseBranch, SyntaxExpression, SyntaxExpressionKind,
    SyntaxLambdaBody, SyntaxStatementKind,
};

use crate::{Register, UnlinkedCodeObject, UnlinkedInstructionKind};

use super::body_payloads::CompilerBodyPayload;
use super::record_shapes::ValueShape;
use super::{CompileError, CompileErrorKind, CompileResult, Compiler};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LambdaCapture {
    pub local: HirLocalId,
    pub name: String,
    pub register: Register,
}

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn compile_syntax_lambda_with_callback_shapes(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        callback_shapes: &[Option<ValueShape>],
    ) -> CompileResult<Option<Register>> {
        let Some(lambda) = expression.as_lambda() else {
            return Ok(None);
        };
        let Some(param_list) = lambda.param_list() else {
            return Ok(None);
        };
        let Some(body) = lambda.body() else {
            return Ok(None);
        };
        let params = param_list
            .params()
            .map(|param| {
                let name = param.name_text().ok_or_else(|| {
                    CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST lambda parameter name",
                    ))
                })?;
                Ok(Param {
                    name,
                    span: syntax_param_span(source, &param),
                    type_hint: None,
                    default_value: None,
                })
            })
            .collect::<CompileResult<Vec<_>>>()?;
        let mut captures = BTreeMap::new();
        match &body {
            SyntaxLambdaBody::Expression(body) => {
                collect_syntax_expr(self.bindings, &self.hir_locals, source, body, &mut captures);
            }
            SyntaxLambdaBody::Block(block) => {
                collect_syntax_block(
                    self.bindings,
                    &self.hir_locals,
                    source,
                    block,
                    &mut captures,
                );
            }
        }
        let captures = captures.into_values().collect::<Vec<_>>();
        let capture_registers = captures
            .iter()
            .map(|capture| capture.register)
            .collect::<Vec<_>>();
        let lambda_span = syntax_expr_span(source, expression);
        let mut lambda_compiler = Compiler::new_lambda(
            format!("{}::<lambda@{}>", self.code.name, lambda_span.start),
            lambda_span,
            &params,
            self.body.clone(),
            &captures,
            self.bindings,
            self.facts.clone(),
        )?;
        for capture in &captures {
            if let Some(script_fact) = self.script_types.local_fact(capture.local) {
                lambda_compiler.script_types.set_local_fact(
                    capture.local,
                    &capture.name,
                    Some(script_fact),
                );
            }
            if let Some(value_type) = self.value_types.local(capture.local) {
                lambda_compiler.value_types.set_local(
                    capture.local,
                    &capture.name,
                    Some(value_type),
                );
            }
            if let Some(value_shape) = self.value_shapes.local(capture.local) {
                lambda_compiler.value_shapes.set_local(
                    capture.local,
                    &capture.name,
                    Some(value_shape),
                );
            }
        }
        for (index, shape) in callback_shapes.iter().enumerate() {
            let Some(shape) = shape else {
                continue;
            };
            let Some(param) = params.get(index) else {
                continue;
            };
            if let Some(local) = self.bindings.local_named_at(
                &param.name,
                vela_hir::binding::LocalBindingKind::LambdaParameter,
                param.span,
            ) {
                lambda_compiler
                    .value_types
                    .set_local(local, &param.name, shape.value_type());
                lambda_compiler
                    .value_shapes
                    .set_local(local, &param.name, Some(shape.clone()));
            } else {
                lambda_compiler
                    .value_types
                    .set_name(&param.name, shape.value_type());
                lambda_compiler
                    .value_shapes
                    .set_name(&param.name, Some(shape.clone()));
            }
        }
        let code = lambda_compiler.compile_syntax_lambda_body(source, body)?;
        let function = self.code.push_nested_function(code);
        let dst = self.alloc_register()?;
        self.emit(UnlinkedInstructionKind::MakeClosure {
            dst,
            function,
            captures: capture_registers,
        });
        Ok(Some(dst))
    }

    fn compile_syntax_lambda_body(
        mut self,
        source: SourceId,
        body: SyntaxLambdaBody,
    ) -> CompileResult<UnlinkedCodeObject> {
        self.compile_param_defaults()?;
        match body {
            SyntaxLambdaBody::Expression(expression) => {
                let value = self
                    .compile_syntax_expression(source, &expression)?
                    .ok_or_else(|| {
                        CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "unsupported CST lambda expression body",
                        ))
                        .with_span(syntax_expr_span(source, &expression))
                    })?;
                self.emit(UnlinkedInstructionKind::Return { src: value });
            }
            SyntaxLambdaBody::Block(block) => {
                let dst = self.alloc_register()?;
                let body = CompilerBodyPayload::nested_syntax(source, block);
                let returned = self.compile_block_payload_value_to(&body, dst)?;
                if !returned {
                    self.emit(UnlinkedInstructionKind::Return { src: dst });
                }
            }
        }
        self.code.register_count = self.next_register;
        Ok(self.code)
    }
}

fn collect_syntax_expr(
    bindings: &BindingMap,
    available: &HashMap<HirLocalId, Register>,
    source: SourceId,
    expr: &SyntaxExpression,
    captures: &mut BTreeMap<HirLocalId, LambdaCapture>,
) {
    match expr.expression_kind() {
        SyntaxExpressionKind::Path => {
            collect_syntax_path(bindings, available, source, expr, captures)
        }
        SyntaxExpressionKind::Paren => {
            if let Some(paren) = expr.as_paren()
                && let Some(inner) = paren.expression()
            {
                collect_syntax_expr(bindings, available, source, &inner, captures);
            }
        }
        SyntaxExpressionKind::Unary => {
            if let Some(unary) = expr.as_unary()
                && let Some(operand) = unary.expression()
            {
                collect_syntax_expr(bindings, available, source, &operand, captures);
            }
        }
        SyntaxExpressionKind::Binary => {
            if let Some(binary) = expr.as_binary() {
                if let Some(left) = binary.lhs() {
                    collect_syntax_expr(bindings, available, source, &left, captures);
                }
                if let Some(right) = binary.rhs() {
                    collect_syntax_expr(bindings, available, source, &right, captures);
                }
            }
        }
        SyntaxExpressionKind::Assign => {
            if let Some(assign) = expr.as_assign() {
                if let Some(target) = assign.target() {
                    collect_syntax_expr(bindings, available, source, &target, captures);
                }
                if let Some(value) = assign.value() {
                    collect_syntax_expr(bindings, available, source, &value, captures);
                }
            }
        }
        SyntaxExpressionKind::Field => {
            if let Some(field) = expr.as_field()
                && let Some(receiver) = field.receiver()
            {
                collect_syntax_expr(bindings, available, source, &receiver, captures);
            }
        }
        SyntaxExpressionKind::Call => {
            if let Some(call) = expr.as_call() {
                if let Some(callee) = call.callee() {
                    collect_syntax_expr(bindings, available, source, &callee, captures);
                }
                for argument in call.arguments() {
                    if let Some(value) = argument.expression() {
                        collect_syntax_expr(bindings, available, source, &value, captures);
                    }
                }
            }
        }
        SyntaxExpressionKind::Index => {
            if let Some(index) = expr.as_index() {
                if let Some(receiver) = index.receiver() {
                    collect_syntax_expr(bindings, available, source, &receiver, captures);
                }
                if let Some(value) = index.index() {
                    collect_syntax_expr(bindings, available, source, &value, captures);
                }
            }
        }
        SyntaxExpressionKind::Try => {
            if let Some(try_expr) = expr.as_try()
                && let Some(operand) = try_expr.expression()
            {
                collect_syntax_expr(bindings, available, source, &operand, captures);
            }
        }
        SyntaxExpressionKind::Array => {
            if let Some(array) = expr.as_array() {
                for item in array.expressions() {
                    collect_syntax_expr(bindings, available, source, &item, captures);
                }
            }
        }
        SyntaxExpressionKind::Map => {
            if let Some(map) = expr.as_map() {
                for entry in map.entries() {
                    if let Some(key) = entry.key() {
                        collect_syntax_expr(bindings, available, source, &key, captures);
                    }
                    if let Some(value) = entry.value() {
                        collect_syntax_expr(bindings, available, source, &value, captures);
                    }
                }
            }
        }
        SyntaxExpressionKind::Record => {
            if let Some(record) = expr.as_record() {
                for field in record.fields() {
                    if let Some(value) = field.expression() {
                        collect_syntax_expr(bindings, available, source, &value, captures);
                    }
                }
            }
        }
        SyntaxExpressionKind::Lambda => {
            if let Some(lambda) = expr.as_lambda()
                && let Some(body) = lambda.body()
            {
                match body {
                    vela_syntax::ast::SyntaxLambdaBody::Expression(body) => {
                        collect_syntax_expr(bindings, available, source, &body, captures);
                    }
                    vela_syntax::ast::SyntaxLambdaBody::Block(block) => {
                        collect_syntax_block(bindings, available, source, &block, captures);
                    }
                }
            }
        }
        SyntaxExpressionKind::Block => {
            if let Some(block) = expr.as_block() {
                collect_syntax_block(bindings, available, source, &block, captures);
            }
        }
        SyntaxExpressionKind::If => {
            if let Some(if_expr) = expr.as_if() {
                if let Some(condition) = if_expr.condition() {
                    collect_syntax_expr(bindings, available, source, &condition, captures);
                }
                if let Some(then_block) = if_expr.then_block() {
                    collect_syntax_block(bindings, available, source, &then_block, captures);
                }
                match if_expr.else_branch() {
                    Some(SyntaxElseBranch::If(else_if)) => {
                        if let Some(else_if_expr) = SyntaxExpression::cast(else_if.syntax().clone())
                        {
                            collect_syntax_expr(
                                bindings,
                                available,
                                source,
                                &else_if_expr,
                                captures,
                            );
                        }
                    }
                    Some(SyntaxElseBranch::Block(block)) => {
                        collect_syntax_block(bindings, available, source, &block, captures);
                    }
                    None => {}
                }
            }
        }
        SyntaxExpressionKind::Match => {
            if let Some(match_expr) = expr.as_match() {
                if let Some(scrutinee) = match_expr.scrutinee() {
                    collect_syntax_expr(bindings, available, source, &scrutinee, captures);
                }
                for arm in match_expr.arms() {
                    if let Some(guard) = arm.guard() {
                        collect_syntax_expr(bindings, available, source, &guard, captures);
                    }
                    if let Some(body) = arm.body_as_expression() {
                        collect_syntax_expr(bindings, available, source, &body, captures);
                    }
                }
            }
        }
        SyntaxExpressionKind::Literal => {}
    }
}

fn collect_syntax_path(
    bindings: &BindingMap,
    available: &HashMap<HirLocalId, Register>,
    source: SourceId,
    expr: &SyntaxExpression,
    captures: &mut BTreeMap<HirLocalId, LambdaCapture>,
) {
    let span = syntax_expr_span(source, expr);
    let Some(BindingResolution::Local(local)) = bindings.resolution_at_span(span) else {
        return;
    };
    let Some(register) = available.get(local).copied() else {
        return;
    };
    let Some(name) = expr
        .as_path()
        .and_then(|path| path.path_segments().into_iter().next())
    else {
        return;
    };

    captures.entry(*local).or_insert_with(|| LambdaCapture {
        local: *local,
        name,
        register,
    });
}

fn collect_syntax_block(
    bindings: &BindingMap,
    available: &HashMap<HirLocalId, Register>,
    source: SourceId,
    block: &SyntaxBlock,
    captures: &mut BTreeMap<HirLocalId, LambdaCapture>,
) {
    for statement in block.statements() {
        match statement.statement_kind() {
            SyntaxStatementKind::Let => {
                if let Some(value) = statement.as_let().and_then(|stmt| stmt.initializer()) {
                    collect_syntax_expr(bindings, available, source, &value, captures);
                }
            }
            SyntaxStatementKind::Return => {
                if let Some(value) = statement.as_return().and_then(|stmt| stmt.expression()) {
                    collect_syntax_expr(bindings, available, source, &value, captures);
                }
            }
            SyntaxStatementKind::For => {
                if let Some(for_stmt) = statement.as_for() {
                    if let Some(iterable) = for_stmt.iterable() {
                        collect_syntax_expr(bindings, available, source, &iterable, captures);
                    }
                    if let Some(body) = for_stmt.body() {
                        collect_syntax_block(bindings, available, source, &body, captures);
                    }
                }
            }
            SyntaxStatementKind::If | SyntaxStatementKind::Match => {
                if let Some(expression) = SyntaxExpression::cast(statement.syntax().clone()) {
                    collect_syntax_expr(bindings, available, source, &expression, captures);
                }
            }
            SyntaxStatementKind::Block => {
                if let Some(block) = statement.as_block() {
                    collect_syntax_block(bindings, available, source, &block, captures);
                }
            }
            SyntaxStatementKind::Expr => {
                if let Some(expr_stmt) = statement.as_expr()
                    && let Some(expression) = expr_stmt.expression()
                {
                    collect_syntax_expr(bindings, available, source, &expression, captures);
                }
            }
            SyntaxStatementKind::Break | SyntaxStatementKind::Continue => {}
        }
    }
}

fn syntax_expr_span(source: SourceId, expression: &SyntaxExpression) -> Span {
    let range = expression.syntax().text_range();
    Span::new(source, range.start().into(), range.end().into())
}

fn syntax_param_span(source: SourceId, param: &vela_syntax::ast::SyntaxParam) -> Span {
    let range = param
        .name_token()
        .map_or_else(|| param.syntax().text_range(), |token| token.text_range());
    Span::new(source, range.start().into(), range.end().into())
}
