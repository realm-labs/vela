use vela_common::{SourceId, Span};
use vela_hir::binding::LocalBindingKind;
use vela_syntax::ast::{AstNode, BinaryOp, SyntaxExpression, SyntaxForStmt};

use crate::Register;

use crate::compiler::body_payloads::CompilerBodyPayload;
use crate::compiler::control_flow::classification::{i64_pattern_facts, iterable_item_shape};
use crate::compiler::control_flow::syntax_statement_values::syntax_expression_span;
use crate::compiler::patterns::PatternBindingFacts;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::compiler) struct LoopContext {
    continue_target: usize,
    break_jumps: Vec<usize>,
    continue_jumps: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LoopIterable {
    Generic {
        iterator: Register,
    },
    Range {
        cursor: Register,
        end: Register,
        done: Register,
        inclusive: bool,
    },
}

impl LoopContext {
    pub(super) fn new(continue_target: usize) -> Self {
        Self {
            continue_target,
            break_jumps: Vec::new(),
            continue_jumps: Vec::new(),
        }
    }

    pub(super) fn continue_target(&self) -> usize {
        self.continue_target
    }

    pub(super) fn break_jumps(&self) -> &[usize] {
        &self.break_jumps
    }

    pub(super) fn continue_jumps(&self) -> &[usize] {
        &self.continue_jumps
    }

    pub(super) fn push_break(&mut self, offset: usize) {
        self.break_jumps.push(offset);
    }

    pub(super) fn push_continue(&mut self, offset: usize) {
        self.continue_jumps.push(offset);
    }
}

impl crate::compiler::Compiler<'_, '_> {
    pub(in crate::compiler::control_flow) fn compile_syntax_for_statement(
        &mut self,
        source: SourceId,
        statement: &SyntaxForStmt,
    ) -> crate::compiler::CompileResult<Option<bool>> {
        let Some(iterable_expression) = statement.iterable() else {
            return Ok(None);
        };
        let Some(value_pattern) = statement.value_pattern() else {
            return Ok(None);
        };
        let Some(body) = statement.body() else {
            return Ok(None);
        };
        let statement_span = syntax_for_statement_span(source, statement);
        let range_iterable = syntax_range_iterable(&iterable_expression);
        let item_facts = if range_iterable.is_some() {
            i64_pattern_facts()
        } else {
            PatternBindingFacts::value_shape(
                self.value_shape_for_syntax_expression(Some(source), &iterable_expression)
                    .and_then(iterable_item_shape),
            )
        };
        let loop_iterable = if let Some(inclusive) = range_iterable {
            let Some(binary) = iterable_expression.as_binary() else {
                return Ok(None);
            };
            let Some(start_expression) = binary.lhs() else {
                return Ok(None);
            };
            let Some(end_expression) = binary.rhs() else {
                return Ok(None);
            };
            let Some(cursor) = self.compile_syntax_expression(source, &start_expression)? else {
                return Ok(None);
            };
            let Some(end) = self.compile_syntax_expression(source, &end_expression)? else {
                return Ok(None);
            };
            let done = self.alloc_register()?;
            self.emit_bool_constant_to(done, false);
            LoopIterable::Range {
                cursor,
                end,
                done,
                inclusive,
            }
        } else {
            let Some(iterable_register) =
                self.compile_syntax_expression(source, &iterable_expression)?
            else {
                return Ok(None);
            };
            let iterator = self.alloc_register()?;
            self.emit_spanned(
                crate::UnlinkedInstructionKind::IterInit {
                    dst: iterator,
                    iterable: iterable_register,
                },
                syntax_expression_span(source, &iterable_expression),
            );
            LoopIterable::Generic { iterator }
        };

        let item_register = self.alloc_register()?;
        let index_pattern = statement.index_pattern();
        let loop_index = if index_pattern.is_some() {
            let counter = self.alloc_register()?;
            self.emit_constant_to(
                counter,
                crate::Constant::Scalar(vela_common::ScalarValue::I64(0)),
            );
            Some((
                counter,
                self.emit_constant(crate::Constant::Scalar(vela_common::ScalarValue::I64(1)))?,
            ))
        } else {
            None
        };
        let index_register = if index_pattern.is_some() {
            Some(self.alloc_register()?)
        } else {
            None
        };

        let previous_locals = self.locals.clone();
        let previous_hir_locals = self.hir_locals.clone();
        let previous_script_types = self.script_types.clone();
        let previous_value_types = self.value_types.clone();
        let previous_value_shapes = self.value_shapes.clone();

        let loop_start = self.current_offset();
        let done_jump = match loop_iterable {
            LoopIterable::Generic { iterator } => self.emit_iter_next(iterator, item_register),
            LoopIterable::Range {
                cursor,
                end,
                done,
                inclusive,
            } => self.emit_range_next(cursor, end, done, inclusive, item_register),
        };
        if let (Some((counter, one)), Some(index_register)) = (loop_index, index_register) {
            self.emit(crate::UnlinkedInstructionKind::Move {
                dst: index_register,
                src: counter,
            });
            self.emit(crate::UnlinkedInstructionKind::Add {
                dst: counter,
                lhs: counter,
                rhs: one,
            });
        }

        let mut mismatch_jumps = Vec::new();
        if let (Some(index_pattern), Some(index_register)) =
            (index_pattern.as_ref(), index_register)
        {
            mismatch_jumps.extend(self.compile_syntax_match_pattern(
                source,
                index_register,
                index_pattern,
            )?);
            self.bind_syntax_pattern_locals(
                index_register,
                index_pattern,
                statement_span,
                i64_pattern_facts(),
                LocalBindingKind::For,
            )?;
        }
        mismatch_jumps.extend(self.compile_syntax_match_pattern(
            source,
            item_register,
            &value_pattern,
        )?);
        self.bind_syntax_pattern_locals(
            item_register,
            &value_pattern,
            statement_span,
            item_facts,
            LocalBindingKind::For,
        )?;

        self.loop_stack.push(LoopContext::new(loop_start));
        let body_payload = CompilerBodyPayload::nested_syntax(source, body);
        let body_returned = self.compile_body_payload_statements(&body_payload)?;
        let loop_context = self
            .loop_stack
            .pop()
            .expect("loop context pushed before compiling for body");
        if !body_returned {
            self.emit(crate::UnlinkedInstructionKind::Jump {
                target: crate::InstructionOffset(loop_start),
            });
        }
        let loop_end = self.current_offset();
        self.patch_jump(done_jump, loop_end)?;
        for jump in mismatch_jumps {
            self.patch_jump(jump, loop_start)?;
        }
        for jump in loop_context.break_jumps() {
            self.patch_jump(*jump, loop_end)?;
        }
        for jump in loop_context.continue_jumps() {
            self.patch_jump(*jump, loop_context.continue_target())?;
        }

        self.locals = previous_locals;
        self.hir_locals = previous_hir_locals;
        self.script_types = previous_script_types;
        self.value_types = previous_value_types;
        self.value_shapes = previous_value_shapes;

        Ok(Some(false))
    }
}

fn syntax_range_iterable(expression: &SyntaxExpression) -> Option<bool> {
    match expression.as_binary()?.operator()? {
        BinaryOp::Range => Some(false),
        BinaryOp::RangeInclusive => Some(true),
        _ => None,
    }
}

fn syntax_for_statement_span(source: SourceId, statement: &SyntaxForStmt) -> Span {
    let range = statement.syntax().text_range();
    Span::new(source, range.start().into(), range.end().into())
}
