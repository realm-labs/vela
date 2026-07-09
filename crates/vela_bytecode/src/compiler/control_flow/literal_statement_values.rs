use std::collections::BTreeMap;

use vela_common::{PrimitiveTag, SourceId, Span};
use vela_hir::binding::LocalBindingKind;
use vela_syntax::ast::{Literal, SyntaxExpression, SyntaxExpressionKind};

use crate::compiler::const_eval::{
    compile_literal_constant_for_type, compile_negated_literal_constant,
    compile_negated_literal_constant_for_type, evaluate_syntax_const_expr,
};
use crate::compiler::record_shapes::ValueShape;
use crate::compiler::script_types::{ScriptTypeFact, type_hint_script_type};
use crate::compiler::value_types::{
    ExpectedTypeOutcome, RuntimeTypeFact, StaticExprType, TypeContractContext, check_expected_type,
    static_literal_type, type_hint_value_type,
};
use crate::compiler::{CompileResult, Compiler, frame_slot_kind};
use crate::{Constant, UnlinkedInstructionKind};

use super::static_type_runtime_fact;

impl Compiler<'_, '_> {
    pub(super) fn compile_let_literal(
        &mut self,
        name: String,
        span: Span,
        literal: Literal,
        literal_span: Span,
    ) -> CompileResult<bool> {
        let local_binding = self
            .bindings
            .local_named_at(&name, LocalBindingKind::Let, span)
            .and_then(|local| {
                self.bindings
                    .local(local)
                    .map(|binding| (local, binding.type_hint.clone()))
            });
        let hir_type_hint = local_binding.as_ref().and_then(|(_, hint)| hint.as_ref());
        let hinted_script_fact = hir_type_hint.and_then(|hint| {
            let known_type_names = self.facts.known_type_names();
            type_hint_script_type(hint, known_type_names.iter()).map(ScriptTypeFact::new)
        });
        let hinted_value_type = hir_type_hint.and_then(type_hint_value_type);
        let value_type = hinted_value_type
            .clone()
            .or_else(|| static_type_runtime_fact(static_literal_type(&literal)));
        let register = match hinted_value_type.clone() {
            Some(expected @ RuntimeTypeFact::Primitive(tag)) => {
                if let Some(constant) = compile_literal_constant_for_type(&literal, tag)
                    .map_err(|error| error.with_span(literal_span))?
                {
                    self.emit_constant(constant)?
                } else {
                    check_expected_type(
                        static_literal_type(&literal),
                        expected,
                        literal_span,
                        TypeContractContext::TypedLet { name: name.clone() },
                    )?;
                    self.compile_literal(Some(literal_span), &literal)?
                }
            }
            Some(expected) => {
                check_expected_type(
                    static_literal_type(&literal),
                    expected,
                    literal_span,
                    TypeContractContext::TypedLet { name: name.clone() },
                )?;
                self.compile_literal(Some(literal_span), &literal)?
            }
            None => self.compile_literal(Some(literal_span), &literal)?,
        };
        self.locals.insert(name.clone(), register);
        if let Some((local, _)) = local_binding {
            self.hir_locals.insert(local, register);
            self.record_frame_slot(
                name.clone(),
                register,
                frame_slot_kind(LocalBindingKind::Let),
                Some(local),
                Some(span),
            );
            self.script_types
                .set_local_fact(local, name.clone(), hinted_script_fact);
            self.value_types.set_local(local, name.clone(), value_type);
            self.value_shapes.set_local(local, name, None);
        } else {
            self.record_frame_slot(
                name.clone(),
                register,
                frame_slot_kind(LocalBindingKind::Let),
                None,
                Some(span),
            );
            self.script_types
                .set_name_fact(name.clone(), hinted_script_fact);
            self.value_types.set_name(name.clone(), value_type);
            self.value_shapes.set_name(name, None);
        }
        Ok(false)
    }

