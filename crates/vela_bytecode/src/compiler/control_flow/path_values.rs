use vela_common::Span;
use vela_hir::binding::LocalBindingKind;
use vela_syntax::ast::{AstNode, SyntaxExpression};

use crate::{
    GuardKind, Register, UnlinkedGuardContext, UnlinkedInstructionKind, UnlinkedTypeGuard,
};

use crate::compiler::body_payloads::expression_syntax_path_or_self;
use crate::compiler::expected_exprs::guard_location_and_name;
use crate::compiler::record_shapes::ValueShape;
use crate::compiler::value_types::ExpectedTypeOutcome;
use crate::compiler::{type_guard_for_hint, type_guard_plan_for_runtime_type};

use super::classification::{is_map_or_set_type_hint, merge_type_hint_and_value_fact};
use super::{
    CompileError, CompileErrorKind, CompileResult, Compiler, RuntimeTypeFact, ScriptTypeFact,
    StaticExprType, TypeContractContext, check_expected_type, frame_slot_kind,
    type_hint_script_type, type_hint_value_type,
};

impl Compiler<'_, '_> {
    pub(super) fn compile_let_path(
        &mut self,
        name: String,
        span: Span,
        path: Vec<String>,
        path_span: Span,
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
        let value_script_fact = self.script_fact_for_path(path_span, &path);
        let script_hint_proven = hinted_script_fact
            .as_ref()
            .zip(value_script_fact.as_ref())
            .is_some_and(|(hint, value)| hint == value);
        let script_fact = merge_type_hint_and_value_fact(hinted_script_fact, value_script_fact);
        let hinted_value_type = hir_type_hint.and_then(type_hint_value_type);
        let value_type = hinted_value_type
            .clone()
            .or_else(|| self.value_type_for_path(path_span, &path));
        let value_shape = self.value_shape_for_path(path_span, &path);
        let register = self.compile_path_with_expected_type(
            path_span,
            &path,
            hinted_value_type.clone(),
            TypeContractContext::TypedLet { name: name.clone() },
        )?;
        if let (Some(hint), None) = (hir_type_hint, hinted_value_type.as_ref())
            && is_map_or_set_type_hint(hint)
            && !script_hint_proven
            && let Some(guard) =
                type_guard_for_hint(hint, crate::GuardLocation::Local, name.clone(), &self.facts)
        {
            self.emit_spanned(
                UnlinkedInstructionKind::GuardType {
                    src: register,
                    guard,
                },
                path_span,
            );
        }
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
                .set_local_fact(local, name.clone(), script_fact);
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
            self.script_types.set_name_fact(name.clone(), script_fact);
            self.value_types.set_name(name.clone(), value_type);
            self.value_shapes.set_name(name, value_shape);
        }
        Ok(false)
    }

    pub(super) fn compile_return_path(
        &mut self,
        path: Vec<String>,
        path_span: Span,
    ) -> CompileResult<bool> {
        let register = self.compile_path_with_expected_type(
            path_span,
            &path,
            self.return_type.clone(),
            TypeContractContext::Return,
        )?;
        self.emit(UnlinkedInstructionKind::Return { src: register });
        Ok(true)
    }

    pub(super) fn compile_syntax_path_expr_statement(
        &mut self,
        source: vela_common::SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<bool>> {
        let Some((path, span)) = syntax_path_and_span(source, expression) else {
            return Ok(None);
        };
        self.compile_path_expr(span, &path)?;
        Ok(Some(false))
    }

    pub(super) fn compile_syntax_path_expr_to(
        &mut self,
        source: vela_common::SourceId,
        expression: &SyntaxExpression,
        dst: Register,
    ) -> CompileResult<Option<bool>> {
        let Some((path, span)) = syntax_path_and_span(source, expression) else {
            return Ok(None);
        };
        let value = self.compile_path_expr(span, &path)?;
        if value != dst {
            self.emit(UnlinkedInstructionKind::Move { dst, src: value });
        }
        Ok(Some(false))
    }

    fn compile_path_with_expected_type(
        &mut self,
        span: Span,
        path: &[String],
        expected: Option<RuntimeTypeFact>,
        context: TypeContractContext,
    ) -> CompileResult<Register> {
        if path.is_empty() {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "path expression",
            )));
        }
        let Some(expected) = expected else {
            return self.compile_path_expr(span, path);
        };
        let outcome = check_expected_type(
            self.static_type_for_path(span, path),
            expected,
            span,
            context.clone(),
        )?;
        let register = self.compile_path_expr(span, path)?;
        if let ExpectedTypeOutcome::RequiresRuntimeGuard(expected) = &outcome
            && let Some((location, name)) = guard_location_and_name(context)
            && let Some(plan) = type_guard_plan_for_runtime_type(expected)
        {
            self.emit_spanned(
                UnlinkedInstructionKind::GuardType {
                    src: register,
                    guard: UnlinkedTypeGuard::new(
                        plan,
                        UnlinkedGuardContext::new(GuardKind::Contract, location, name),
                    ),
                },
                span,
            );
        }
        Ok(register)
    }

    fn static_type_for_path(&self, span: Span, path: &[String]) -> StaticExprType {
        self.value_type_for_path(span, path)
            .map(StaticExprType::Exact)
            .unwrap_or(StaticExprType::Dynamic)
    }

    pub(super) fn value_type_for_path(
        &self,
        span: Span,
        path: &[String],
    ) -> Option<RuntimeTypeFact> {
        let [name] = path else {
            return self.value_types.local_at_span(self.bindings, span);
        };
        self.value_types
            .local_at_span(self.bindings, span)
            .or_else(|| self.value_types.name(name))
    }

    pub(super) fn script_fact_for_path(
        &self,
        span: Span,
        path: &[String],
    ) -> Option<ScriptTypeFact> {
        let [name] = path else {
            return self.script_types.local_fact_at_span(self.bindings, span);
        };
        self.script_types
            .local_fact_at_span(self.bindings, span)
            .or_else(|| self.script_types.name_fact(name))
            .or_else(|| self.global_type_named(name).map(ScriptTypeFact::new))
    }

    fn value_shape_for_path(&self, span: Span, path: &[String]) -> Option<ValueShape> {
        let [name] = path else {
            return self.value_shapes.local_at_span(self.bindings, span);
        };
        self.value_shapes
            .local_at_span(self.bindings, span)
            .or_else(|| self.value_shapes.name(name))
            .or_else(|| {
                self.script_types
                    .name(name)
                    .or_else(|| self.global_type_named(name))
                    .and_then(|type_name| self.record_shape_for_type(&type_name))
                    .map(ValueShape::Record)
            })
    }
}

fn syntax_path_and_span(
    source: vela_common::SourceId,
    expression: &SyntaxExpression,
) -> Option<(Vec<String>, Span)> {
    let path = expression_syntax_path_or_self(expression)?;
    if path.is_empty() {
        return None;
    }
    let range = expression.syntax().text_range();
    Some((
        path,
        Span::new(source, range.start().into(), range.end().into()),
    ))
}
