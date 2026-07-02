use vela_common::Span;
use vela_syntax::ast::{Block, Expr, Pattern};

use crate::Register;

use crate::compiler::body_payloads::{
    CompilerBodyPayload, CompilerExpressionPayload, CompilerPatternPayload,
};
use crate::compiler::{CompileError, CompileErrorKind, CompileResult};

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

pub(super) struct ForStatementParts<'ast> {
    pub(super) stmt_span: Span,
    pub(super) index_pattern: Option<&'ast Pattern>,
    pub(super) pattern: &'ast Pattern,
    pub(super) iterable: &'ast Expr,
    pub(super) body: &'ast Block,
    pub(super) index_pattern_payload: Option<CompilerPatternPayload<'ast>>,
    pub(super) pattern_payload: Option<CompilerPatternPayload<'ast>>,
    pub(super) iterable_payload: Option<CompilerExpressionPayload<'ast>>,
    pub(super) body_payload: Option<CompilerBodyPayload<'ast>>,
}

pub(super) fn reject_missing_for_pattern_payloads(
    index_pattern_payload: Option<&CompilerPatternPayload<'_>>,
    value_pattern_payload: Option<&CompilerPatternPayload<'_>>,
) -> CompileResult<()> {
    if index_pattern_payload.is_some_and(|payload| payload.syntax_pattern_kind().is_none()) {
        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "missing CST for index pattern payload",
        )));
    }
    if value_pattern_payload
        .and_then(CompilerPatternPayload::syntax_pattern_kind)
        .is_none()
    {
        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "missing CST for value pattern payload",
        )));
    }
    Ok(())
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
