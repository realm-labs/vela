mod loops;
mod path_values;

pub(super) use loops::{LoopContext, LoopIterable};

use super::{CompileError, CompileErrorKind, CompileResult, Compiler};

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn compile_break(&mut self) -> CompileResult<bool> {
        if self.loop_stack.is_empty() {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "break outside loop",
            )));
        }
        let jump = self.emit_jump();
        self.loop_stack
            .last_mut()
            .expect("loop stack checked above")
            .push_break(jump);
        Ok(true)
    }

    pub(in crate::compiler) fn compile_continue(&mut self) -> CompileResult<bool> {
        if self.loop_stack.is_empty() {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "continue outside loop",
            )));
        }
        let jump = self.emit_jump();
        self.loop_stack
            .last_mut()
            .expect("loop stack checked above")
            .push_continue(jump);
        Ok(true)
    }
}
