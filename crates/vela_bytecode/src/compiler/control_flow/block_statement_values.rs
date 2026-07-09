use vela_common::{SourceId, Span};
use vela_hir::binding::LocalBindingKind;
use vela_syntax::ast::SyntaxExpression;

use crate::{
    GuardKind, Register, UnlinkedGuardContext, UnlinkedInstructionKind, UnlinkedTypeGuard,
};

use crate::compiler::expected_exprs::guard_location_and_name;
use crate::compiler::script_types::{ScriptTypeFact, type_hint_script_type};
use crate::compiler::value_types::{
    ExpectedTypeOutcome, RuntimeTypeFact, StaticExprType, TypeContractContext, check_expected_type,
    type_hint_value_type,
};
use crate::compiler::{
    CompileResult, Compiler, frame_slot_kind, type_guard_for_hint, type_guard_plan_for_runtime_type,
};

impl Compiler<'_, '_> {
    pub(super) fn compile_let_syntax_block_value(
        &mut self,
        name: String,
        span: Span,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<bool> {
        let Some(block) = expression.as_block() else {
            return Err(crate::compiler::CompileError::new(
                crate::compiler::CompileErrorKind::UnsupportedSyntax(
                    "missing let initializer block body",
                ),
            ));
        };
        let local_binding = self.let_local_binding_at_statement_span(&name, span);
        let hir_type_hint = local_binding.as_ref().and_then(|(_, hint)| hint.as_ref());
        let hinted_script_fact = hir_type_hint.and_then(|hint| {
            let known_type_names = self.facts.known_type_names();
            type_hint_script_type(hint, known_type_names.iter()).map(ScriptTypeFact::new)
        });
        let hinted_value_type = hir_type_hint.and_then(type_hint_value_type);
        let register = self.alloc_register()?;
        let body = self.hir_block_body_payload(source, block)?;
        let returned = self.compile_block_payload_value_to(&body, register)?;
        if let Some(expected) = hinted_value_type.as_ref() {
            self.emit_dynamic_contract_guard(
                register,
                span,
                expected,
                TypeContractContext::TypedLet { name: name.clone() },
            )?;
        } else if let Some(hint) = hir_type_hint
            && let Some(guard) =
                type_guard_for_hint(hint, crate::GuardLocation::Local, name.clone(), &self.facts)
        {
            self.emit_spanned(
                UnlinkedInstructionKind::GuardType {
                    src: register,
                    guard,
                },
                span,
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
                .set_local_fact(local, name.clone(), hinted_script_fact);
            self.value_types
                .set_local(local, name.clone(), hinted_value_type);
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
            self.value_types.set_name(name.clone(), hinted_value_type);
            self.value_shapes.set_name(name, None);
        }
        Ok(returned)
    }

    pub(super) fn compile_return_syntax_block_value(
        &mut self,
        span: Span,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<bool> {
        let Some(block) = expression.as_block() else {
            return Err(crate::compiler::CompileError::new(
                crate::compiler::CompileErrorKind::UnsupportedSyntax("missing return block body"),
            ));
        };
        let register = self.alloc_register()?;
        let body = self.hir_block_body_payload(source, block)?;
        let returned = self.compile_block_payload_value_to(&body, register)?;
        if let Some(expected) = self.return_type.clone().as_ref() {
            self.emit_dynamic_contract_guard(
                register,
                span,
                expected,
                TypeContractContext::Return,
            )?;
        }
        if !returned {
            self.emit(UnlinkedInstructionKind::Return { src: register });
        }
        Ok(true)
    }

    pub(super) fn emit_dynamic_contract_guard(
        &mut self,
        register: Register,
        span: Span,
        expected: &RuntimeTypeFact,
        context: TypeContractContext,
    ) -> CompileResult<()> {
        let outcome = check_expected_type(
            StaticExprType::Dynamic,
            expected.clone(),
            span,
            context.clone(),
        )?;
        if let ExpectedTypeOutcome::RequiresRuntimeGuard(expected) = outcome
            && let Some((location, name)) = guard_location_and_name(context)
            && let Some(plan) = type_guard_plan_for_runtime_type(&expected)
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
        Ok(())
    }
}
