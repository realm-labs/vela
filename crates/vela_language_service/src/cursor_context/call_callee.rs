use vela_syntax::ast::{AstNode, SyntaxCallExpr, SyntaxSourceFile};
use vela_syntax::{TextRange as SyntaxTextRange, TextSize};

use crate::TextRange;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct CallCalleeRanges {
    pub(super) callee: TextRange,
    pub(super) member_receiver: Option<TextRange>,
}

pub(super) fn call_callee_for_source(
    source: &SyntaxSourceFile,
    offset: usize,
) -> Option<CallCalleeRanges> {
    let offset = syntax_offset(offset)?;
    source
        .syntax()
        .descendants()
        .filter_map(SyntaxCallExpr::cast)
        .filter_map(|call| call_callee_for_argument_offset(&call, offset))
        .min_by_key(|(_, arg_list_range)| u32::from(arg_list_range.len()))
        .map(|(ranges, _)| ranges)
}

fn call_callee_for_argument_offset(
    call: &SyntaxCallExpr,
    offset: TextSize,
) -> Option<(CallCalleeRanges, SyntaxTextRange)> {
    let arg_list = call.arg_list()?;
    let arg_list_range = arg_list.syntax().text_range();
    if !range_contains_argument_offset(arg_list_range, offset) {
        return None;
    }

    Some((call_callee_ranges(call)?, arg_list_range))
}

fn call_callee_ranges(call: &SyntaxCallExpr) -> Option<CallCalleeRanges> {
    let callee = call.callee()?;
    let member_receiver = callee
        .as_field()
        .and_then(|field| field.receiver())
        .and_then(|receiver| text_range(receiver.syntax().text_range()));

    Some(CallCalleeRanges {
        callee: text_range(callee.syntax().text_range())?,
        member_receiver,
    })
}

fn text_range(range: SyntaxTextRange) -> Option<TextRange> {
    Some(TextRange::new(
        text_size_to_usize(range.start()),
        text_size_to_usize(range.end()),
    ))
}

fn text_size_to_usize(size: TextSize) -> usize {
    u32::from(size) as usize
}

fn range_contains_argument_offset(range: SyntaxTextRange, offset: TextSize) -> bool {
    range.start() < offset && offset <= range.end()
}

fn syntax_offset(offset: usize) -> Option<TextSize> {
    let offset = u32::try_from(offset).ok()?;
    Some(TextSize::from(offset))
}
