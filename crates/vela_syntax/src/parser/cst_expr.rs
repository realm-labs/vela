use super::{CstParser, DelimiterDepth};
use crate::SyntaxKind;

impl CstParser<'_, '_> {
    pub(super) fn expression_range(&mut self, start: usize, end: usize) {
        let expression_start = self.skip_trivia(start);
        let expression_end = self.trim_trailing_trivia(expression_start, end);
        self.emit_until(expression_start);
        if expression_start >= expression_end {
            self.emit_until(end);
            return;
        }

        let kind = self.expression_kind(expression_start, expression_end);
        self.builder.start_node(kind);
        match kind {
            SyntaxKind::AssignExpr => {
                self.assignment_expression_body(expression_start, expression_end);
            }
            SyntaxKind::BinaryExpr => self.binary_expression_body(expression_start, expression_end),
            SyntaxKind::UnaryExpr => self.unary_expression_body(expression_start, expression_end),
            SyntaxKind::FieldExpr => self.field_expression_body(expression_start, expression_end),
            SyntaxKind::CallExpr => self.call_expression_body(expression_start, expression_end),
            SyntaxKind::IfExpr => self.if_expression_body(expression_start, expression_end),
            _ => self.emit_until(expression_end),
        }
        self.builder.finish_node();
        self.emit_until(end);
    }

    pub(super) fn statement_expression_end(&self, start: usize, end: usize) -> usize {
        let trimmed = self.trim_trailing_trivia(start, end);
        if trimmed > start && self.at_kind(trimmed - 1, SyntaxKind::Semicolon) {
            self.trim_trailing_trivia(start, trimmed - 1)
        } else {
            trimmed
        }
    }

    fn assignment_expression_body(&mut self, start: usize, end: usize) {
        let Some(operator) = self.find_root_assign_op_before(start, end) else {
            self.emit_until(end);
            return;
        };
        self.expression_range(start, operator);
        self.emit_until(operator + 1);
        let value_start = self.skip_trivia(operator + 1);
        self.expression_range(value_start, end);
    }

    fn binary_expression_body(&mut self, start: usize, end: usize) {
        let Some(operator) = self.find_root_binary_op_before(start, end) else {
            self.emit_until(end);
            return;
        };
        self.expression_range(start, operator);
        self.emit_until(operator + 1);
        let rhs_start = self.skip_trivia(operator + 1);
        self.expression_range(rhs_start, end);
    }

    fn unary_expression_body(&mut self, start: usize, end: usize) {
        self.emit_until(start + 1);
        let operand_start = self.skip_trivia(start + 1);
        self.expression_range(operand_start, end);
    }

    fn field_expression_body(&mut self, start: usize, end: usize) {
        let Some(dot) = self.find_last_root_kind_before(SyntaxKind::Dot, start, end) else {
            self.emit_until(end);
            return;
        };
        self.expression_range(start, dot);
        self.emit_until(end);
    }

