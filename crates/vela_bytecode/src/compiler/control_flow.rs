mod block_statement_values;
mod block_values;
mod classification;
mod literal_statement_values;
mod loops;
mod null_values;
mod path_values;
mod range_statement_values;
mod spans;
mod statements;
mod syntax_assignments;
mod syntax_call_args;
mod syntax_calls;
mod syntax_constructors;
mod syntax_containers;
mod syntax_expression_dispatch;
mod syntax_host_indexes;
mod syntax_if_values;
mod syntax_indexes;
mod syntax_match_values;
mod syntax_operator_values;
mod syntax_record_values;
mod syntax_statement_values;

use vela_common::PrimitiveTag;

use super::body_payloads::CompilerStatementPayload;
use super::script_types::{ScriptTypeFact, type_hint_script_type};
use super::value_types::{
    RuntimeTypeFact, StaticExprType, TypeContractContext, check_expected_type, type_hint_value_type,
};
use super::{CompileError, CompileErrorKind, CompileResult, Compiler, frame_slot_kind};
pub(super) use loops::LoopContext;

impl Compiler<'_, '_> {
    fn compile_block_statement_payload(
        &mut self,
        stmt: &CompilerStatementPayload<'_>,
    ) -> CompileResult<bool> {
        let Some(body) = stmt.block_body_payload() else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing block statement body",
            )));
        };
        self.compile_body_payload_statements(&body)
    }

    fn compile_expr_statement_payload(
        &mut self,
        stmt: &CompilerStatementPayload<'_>,
    ) -> CompileResult<bool> {
        let Some(kind) = stmt.stored_expression_kind() else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing expression statement",
            )));
        };
        if let Some((source, expression)) = stmt.expression_statement_syntax_expression()
            && let Some(done) = self.compile_syntax_constant_expr_statement(source, &expression)?
        {
            return Ok(done);
        }
        if let Some((source, expression)) = stmt.expression_statement_syntax_expression()
            && let Some(done) = self.compile_syntax_path_expr_statement(source, &expression)?
        {
            return Ok(done);
        }
        if let Some((source, expression)) = stmt.expression_statement_syntax_expression()
            && let Some(done) = self.compile_syntax_range_expr_statement(source, &expression)?
        {
            return Ok(done);
        }
        if let Some((source, expression)) = stmt.expression_statement_syntax_expression()
            && let Some(done) = self.compile_syntax_block_expr_statement(source, &expression)?
        {
            return Ok(done);
        }
        if let Some((source, expression)) = stmt.expression_statement_syntax_expression()
            && let Some(done) = self.compile_syntax_value_expr_statement(source, &expression)?
        {
            return Ok(done);
        }
        {
            let _ = kind;
            Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "unsupported expression statement",
            )))
        }
    }

    fn compile_break(&mut self) -> CompileResult<bool> {
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

    fn compile_continue(&mut self) -> CompileResult<bool> {
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

fn static_type_runtime_fact(static_type: StaticExprType) -> Option<RuntimeTypeFact> {
    match static_type {
        StaticExprType::Exact(fact) => Some(fact),
        StaticExprType::UnsuffixedIntegerLiteral => {
            Some(RuntimeTypeFact::primitive(PrimitiveTag::I64))
        }
        StaticExprType::UnsuffixedFloatLiteral => {
            Some(RuntimeTypeFact::primitive(PrimitiveTag::F64))
        }
        StaticExprType::Dynamic => None,
    }
}
