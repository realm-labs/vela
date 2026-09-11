use super::{CstParser, DelimiterDepth};
use crate::SyntaxKind;

impl CstParser<'_, '_> {
    pub(super) fn call_expression_body(&mut self, start: usize, end: usize) {
        let Some(args_start) = self.find_outer_call_arg_list_start(start, end) else {
            self.emit_until(end);
            return;
        };
        self.expression_range(start, args_start);
        let args_end = self
            .find_matching_delimiter_end(args_start, SyntaxKind::LParen, SyntaxKind::RParen)
            .filter(|candidate| *candidate <= end)
            .unwrap_or(end);
        self.arg_list(args_start, args_end);
        self.emit_until(end);
    }

    fn arg_list(&mut self, start: usize, end: usize) {
        self.builder.start_node(SyntaxKind::ArgList);
        self.emit_until(start + 1);
        let close =
            if self.find_matching_delimiter_end(start, SyntaxKind::LParen, SyntaxKind::RParen)
                == Some(end)
            {
                end.saturating_sub(1)
            } else {
                end
            };
        while self.pos < close {
            let argument_start = self.skip_trivia(self.pos);
            self.emit_until(argument_start);
            if argument_start >= close {
                break;
            }
            if self.at_kind(argument_start, SyntaxKind::Comma) {
                self.emit_current_token();
                continue;
            }

            let argument_end = self.find_argument_end(argument_start, close);
            self.argument_range(argument_start, argument_end);
            if self.pos < close && self.at_kind(self.pos, SyntaxKind::Comma) {
                self.emit_current_token();
            }
        }
        self.emit_until(end);
        self.builder.finish_node();
    }

    fn argument_range(&mut self, start: usize, end: usize) {
        self.builder.start_node(SyntaxKind::Argument);
        if let Some(equal) = self.find_root_kind_before(SyntaxKind::Equal, start, end) {
            let value_start = self.skip_trivia(equal + 1);
            self.emit_until(value_start);
            self.expression_range(value_start, end);
        } else {
            self.expression_range(start, end);
        }
        self.emit_until(end);
        self.builder.finish_node();
    }

    pub(super) fn find_outer_call_arg_list_start(&self, start: usize, end: usize) -> Option<usize> {
        let mut depth = DelimiterDepth::default();
        for cursor in start..end {
            let Some(current) = self.kind_at(cursor) else {
                break;
            };
            if depth.is_root()
                && current == SyntaxKind::LParen
                && cursor > start
                && self
                    .find_matching_delimiter_end(cursor, SyntaxKind::LParen, SyntaxKind::RParen)
                    .is_none_or(|close| close == end)
            {
                return Some(cursor);
            }
            depth.bump(current);
        }
        None
    }
}
