use vela_common::Span;
use vela_hir::binding::LocalBindingKind;
use vela_hir::ids::HirExprId;
use vela_syntax::ast::SyntaxExpression;

use crate::{
    GuardKind, Register, UnlinkedGuardContext, UnlinkedInstructionKind, UnlinkedTypeGuard,
};

use crate::compiler::expected_exprs::guard_location_and_name;
use crate::compiler::record_shapes::ValueShape;
use crate::compiler::value_types::ExpectedTypeOutcome;
use crate::compiler::{type_guard_for_hint, type_guard_plan_for_runtime_type};

use super::classification::{is_map_or_set_type_hint, merge_type_hint_and_value_fact};
use super::spans::syntax_expression_span;
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
        let local_binding = self.let_local_binding_at_statement_span(&name, span);
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
        let Some((path, span)) = self.hir_value_path_and_span(source, expression) else {
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
        let Some((path, span)) = self.hir_value_path_and_span(source, expression) else {
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
        let expression = self.expression_at_span(span)?;
        self.value_type_for_path_expression(expression, path)
    }

    fn value_type_for_path_expression(
        &self,
        expression: HirExprId,
        path: &[String],
    ) -> Option<RuntimeTypeFact> {
        let [name] = path else {
            return self
                .local_for_expression(expression)
                .and_then(|local| self.value_types.local(local));
        };
        self.local_for_expression(expression)
            .and_then(|local| self.value_types.local(local))
            .or_else(|| self.value_types.name(name))
    }

    pub(super) fn script_fact_for_path(
        &self,
        span: Span,
        path: &[String],
    ) -> Option<ScriptTypeFact> {
        let expression = self.expression_at_span(span)?;
        self.script_fact_for_path_expression(expression, path)
    }

    fn script_fact_for_path_expression(
        &self,
        expression: HirExprId,
        path: &[String],
    ) -> Option<ScriptTypeFact> {
        let [name] = path else {
            return self
                .local_for_expression(expression)
                .and_then(|local| self.script_types.local_fact(local));
        };
        self.local_for_expression(expression)
            .and_then(|local| self.script_types.local_fact(local))
            .or_else(|| self.script_types.name_fact(name))
            .or_else(|| self.global_type_named(name).map(ScriptTypeFact::new))
    }

    pub(in crate::compiler) fn value_shape_for_path(
        &self,
        span: Span,
        path: &[String],
    ) -> Option<ValueShape> {
        let expression = self.expression_at_span(span)?;
        self.value_shape_for_path_expression(expression, path)
    }

    fn value_shape_for_path_expression(
        &self,
        expression: HirExprId,
        path: &[String],
    ) -> Option<ValueShape> {
        let [name] = path else {
            return self
                .local_for_expression(expression)
                .and_then(|local| self.value_shapes.local(local));
        };
        self.local_for_expression(expression)
            .and_then(|local| self.value_shapes.local(local))
            .or_else(|| self.value_shapes.name(name))
            .or_else(|| {
                self.script_types
                    .name(name)
                    .or_else(|| self.global_type_named(name))
                    .and_then(|type_name| self.record_shape_for_type(&type_name))
                    .map(ValueShape::Record)
            })
    }

    fn hir_value_path_and_span(
        &self,
        source: vela_common::SourceId,
        expression: &SyntaxExpression,
    ) -> Option<(Vec<String>, Span)> {
        let span = syntax_expression_span(source, expression);
        self.hir_value_path_for_span(span).map(|path| (path, span))
    }

    pub(in crate::compiler) fn hir_value_path_for_span(&self, span: Span) -> Option<Vec<String>> {
        let expression = self.expression_at_span(span)?;
        self.hir_value_path_for_expression(expression)
    }

    pub(in crate::compiler) fn hir_value_path_root_span_for_span(
        &self,
        span: Span,
    ) -> Option<Span> {
        let expression = self.expression_at_span(span)?;
        let root = self.hir_value_path_root_expression(expression)?;
        self.expression_span(root)
    }

    fn hir_value_path_root_expression(&self, expression: HirExprId) -> Option<HirExprId> {
        if let Some(field) = self
            .hir_bodies
            .iter()
            .find_map(|body| body.fields.get(&expression))
        {
            return self.hir_value_path_root_expression(field.receiver);
        }
        if self
            .hir_value_path(expression)
            .is_some_and(|path| !path.is_empty())
            || self.local_for_expression(expression).is_some()
        {
            return Some(expression);
        }
        None
    }

    fn hir_value_path_for_expression(&self, expression: HirExprId) -> Option<Vec<String>> {
        if let Some(path) = self.hir_value_path(expression)
            && !path.is_empty()
        {
            return Some(path.to_vec());
        }
        if let Some(local) = self.local_for_expression(expression)
            && let Some(binding) = self.bindings.local(local)
        {
            return Some(vec![binding.name.clone()]);
        }
        let field = self
            .hir_bodies
            .iter()
            .find_map(|body| body.fields.get(&expression))?;
        let mut path = self.hir_value_path_for_expression(field.receiver)?;
        path.push(field.name.clone());
        Some(path)
    }
}
