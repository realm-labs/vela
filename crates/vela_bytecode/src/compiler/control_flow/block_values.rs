use vela_syntax::ast::Block;

use crate::compiler::body_payloads::CompilerBodyPayload;
use crate::compiler::value_flow::{BlockValue, block_value};
use crate::compiler::{CompileResult, Compiler};
use crate::{Constant, Register, UnlinkedInstructionKind};

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn compile_block_value_to(
        &mut self,
        block: &Block,
        dst: Register,
    ) -> CompileResult<bool> {
        match block_value(block) {
            BlockValue::Empty => {
                self.emit_constant_to(dst, Constant::Null);
                Ok(false)
            }
            BlockValue::TailExpr { prefix, expr } => {
                for stmt in prefix {
                    if self.compile_statement(stmt)? {
                        return Ok(true);
                    }
                }
                let value = self.compile_expr(expr)?;
                self.emit(UnlinkedInstructionKind::Move { dst, src: value });
                Ok(false)
            }
            BlockValue::Statements(statements) => {
                let returned = self.compile_statements(statements)?;
                if !returned {
                    self.emit_constant_to(dst, Constant::Null);
                }
                Ok(returned)
            }
        }
    }

    pub(in crate::compiler::control_flow) fn compile_block_payload_value_to(
        &mut self,
        body: &CompilerBodyPayload<'_>,
        dst: Register,
    ) -> CompileResult<bool> {
        let statements = body.statement_payloads();
        match block_value(body.fallback()) {
            BlockValue::Empty => {
                self.emit_constant_to(dst, Constant::Null);
                Ok(false)
            }
            BlockValue::TailExpr { prefix, expr } => {
                for stmt in statements.iter().take(prefix.len()) {
                    if self.compile_statement_payload(stmt)? {
                        return Ok(true);
                    }
                }
                let value = self.compile_expr(expr)?;
                self.emit(UnlinkedInstructionKind::Move { dst, src: value });
                Ok(false)
            }
            BlockValue::Statements(_) => {
                let returned = self.compile_statement_payloads(&statements)?;
                if !returned {
                    self.emit_constant_to(dst, Constant::Null);
                }
                Ok(returned)
            }
        }
    }
}
