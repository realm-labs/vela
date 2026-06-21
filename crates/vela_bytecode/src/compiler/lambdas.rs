use std::collections::{BTreeMap, HashMap};

use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingMap, BindingResolution};
use vela_hir::ids::HirLocalId;
use vela_syntax::ast::{
    Argument, AstNode, Block, ElseBranch, Expr, ExprKind, IfExpr, MapEntry, MatchArm, MatchExpr,
    Param, RecordField, Stmt, StmtKind, SyntaxBlock, SyntaxElseBranch, SyntaxExpression,
    SyntaxExpressionKind, SyntaxStatementKind,
};

use crate::{Register, UnlinkedCodeObject, UnlinkedInstructionKind};

use super::body_payloads::CompilerExpressionPayload;
use super::record_shapes::ValueShape;
use super::{CompileResult, Compiler};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LambdaCapture {
    pub local: HirLocalId,
    pub name: String,
    pub register: Register,
}

pub(crate) fn collect_lambda_captures(
    bindings: &BindingMap,
    available: &HashMap<HirLocalId, Register>,
    body: &Expr,
) -> Vec<LambdaCapture> {
    let mut captures = BTreeMap::new();
    collect_expr(bindings, available, body, &mut captures);
    captures.into_values().collect()
}

pub(in crate::compiler) fn collect_lambda_captures_with_payload(
    bindings: &BindingMap,
    available: &HashMap<HirLocalId, Register>,
    body: &Expr,
    body_payload: Option<&CompilerExpressionPayload<'_>>,
) -> Vec<LambdaCapture> {
    let Some(payload) = body_payload else {
        return collect_lambda_captures(bindings, available, body);
    };
    let (Some(source), Some(syntax)) = (payload.source(), payload.syntax_expression()) else {
        return collect_lambda_captures(bindings, available, body);
    };

    let mut captures = BTreeMap::new();
    collect_syntax_expr(bindings, available, source, syntax, &mut captures);
    captures.into_values().collect()
}

