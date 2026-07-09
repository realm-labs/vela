use vela_common::{PrimitiveTag, Span};
use vela_hir::binding::LocalBindingKind;
use vela_hir::ids::HirPatternId;

use crate::{Constant, UnlinkedInstructionKind};

use crate::compiler::script_types::{ScriptTypeFact, type_hint_script_type};
use crate::compiler::value_types::{
    RuntimeTypeFact, StaticExprType, TypeContractContext, check_expected_type, type_hint_value_type,
};
use crate::compiler::{CompileResult, Compiler, frame_slot_kind};

impl Compiler<'_, '_> {
    pub(super) fn compile_let_without_initializer(
        &mut self,
        name: String,
        span: Span,
        hir_patterns: &[HirPatternId],
    ) -> CompileResult<bool> {
        let local_binding = self.let_local_binding_for_patterns(hir_patterns);
        let hir_type_hint = local_binding.as_ref().and_then(|(_, hint)| hint.as_ref());
        let script_fact = hir_type_hint.and_then(|hint| {
            let known_type_names = self.facts.known_type_names();
            type_hint_script_type(hint, known_type_names.iter()).map(ScriptTypeFact::new)
        });
        let value_type = hir_type_hint.and_then(type_hint_value_type);
        let register = self.emit_constant(Constant::Unit)?;
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
            self.value_shapes.set_local(local, name, None);
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
            self.value_shapes.set_name(name, None);
        }
        Ok(false)
    }

    pub(super) fn compile_empty_return(&mut self, span: Span) -> CompileResult<bool> {
        if let Some(expected) = self.return_type.clone() {
            check_expected_type(
                StaticExprType::Exact(RuntimeTypeFact::primitive(PrimitiveTag::Unit)),
                expected,
                span,
                TypeContractContext::Return,
            )?;
        }
        let register = self.emit_constant(Constant::Unit)?;
        self.emit(UnlinkedInstructionKind::Return { src: register });
        Ok(true)
    }
}