    fn call_expression_body(&mut self, start: usize, end: usize) {
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
        let close = end.saturating_sub(1);
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

    fn expression_kind(&self, start: usize, end: usize) -> SyntaxKind {
        if self.find_root_assign_op_before(start, end).is_some() {
            return SyntaxKind::AssignExpr;
        }
        if self.find_root_binary_op_before(start, end).is_some() {
            return SyntaxKind::BinaryExpr;
        }
        if self.is_unary_expression(start, end) {
            return SyntaxKind::UnaryExpr;
        }
        if self
            .find_last_root_kind_before(SyntaxKind::Dot, start, end)
            .is_some()
        {
            return SyntaxKind::FieldExpr;
        }
        if self.find_outer_call_arg_list_start(start, end).is_some() {
            return SyntaxKind::CallExpr;
        }
        match self.kind_at(start) {
            Some(
                SyntaxKind::Int
                | SyntaxKind::Float
                | SyntaxKind::Char
                | SyntaxKind::String
                | SyntaxKind::InterpolatedString
                | SyntaxKind::Bytes
                | SyntaxKind::TrueKw
                | SyntaxKind::FalseKw
                | SyntaxKind::NullKw,
            ) if self.single_significant_token(start, end) => SyntaxKind::Literal,
            Some(SyntaxKind::LBracket) => SyntaxKind::ArrayExpr,
            Some(SyntaxKind::LBrace) => SyntaxKind::MapExpr,
            Some(SyntaxKind::Pipe) => SyntaxKind::LambdaExpr,
            Some(SyntaxKind::IfKw) => SyntaxKind::IfExpr,
            Some(SyntaxKind::MatchKw) => SyntaxKind::MatchExpr,
            _ => SyntaxKind::PathExpr,
        }
    }

    fn find_argument_end(&self, start: usize, end: usize) -> usize {
        let mut depth = DelimiterDepth::default();
        for cursor in start..end {
            let Some(current) = self.kind_at(cursor) else {
                break;
            };
            if depth.is_root() && current == SyntaxKind::Comma {
                return cursor;
            }
            depth.bump(current);
        }
        end
    }

    fn find_last_root_kind_before(
        &self,
        kind: SyntaxKind,
        start: usize,
        end: usize,
    ) -> Option<usize> {
        let mut result = None;
        let mut depth = DelimiterDepth::default();
        for cursor in start..end {
            let Some(current) = self.kind_at(cursor) else {
                break;
            };
            if depth.is_root() && current == kind {
                result = Some(cursor);
            }
            depth.bump(current);
        }
        result
    }

    fn find_root_assign_op_before(&self, start: usize, end: usize) -> Option<usize> {
        let mut depth = DelimiterDepth::default();
        for cursor in start..end {
            let Some(current) = self.kind_at(cursor) else {
                break;
            };
            if depth.is_root() && Self::is_assignment_operator(current) {
                return Some(cursor);
            }
            depth.bump(current);
        }
        None
    }

    fn find_root_binary_op_before(&self, start: usize, end: usize) -> Option<usize> {
        let mut depth = DelimiterDepth::default();
        for cursor in start..end {
            let Some(current) = self.kind_at(cursor) else {
                break;
            };
            if depth.is_root() && cursor > start && Self::is_binary_operator(current) {
                return Some(cursor);
            }
            depth.bump(current);
        }
        None
    }

    fn find_outer_call_arg_list_start(&self, start: usize, end: usize) -> Option<usize> {
        let mut depth = DelimiterDepth::default();
        for cursor in start..end {
            let Some(current) = self.kind_at(cursor) else {
                break;
            };
            if depth.is_root()
                && current == SyntaxKind::LParen
                && cursor > start
                && self.find_matching_delimiter_end(cursor, SyntaxKind::LParen, SyntaxKind::RParen)
                    == Some(end)
            {
                return Some(cursor);
            }
            depth.bump(current);
        }
        None
    }

    fn is_unary_expression(&self, start: usize, end: usize) -> bool {
        matches!(
            self.kind_at(start),
            Some(SyntaxKind::Bang | SyntaxKind::Minus)
        ) && self.next_significant_before(start + 1, end).is_some()
    }

    fn single_significant_token(&self, start: usize, end: usize) -> bool {
        self.tokens[start..end]
            .iter()
            .filter(|token| !token.kind.is_trivia())
            .count()
            == 1
    }

    fn is_assignment_operator(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            SyntaxKind::Equal
                | SyntaxKind::PlusEqual
                | SyntaxKind::MinusEqual
                | SyntaxKind::StarEqual
                | SyntaxKind::SlashEqual
                | SyntaxKind::PercentEqual
        )
    }

    fn is_binary_operator(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            SyntaxKind::OrOr
                | SyntaxKind::AndAnd
                | SyntaxKind::EqualEqual
                | SyntaxKind::BangEqual
                | SyntaxKind::EqualEqualEqual
                | SyntaxKind::BangEqualEqual
                | SyntaxKind::Less
                | SyntaxKind::LessEqual
                | SyntaxKind::Greater
                | SyntaxKind::GreaterEqual
                | SyntaxKind::DotDot
                | SyntaxKind::DotDotEqual
                | SyntaxKind::Plus
                | SyntaxKind::Minus
                | SyntaxKind::Star
                | SyntaxKind::Slash
                | SyntaxKind::Percent
        )
    }
}
