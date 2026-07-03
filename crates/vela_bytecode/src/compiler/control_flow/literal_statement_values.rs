use vela_common::Span;
use vela_hir::binding::LocalBindingKind;
use vela_syntax::ast::Literal;

use crate::UnlinkedInstructionKind;
use crate::compiler::const_eval::compile_literal_constant_for_type;
use crate::compiler::script_types::{ScriptTypeFact, type_hint_script_type};
use crate::compiler::value_types::{
    RuntimeTypeFact, TypeContractContext, check_expected_type, static_literal_type,
    type_hint_value_type,
};
use crate::compiler::{CompileResult, Compiler, frame_slot_kind};

use super::static_type_runtime_fact;

impl Compiler<'_, '_> {
    pub(in crate::compiler::control_flow) fn compile_let_literal(
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

    pub(in crate::compiler::control_flow) fn compile_return_literal(
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
}