impl Compiler<'_, '_> {
    pub(super) fn compile_lambda(
        &mut self,
        lambda: &Expr,
        params: &[Param],
        body: &Expr,
        body_payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<Register> {
        self.compile_lambda_with_callback_shapes(lambda, params, body, body_payload, &[])
    }

    pub(super) fn compile_lambda_with_callback_shapes(
        &mut self,
        lambda: &Expr,
        params: &[Param],
        body: &Expr,
        body_payload: Option<&CompilerExpressionPayload<'_>>,
        callback_shapes: &[Option<ValueShape>],
    ) -> CompileResult<Register> {
        let captures = collect_lambda_captures_with_payload(
            self.bindings,
            &self.hir_locals,
            body,
            body_payload,
        );
        let capture_registers = captures
            .iter()
            .map(|capture| capture.register)
            .collect::<Vec<_>>();
        let mut lambda_compiler = Compiler::new_lambda(
            format!("{}::<lambda@{}>", self.code.name, lambda.span.start),
            lambda.span,
            params,
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
        let code = lambda_compiler.compile_lambda_body(body, body_payload)?;
        let function = self.code.push_nested_function(code);
        let dst = self.alloc_register()?;
        self.emit(UnlinkedInstructionKind::MakeClosure {
            dst,
            function,
            captures: capture_registers,
        });
        Ok(dst)
    }

    fn compile_lambda_body(
        mut self,
        body: &Expr,
        body_payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<UnlinkedCodeObject> {
        self.compile_param_defaults()?;
        match &body.kind {
            ExprKind::Block(block) => {
                let dst = self.alloc_register()?;
                let returned = if let Some(block_payload) =
                    body_payload.and_then(CompilerExpressionPayload::block_body_payload)
                {
                    self.compile_block_payload_value_to(&block_payload, dst)?
                } else {
                    self.compile_block_value_to(block, dst)?
                };
                if !returned {
                    self.emit(UnlinkedInstructionKind::Return { src: dst });
                }
            }
            _ => {
                let value = self.compile_expr_with_payload(body, body_payload)?;
                self.emit(UnlinkedInstructionKind::Return { src: value });
            }
        }
        self.code.register_count = self.next_register;
        Ok(self.code)
    }
}

fn collect_expr(
    bindings: &BindingMap,
    available: &HashMap<HirLocalId, Register>,
    expr: &Expr,
    captures: &mut BTreeMap<HirLocalId, LambdaCapture>,
) {
    match &expr.kind {
        ExprKind::Path(path) => {
            let Some(BindingResolution::Local(local)) = bindings.resolution_at_span(expr.span)
            else {
                return;
            };
            let Some(register) = available.get(local).copied() else {
                return;
            };
            let Some(name) = path.first() else {
                return;
            };
            captures.entry(*local).or_insert_with(|| LambdaCapture {
                local: *local,
                name: name.clone(),
                register,
            });
        }
        ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => {
            collect_expr(bindings, available, expr, captures);
        }
        ExprKind::Binary { left, right, .. } => {
            collect_expr(bindings, available, left, captures);
            collect_expr(bindings, available, right, captures);
        }
        ExprKind::Assign { target, value, .. } => {
            collect_expr(bindings, available, target, captures);
            collect_expr(bindings, available, value, captures);
        }
        ExprKind::Field { base, .. } => collect_expr(bindings, available, base, captures),
        ExprKind::Call { callee, args } => {
            collect_expr(bindings, available, callee, captures);
            for arg in args {
                collect_argument(bindings, available, arg, captures);
            }
        }
        ExprKind::Index { base, index } => {
            collect_expr(bindings, available, base, captures);
            collect_expr(bindings, available, index, captures);
        }
        ExprKind::Array(items) => {
            for item in items {
                collect_expr(bindings, available, item, captures);
            }
        }
        ExprKind::Map(entries) => {
            for entry in entries {
                collect_map_entry(bindings, available, entry, captures);
            }
        }
        ExprKind::Record { fields, .. } => {
            for field in fields {
                collect_record_field(bindings, available, field, captures);
            }
        }
        ExprKind::InterpolatedString(parts) => {
            for part in parts {
                if let vela_syntax::ast::InterpolatedStringPart::Expr(expr) = part {
                    collect_expr(bindings, available, expr, captures);
                }
            }
        }
        ExprKind::If(if_expr) => collect_if(bindings, available, if_expr, captures),
        ExprKind::Match(match_expr) => collect_match(bindings, available, match_expr, captures),
        ExprKind::Block(block) => collect_block(bindings, available, block, captures),
        ExprKind::Lambda { body, .. } => collect_expr(bindings, available, body, captures),
        ExprKind::Literal(_) | ExprKind::SelfValue | ExprKind::Error => {}
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

fn collect_argument(
    bindings: &BindingMap,
    available: &HashMap<HirLocalId, Register>,
    argument: &Argument,
    captures: &mut BTreeMap<HirLocalId, LambdaCapture>,
) {
    collect_expr(bindings, available, &argument.value, captures);
}

fn collect_map_entry(
    bindings: &BindingMap,
    available: &HashMap<HirLocalId, Register>,
    entry: &MapEntry,
    captures: &mut BTreeMap<HirLocalId, LambdaCapture>,
) {
    collect_expr(bindings, available, &entry.key, captures);
    collect_expr(bindings, available, &entry.value, captures);
}

fn collect_record_field(
    bindings: &BindingMap,
    available: &HashMap<HirLocalId, Register>,
    field: &RecordField,
    captures: &mut BTreeMap<HirLocalId, LambdaCapture>,
) {
    if let Some(value) = &field.value {
        collect_expr(bindings, available, value, captures);
    }
}

fn collect_if(
    bindings: &BindingMap,
    available: &HashMap<HirLocalId, Register>,
    if_expr: &IfExpr,
    captures: &mut BTreeMap<HirLocalId, LambdaCapture>,
) {
    collect_expr(bindings, available, &if_expr.condition, captures);
    collect_block(bindings, available, &if_expr.then_branch, captures);
    match &if_expr.else_branch {
        Some(ElseBranch::If(if_expr)) => collect_if(bindings, available, if_expr, captures),
        Some(ElseBranch::Block(block)) => collect_block(bindings, available, block, captures),
        None => {}
    }
}

fn collect_match(
    bindings: &BindingMap,
    available: &HashMap<HirLocalId, Register>,
    match_expr: &MatchExpr,
    captures: &mut BTreeMap<HirLocalId, LambdaCapture>,
) {
    collect_expr(bindings, available, &match_expr.scrutinee, captures);
    for arm in &match_expr.arms {
        collect_match_arm(bindings, available, arm, captures);
    }
}

fn collect_match_arm(
    bindings: &BindingMap,
    available: &HashMap<HirLocalId, Register>,
    arm: &MatchArm,
    captures: &mut BTreeMap<HirLocalId, LambdaCapture>,
) {
    if let Some(guard) = &arm.guard {
        collect_expr(bindings, available, guard, captures);
    }
    collect_expr(bindings, available, &arm.body, captures);
}

fn collect_block(
    bindings: &BindingMap,
    available: &HashMap<HirLocalId, Register>,
    block: &Block,
    captures: &mut BTreeMap<HirLocalId, LambdaCapture>,
) {
    for statement in &block.statements {
        collect_statement(bindings, available, statement, captures);
    }
}

fn collect_statement(
    bindings: &BindingMap,
    available: &HashMap<HirLocalId, Register>,
    statement: &Stmt,
    captures: &mut BTreeMap<HirLocalId, LambdaCapture>,
) {
    match &statement.kind {
        StmtKind::Let { value, .. } | StmtKind::Return(value) => {
            if let Some(value) = value {
                collect_expr(bindings, available, value, captures);
            }
        }
        StmtKind::For { iterable, body, .. } => {
            collect_expr(bindings, available, iterable, captures);
            collect_block(bindings, available, body, captures);
        }
        StmtKind::Expr(expr) => collect_expr(bindings, available, expr, captures),
        StmtKind::Block(block) => collect_block(bindings, available, block, captures),
        StmtKind::Break | StmtKind::Continue => {}
    }
}