    pub(super) fn compile_let_syntax_constant(
        &mut self,
        source: SourceId,
        name: String,
        span: Span,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<bool>> {
        if !syntax_constant_fast_path_allowed(expression) {
            return Ok(None);
        }
        let Some(constant) = evaluate_syntax_const_expr(source, expression, &BTreeMap::new())?
        else {
            return Ok(None);
        };
        let local_binding = self
            .bindings
            .local_named_at(&name, LocalBindingKind::Let, span)
            .and_then(|local| {
                self.bindings
                    .local(local)
                    .map(|binding| (local, binding.type_hint.clone()))
            });
        let hir_type_hint = local_binding.as_ref().and_then(|(_, hint)| hint.as_ref());
        let hinted_script_fact = hir_type_hint.and_then(|hint| {
            let known_type_names = self.facts.known_type_names();
            type_hint_script_type(hint, known_type_names.iter()).map(ScriptTypeFact::new)
        });
        let hinted_value_type = hir_type_hint.and_then(type_hint_value_type);
        let guard_expected = if let Some(expected) = hinted_value_type.clone() {
            match check_expected_type(
                static_type_for_constant(&constant),
                expected,
                span,
                TypeContractContext::TypedLet { name: name.clone() },
            )? {
                ExpectedTypeOutcome::RequiresRuntimeGuard(expected) => Some(expected),
                ExpectedTypeOutcome::Proven | ExpectedTypeOutcome::Contextualized(_) => None,
            }
        } else {
            None
        };
        let register = self.emit_constant(constant.clone())?;
        if let Some(expected) = guard_expected.as_ref() {
            self.emit_dynamic_contract_guard(
                register,
                span,
                expected,
                TypeContractContext::TypedLet { name: name.clone() },
            )?;
        }
        let value_shape = self.value_shape_for_syntax_expression(Some(source), expression);
        let value_type = hinted_value_type
            .or_else(|| runtime_type_for_constant(&constant))
            .or_else(|| value_shape.as_ref().and_then(ValueShape::value_type));
        self.locals.insert(name.clone(), register);
        if let Some((local, _)) = local_binding {
            self.hir_locals.insert(local, register);
            self.record_frame_slot(
                name.clone(),
                register,
                frame_slot_kind(LocalBindingKind::Let),
                Some(local),
                Some(span),
            );
            self.script_types
                .set_local_fact(local, name.clone(), hinted_script_fact);
            self.value_types.set_local(local, name.clone(), value_type);
            self.value_shapes.set_local(local, name, value_shape);
        } else {
            self.record_frame_slot(
                name.clone(),
                register,
                frame_slot_kind(LocalBindingKind::Let),
                None,
                Some(span),
            );
            self.script_types
                .set_name_fact(name.clone(), hinted_script_fact);
            self.value_types.set_name(name.clone(), value_type);
            self.value_shapes.set_name(name, value_shape);
        }
        Ok(Some(false))
    }

    pub(super) fn compile_let_negated_literal(
        &mut self,
        name: String,
        span: Span,
        literal: Literal,
        literal_span: Span,
    ) -> CompileResult<bool> {
        let local_binding = self
            .bindings
            .local_named_at(&name, LocalBindingKind::Let, span)
            .and_then(|local| {
                self.bindings
                    .local(local)
                    .map(|binding| (local, binding.type_hint.clone()))
            });
        let hir_type_hint = local_binding.as_ref().and_then(|(_, hint)| hint.as_ref());
        let hinted_script_fact = hir_type_hint.and_then(|hint| {
            let known_type_names = self.facts.known_type_names();
            type_hint_script_type(hint, known_type_names.iter()).map(ScriptTypeFact::new)
        });
        let hinted_value_type = hir_type_hint.and_then(type_hint_value_type);
        let register = match hinted_value_type.clone() {
            Some(expected @ RuntimeTypeFact::Primitive(tag)) => {
                if let Some(constant) = compile_negated_literal_constant_for_type(&literal, tag)
                    .map_err(|error| error.with_span(literal_span))?
                {
                    self.emit_constant(constant)?
                } else {
                    check_negated_literal_expected_type(
                        &literal,
                        expected,
                        literal_span,
                        TypeContractContext::TypedLet { name: name.clone() },
                    )?;
                    self.compile_negated_literal_without_context(&literal, literal_span)?
                }
            }
            Some(expected) => {
                check_negated_literal_expected_type(
                    &literal,
                    expected,
                    literal_span,
                    TypeContractContext::TypedLet { name: name.clone() },
                )?;
                self.compile_negated_literal_without_context(&literal, literal_span)?
            }
            None => self.compile_negated_literal_without_context(&literal, literal_span)?,
        };
        let value_type =
            hinted_value_type.or_else(|| static_type_runtime_fact(static_literal_type(&literal)));
        self.locals.insert(name.clone(), register);
        if let Some((local, _)) = local_binding {
            self.hir_locals.insert(local, register);
            self.record_frame_slot(
                name.clone(),
                register,
                frame_slot_kind(LocalBindingKind::Let),
                Some(local),
                Some(span),
            );
            self.script_types
                .set_local_fact(local, name.clone(), hinted_script_fact);
            self.value_types.set_local(local, name.clone(), value_type);
            self.value_shapes.set_local(local, name, None);
        } else {
            self.record_frame_slot(
                name.clone(),
                register,
                frame_slot_kind(LocalBindingKind::Let),
                None,
                Some(span),
            );
            self.script_types
                .set_name_fact(name.clone(), hinted_script_fact);
            self.value_types.set_name(name.clone(), value_type);
            self.value_shapes.set_name(name, None);
        }
        Ok(false)
    }

    pub(super) fn compile_return_literal(
        &mut self,
        literal: Literal,
        span: Span,
    ) -> CompileResult<bool> {
        let register = match self.return_type.clone() {
            Some(expected @ RuntimeTypeFact::Primitive(tag)) => {
                if let Some(constant) = compile_literal_constant_for_type(&literal, tag)
                    .map_err(|error| error.with_span(span))?
                {
                    self.emit_constant(constant)?
                } else {
                    check_expected_type(
                        static_literal_type(&literal),
                        expected,
                        span,
                        TypeContractContext::Return,
                    )?;
                    self.compile_literal(Some(span), &literal)?
                }
            }
            Some(expected) => {
                check_expected_type(
                    static_literal_type(&literal),
                    expected,
                    span,
                    TypeContractContext::Return,
                )?;
                self.compile_literal(Some(span), &literal)?
            }
            None => self.compile_literal(Some(span), &literal)?,
        };
        self.emit(UnlinkedInstructionKind::Return { src: register });
        Ok(true)
    }

    pub(super) fn compile_return_negated_literal(
        &mut self,
        literal: Literal,
        span: Span,
    ) -> CompileResult<bool> {
        let register = match self.return_type.clone() {
            Some(expected @ RuntimeTypeFact::Primitive(tag)) => {
                if let Some(constant) = compile_negated_literal_constant_for_type(&literal, tag)
                    .map_err(|error| error.with_span(span))?
                {
                    self.emit_constant(constant)?
                } else {
                    check_negated_literal_expected_type(
                        &literal,
                        expected,
                        span,
                        TypeContractContext::Return,
                    )?;
                    self.compile_negated_literal_without_context(&literal, span)?
                }
            }
            Some(expected) => {
                check_negated_literal_expected_type(
                    &literal,
                    expected,
                    span,
                    TypeContractContext::Return,
                )?;
                self.compile_negated_literal_without_context(&literal, span)?
            }
            None => self.compile_negated_literal_without_context(&literal, span)?,
        };
        self.emit(UnlinkedInstructionKind::Return { src: register });
        Ok(true)
    }

    pub(super) fn compile_return_syntax_constant(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        span: Span,
    ) -> CompileResult<Option<bool>> {
        if !syntax_constant_fast_path_allowed(expression) {
            return Ok(None);
        }
        let Some(constant) = evaluate_syntax_const_expr(source, expression, &BTreeMap::new())?
        else {
            return Ok(None);
        };
        let guard_expected = if let Some(expected) = self.return_type.clone() {
            match check_expected_type(
                static_type_for_constant(&constant),
                expected,
                span,
                TypeContractContext::Return,
            )? {
                ExpectedTypeOutcome::RequiresRuntimeGuard(expected) => Some(expected),
                ExpectedTypeOutcome::Proven | ExpectedTypeOutcome::Contextualized(_) => None,
            }
        } else {
            None
        };
        let register = self.emit_constant(constant)?;
        if let Some(expected) = guard_expected.as_ref() {
            self.emit_dynamic_contract_guard(
                register,
                span,
                expected,
                TypeContractContext::Return,
            )?;
        }
        self.emit(UnlinkedInstructionKind::Return { src: register });
        Ok(Some(true))
    }

    pub(super) fn compile_syntax_constant_expr_statement(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<bool>> {
        if !syntax_constant_fast_path_allowed(expression) {
            return Ok(None);
        }
        let Some(constant) = evaluate_syntax_const_expr(source, expression, &BTreeMap::new())?
        else {
            return Ok(None);
        };
        self.emit_constant(constant)?;
        Ok(Some(false))
    }

    pub(super) fn compile_syntax_constant_expr_to(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        dst: crate::Register,
    ) -> CompileResult<Option<bool>> {
        if !syntax_constant_fast_path_allowed(expression) {
            return Ok(None);
        }
        let Some(constant) = evaluate_syntax_const_expr(source, expression, &BTreeMap::new())?
        else {
            return Ok(None);
        };
        self.emit_constant_to(dst, constant);
        Ok(Some(false))
    }

    fn compile_negated_literal_without_context(
        &mut self,
        literal: &Literal,
        span: Span,
    ) -> CompileResult<crate::Register> {
        let Some(constant) =
            compile_negated_literal_constant(literal).map_err(|error| error.with_span(span))?
        else {
            return self.compile_literal(Some(span), literal);
        };
        self.emit_constant(constant)
    }
}

fn runtime_type_for_constant(value: &Constant) -> Option<RuntimeTypeFact> {
    match value {
        Constant::Unit => Some(RuntimeTypeFact::primitive(PrimitiveTag::Unit)),
        Constant::Bool(_) => Some(RuntimeTypeFact::primitive(PrimitiveTag::Bool)),
        Constant::Char(_) => Some(RuntimeTypeFact::primitive(PrimitiveTag::Char)),
        Constant::Scalar(value) => Some(RuntimeTypeFact::primitive(value.primitive_tag())),
        Constant::String(_) => Some(RuntimeTypeFact::primitive(PrimitiveTag::String)),
        Constant::Bytes(_) => Some(RuntimeTypeFact::primitive(PrimitiveTag::Bytes)),
        Constant::Array(_) | Constant::Map(_) => None,
    }
}

fn static_type_for_constant(value: &Constant) -> StaticExprType {
    runtime_type_for_constant(value).map_or(StaticExprType::Dynamic, StaticExprType::Exact)
}

fn syntax_constant_fast_path_allowed(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_constant_fast_path_allowed(&inner);
    }
    !matches!(
        expression.expression_kind(),
        SyntaxExpressionKind::Block | SyntaxExpressionKind::If | SyntaxExpressionKind::Match
    )
}

fn check_negated_literal_expected_type(
    literal: &Literal,
    expected: RuntimeTypeFact,
    span: Span,
    context: TypeContractContext,
) -> CompileResult<()> {
    let actual = if expected_primitive_unsigned_integer(&expected)
        && matches!(literal, Literal::Integer(value) if value.suffix.is_none())
    {
        StaticExprType::Exact(RuntimeTypeFact::primitive(PrimitiveTag::I64))
    } else {
        static_literal_type(literal)
    };
    check_expected_type(actual, expected, span, context).map(|_| ())
}

fn expected_primitive_unsigned_integer(expected: &RuntimeTypeFact) -> bool {
    matches!(
        expected,
        RuntimeTypeFact::Primitive(
            PrimitiveTag::U8 | PrimitiveTag::U16 | PrimitiveTag::U32 | PrimitiveTag::U64
        )
    )
}
