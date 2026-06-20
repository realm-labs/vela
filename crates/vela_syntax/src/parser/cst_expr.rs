use vela_common::SourceId;

use super::{CstParser, DelimiterDepth};
use crate::SyntaxKind;
use crate::lexer::lex;

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
            SyntaxKind::ParenExpr => self.paren_expression_body(expression_start, expression_end),
            SyntaxKind::BinaryExpr => self.binary_expression_body(expression_start, expression_end),
            SyntaxKind::UnaryExpr => self.unary_expression_body(expression_start, expression_end),
            SyntaxKind::FieldExpr => self.field_expression_body(expression_start, expression_end),
            SyntaxKind::CallExpr => self.call_expression_body(expression_start, expression_end),
            SyntaxKind::IndexExpr => self.index_expression_body(expression_start, expression_end),
            SyntaxKind::TryExpr => self.try_expression_body(expression_start, expression_end),
            SyntaxKind::ArrayExpr => self.array_expression_body(expression_start, expression_end),
            SyntaxKind::MapExpr => self.map_expression_body(expression_start, expression_end),
            SyntaxKind::RecordExpr => {
                self.record_expression_body(expression_start, expression_end);
            }
            SyntaxKind::LambdaExpr => {
                self.lambda_expression_body(expression_start, expression_end);
            }
            SyntaxKind::Literal => self.literal_expression_body(expression_start, expression_end),
            SyntaxKind::Block => self.block_body(expression_start, expression_end),
            SyntaxKind::IfExpr => self.if_expression_body(expression_start, expression_end),
            SyntaxKind::MatchExpr => self.match_expression_body(expression_start, expression_end),
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

    fn paren_expression_body(&mut self, start: usize, end: usize) {
        self.emit_until(start + 1);
        let close = end.saturating_sub(1);
        let value_start = self.skip_trivia(start + 1);
        self.expression_range(value_start, close);
        self.emit_until(end);
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

    fn index_expression_body(&mut self, start: usize, end: usize) {
        let Some(index_start) = self.find_outer_index_list_start(start, end) else {
            self.emit_until(end);
            return;
        };
        self.expression_range(start, index_start);
        self.emit_until(index_start + 1);
        let index_end = end.saturating_sub(1);
        let value_start = self.skip_trivia(index_start + 1);
        self.expression_range(value_start, index_end);
        self.emit_until(end);
    }

    fn try_expression_body(&mut self, start: usize, end: usize) {
        self.expression_range(start, end.saturating_sub(1));
        self.emit_until(end);
    }

    fn literal_expression_body(&mut self, start: usize, end: usize) {
        if self.kind_at(start) == Some(SyntaxKind::InterpolatedString)
            && self.single_significant_token(start, end)
        {
            self.interpolated_string_literal_body(start);
        } else {
            self.emit_until(end);
        }
    }

    fn interpolated_string_literal_body(&mut self, token_index: usize) {
        let text = self.tokens[token_index].text.as_str();
        let (content_start, content_end) = interpolated_content_range(text);
        let Some((content_start, content_end)) = content_start.zip(content_end) else {
            self.emit_current_token();
            return;
        };

        let mut chunk_start = 0;
        let mut cursor = content_start;
        let mut emitted_interpolation = false;

        while cursor < content_end {
            if starts_with_at(text, cursor, "{{") || starts_with_at(text, cursor, "}}") {
                cursor += 2;
                continue;
            }
            if !starts_with_at(text, cursor, "{") {
                cursor = next_char_boundary(text, cursor);
                continue;
            }

            let Some(close) = find_interpolation_close(text, cursor + 1, content_end) else {
                break;
            };

            if chunk_start < cursor {
                self.builder
                    .token(SyntaxKind::InterpolatedString, &text[chunk_start..cursor]);
            }
            self.builder.start_node(SyntaxKind::Interpolation);
            self.builder.token(SyntaxKind::LBrace, "{");
            self.interpolation_expression_body(&text[cursor + 1..close]);
            self.builder.token(SyntaxKind::RBrace, "}");
            self.builder.finish_node();
            emitted_interpolation = true;

            cursor = close + 1;
            chunk_start = cursor;
        }

        if !emitted_interpolation {
            self.emit_current_token();
            return;
        }

        if chunk_start < text.len() {
            self.builder
                .token(SyntaxKind::InterpolatedString, &text[chunk_start..]);
        }
        self.pos = token_index + 1;
    }

    fn interpolation_expression_body(&mut self, source: &str) {
        let lexed = lex(SourceId::new(0), source);
        let end = lexed.lossless_tokens.len().saturating_sub(1);
        if end == 0 {
            return;
        }
        let mut nested = CstParser::new(&lexed.lossless_tokens, self.builder);
        nested.expression_range(0, end);
    }

    fn array_expression_body(&mut self, start: usize, end: usize) {
        let close = end.saturating_sub(1);
        self.emit_until(start + 1);
        self.comma_separated_expressions(close);
        self.emit_until(end);
    }

    fn map_expression_body(&mut self, start: usize, end: usize) {
        let close = end.saturating_sub(1);
        self.emit_until(start + 1);
        while self.pos < close {
            let entry_start = self.skip_trivia(self.pos);
            self.emit_until(entry_start);
            if entry_start >= close {
                break;
            }
            if self.at_kind(entry_start, SyntaxKind::Comma) {
                self.emit_current_token();
                continue;
            }

            let entry_end = self.find_argument_end(entry_start, close);
            self.map_entry_range(entry_start, entry_end);
            if self.pos < close && self.at_kind(self.pos, SyntaxKind::Comma) {
                self.emit_current_token();
            }
        }
        self.emit_until(end);
    }

    fn map_entry_range(&mut self, start: usize, end: usize) {
        self.builder.start_node(SyntaxKind::MapEntry);
        if let Some(colon) = self.find_root_kind_before(SyntaxKind::Colon, start, end) {
            self.expression_range(start, colon);
            self.emit_until(colon + 1);
            let value_start = self.skip_trivia(colon + 1);
            self.expression_range(value_start, end);
        } else {
            self.emit_until(end);
        }
        self.emit_until(end);
        self.builder.finish_node();
    }

    fn record_expression_body(&mut self, start: usize, end: usize) {
        let Some(fields_start) = self.find_outer_record_field_list_start(start, end) else {
            self.emit_until(end);
            return;
        };
        self.expression_range(start, fields_start);
        self.record_expr_field_list(fields_start, end);
        self.emit_until(end);
    }

    fn record_expr_field_list(&mut self, start: usize, end: usize) {
        let fields_end = self
            .find_matching_delimiter_end(start, SyntaxKind::LBrace, SyntaxKind::RBrace)
            .filter(|candidate| *candidate <= end)
            .unwrap_or(end);
        let close = fields_end.saturating_sub(1);
        self.builder.start_node(SyntaxKind::RecordExprFieldList);
        self.emit_until(start + 1);
        while self.pos < close {
            let field_start = self.skip_trivia(self.pos);
            self.emit_until(field_start);
            if field_start >= close {
                break;
            }
            if self.at_kind(field_start, SyntaxKind::Comma) {
                self.emit_current_token();
                continue;
            }

            let field_end = self.find_argument_end(field_start, close);
            self.record_expr_field_range(field_start, field_end);
            if self.pos < close && self.at_kind(self.pos, SyntaxKind::Comma) {
                self.emit_current_token();
            }
        }
        self.emit_until(fields_end);
        self.builder.finish_node();
    }

    fn record_expr_field_range(&mut self, start: usize, end: usize) {
        self.builder.start_node(SyntaxKind::RecordExprField);
        if let Some(colon) = self.find_root_kind_before(SyntaxKind::Colon, start, end) {
            self.emit_until(colon + 1);
            let value_start = self.skip_trivia(colon + 1);
            self.expression_range(value_start, end);
        } else {
            self.emit_until(end);
        }
        self.emit_until(end);
        self.builder.finish_node();
    }

    fn lambda_expression_body(&mut self, start: usize, end: usize) {
        let Some(params_end) = self.find_lambda_param_list_end(start, end) else {
            self.emit_until(end);
            return;
        };
        self.lambda_param_list(start, params_end);

        let body_start = self.skip_trivia(params_end);
        self.emit_until(body_start);
        if body_start >= end {
            return;
        }
        if self.at_kind(body_start, SyntaxKind::LBrace) {
            let body_end = self.find_matching_brace_end(body_start).min(end);
            self.block_range(body_start, body_end);
            self.emit_until(end);
        } else {
            self.expression_range(body_start, end);
        }
    }

    fn lambda_param_list(&mut self, start: usize, end: usize) {
        let close = end.saturating_sub(1);
        self.builder.start_node(SyntaxKind::ParamList);
        self.emit_until(start + 1);
        let mut param_start = self.pos;
        while self.pos < close {
            if self.current_kind() == Some(SyntaxKind::Comma)
                && self.member_range_is_at_delimiter_root(param_start, self.pos)
            {
                self.param_range(param_start, self.pos);
                self.emit_current_token();
                param_start = self.pos;
            } else {
                self.pos += 1;
            }
        }
        self.param_range(param_start, close);
        self.emit_until(end);
        self.builder.finish_node();
    }

    pub(super) fn match_expression_body(&mut self, start: usize, end: usize) {
        let Some(arms_start) = self.find_root_kind_before(SyntaxKind::LBrace, start, end) else {
            self.emit_until(end);
            return;
        };
        let scrutinee_start = self.skip_trivia(start + 1);
        self.emit_until(scrutinee_start);
        if scrutinee_start < arms_start {
            self.expression_range(scrutinee_start, arms_start);
        }
        let arms_end = self.find_matching_brace_end(arms_start).min(end);
        self.match_arm_list(arms_start, arms_end);
        self.emit_until(end);
    }

    fn match_arm_list(&mut self, start: usize, end: usize) {
        let close = end.saturating_sub(1);
        self.builder.start_node(SyntaxKind::MatchArmList);
        self.emit_until(start + 1);
        while self.pos < close {
            let arm_start = self.skip_trivia(self.pos);
            self.emit_until(arm_start);
            if arm_start >= close {
                break;
            }
            if matches!(
                self.kind_at(arm_start),
                Some(SyntaxKind::Comma | SyntaxKind::Semicolon)
            ) {
                self.emit_current_token();
                continue;
            }

            let arm_end = self.find_match_arm_end(arm_start, close);
            self.match_arm_range(arm_start, arm_end);
            while self.pos < close
                && matches!(
                    self.current_kind(),
                    Some(SyntaxKind::Comma | SyntaxKind::Semicolon)
                )
            {
                self.emit_current_token();
            }
        }
        self.emit_until(end);
        self.builder.finish_node();
    }

    fn match_arm_range(&mut self, start: usize, end: usize) {
        self.builder.start_node(SyntaxKind::MatchArm);
        let Some(arrow) = self.find_root_kind_before(SyntaxKind::FatArrow, start, end) else {
            self.pattern_range(start, end);
            self.emit_until(end);
            self.builder.finish_node();
            return;
        };

        let guard = self.find_root_kind_before(SyntaxKind::IfKw, start, arrow);
        let pattern_end = guard.unwrap_or(arrow);
        self.pattern_range(start, pattern_end);

        if let Some(guard_start) = guard {
            let guard_expression_start = self.skip_trivia(guard_start + 1);
            self.emit_until(guard_expression_start);
            if guard_expression_start < arrow {
                self.expression_range(guard_expression_start, arrow);
            }
        }

        self.emit_until(arrow + 1);
        let body_start = self.skip_trivia(arrow + 1);
        self.emit_until(body_start);
        if body_start < end {
            if self.at_kind(body_start, SyntaxKind::LBrace) {
                let body_end = self.find_matching_brace_end(body_start).min(end);
                self.block_range(body_start, body_end);
                self.emit_until(end);
            } else {
                self.expression_range(body_start, end);
            }
        }
        self.emit_until(end);
        self.builder.finish_node();
    }

    pub(super) fn pattern_range(&mut self, start: usize, end: usize) {
        let pattern_start = self.skip_trivia(start);
        let pattern_end = self.trim_trailing_trivia(pattern_start, end);
        self.emit_until(pattern_start);
        if pattern_start >= pattern_end {
            self.emit_until(end);
            return;
        }

        let kind = self.pattern_kind(pattern_start, pattern_end);
        self.builder.start_node(kind);
        match kind {
            SyntaxKind::TuplePattern => self.tuple_pattern_body(pattern_start, pattern_end),
            SyntaxKind::RecordPattern => self.record_pattern_body(pattern_start, pattern_end),
            _ => self.emit_until(pattern_end),
        }
        self.builder.finish_node();
        self.emit_until(end);
    }

    fn tuple_pattern_body(&mut self, start: usize, end: usize) {
        let Some(fields_start) = self.find_outer_call_arg_list_start(start, end) else {
            self.emit_until(end);
            return;
        };
        self.emit_until(fields_start + 1);
        let close = end.saturating_sub(1);
        while self.pos < close {
            let field_start = self.skip_trivia(self.pos);
            self.emit_until(field_start);
            if field_start >= close {
                break;
            }
            if self.at_kind(field_start, SyntaxKind::Comma) {
                self.emit_current_token();
                continue;
            }

            let field_end = self.find_argument_end(field_start, close);
            self.pattern_range(field_start, field_end);
            if self.pos < close && self.at_kind(self.pos, SyntaxKind::Comma) {
                self.emit_current_token();
            }
        }
        self.emit_until(end);
    }

    fn record_pattern_body(&mut self, start: usize, end: usize) {
        let Some(fields_start) = self.find_outer_record_field_list_start(start, end) else {
            self.emit_until(end);
            return;
        };
        self.emit_until(fields_start + 1);
        let close = end.saturating_sub(1);
        while self.pos < close {
            let field_start = self.skip_trivia(self.pos);
            self.emit_until(field_start);
            if field_start >= close {
                break;
            }
            if self.at_kind(field_start, SyntaxKind::Comma) {
                self.emit_current_token();
                continue;
            }

            let field_end = self.find_argument_end(field_start, close);
            self.record_pattern_field_range(field_start, field_end);
            if self.pos < close && self.at_kind(self.pos, SyntaxKind::Comma) {
                self.emit_current_token();
            }
        }
        self.emit_until(end);
    }

    fn record_pattern_field_range(&mut self, start: usize, end: usize) {
        self.builder.start_node(SyntaxKind::RecordPatternField);
        if let Some(colon) = self.find_root_kind_before(SyntaxKind::Colon, start, end) {
            self.emit_until(colon + 1);
            let value_start = self.skip_trivia(colon + 1);
            self.pattern_range(value_start, end);
        } else {
            self.emit_until(end);
        }
        self.emit_until(end);
        self.builder.finish_node();
    }

    fn comma_separated_expressions(&mut self, close: usize) {
        while self.pos < close {
            let expression_start = self.skip_trivia(self.pos);
            self.emit_until(expression_start);
            if expression_start >= close {
                break;
            }
            if self.at_kind(expression_start, SyntaxKind::Comma) {
                self.emit_current_token();
                continue;
            }

            let expression_end = self.find_argument_end(expression_start, close);
            self.expression_range(expression_start, expression_end);
            if self.pos < close && self.at_kind(self.pos, SyntaxKind::Comma) {
                self.emit_current_token();
            }
        }
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
        if self.at_kind(start, SyntaxKind::Pipe) {
            return SyntaxKind::LambdaExpr;
        }
        if self.at_kind(start, SyntaxKind::IfKw) {
            return SyntaxKind::IfExpr;
        }
        if self.at_kind(start, SyntaxKind::MatchKw) {
            return SyntaxKind::MatchExpr;
        }
        if self.at_kind(start, SyntaxKind::LBracket) {
            return SyntaxKind::ArrayExpr;
        }
        if self.at_kind(start, SyntaxKind::LBrace) {
            return self.braced_expression_kind(start, end);
        }
        if self.at_kind(start, SyntaxKind::LParen)
            && self.find_matching_delimiter_end(start, SyntaxKind::LParen, SyntaxKind::RParen)
                == Some(end)
        {
            return SyntaxKind::ParenExpr;
        }
        if self.can_start_record_expression(start)
            && self
                .find_outer_record_field_list_start(start, end)
                .is_some()
        {
            return SyntaxKind::RecordExpr;
        }
        if self.find_root_binary_op_before(start, end).is_some() {
            return SyntaxKind::BinaryExpr;
        }
        if self.is_unary_expression(start, end) {
            return SyntaxKind::UnaryExpr;
        }
        if self.has_trailing_try_suffix(start, end) {
            return SyntaxKind::TryExpr;
        }
        if self.find_outer_index_list_start(start, end).is_some() {
            return SyntaxKind::IndexExpr;
        }
        if self.find_outer_call_arg_list_start(start, end).is_some() {
            return SyntaxKind::CallExpr;
        }
        if self
            .find_last_root_kind_before(SyntaxKind::Dot, start, end)
            .is_some()
        {
            return SyntaxKind::FieldExpr;
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
            _ => SyntaxKind::PathExpr,
        }
    }

    fn braced_expression_kind(&self, start: usize, end: usize) -> SyntaxKind {
        if self.braced_expression_has_map_entry(start, end) {
            SyntaxKind::MapExpr
        } else {
            SyntaxKind::Block
        }
    }

    fn braced_expression_has_map_entry(&self, start: usize, end: usize) -> bool {
        let close = end.saturating_sub(1);
        if self.find_matching_delimiter_end(start, SyntaxKind::LBrace, SyntaxKind::RBrace)
            != Some(end)
        {
            return false;
        }

        let first_entry = self.skip_trivia(start + 1);
        if first_entry >= close {
            return false;
        }

        let first_entry_end = self.find_argument_end(first_entry, close);
        self.find_root_kind_before(SyntaxKind::Colon, first_entry, first_entry_end)
            .is_some()
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

    fn find_outer_record_field_list_start(&self, start: usize, end: usize) -> Option<usize> {
        let mut depth = DelimiterDepth::default();
        for cursor in start..end {
            let Some(current) = self.kind_at(cursor) else {
                break;
            };
            if depth.is_root()
                && current == SyntaxKind::LBrace
                && cursor > start
                && self.find_matching_delimiter_end(cursor, SyntaxKind::LBrace, SyntaxKind::RBrace)
                    == Some(end)
            {
                return Some(cursor);
            }
            depth.bump(current);
        }
        None
    }

    fn can_start_record_expression(&self, start: usize) -> bool {
        matches!(
            self.kind_at(start),
            Some(SyntaxKind::Ident | SyntaxKind::SelfKw)
        )
    }

    fn find_lambda_param_list_end(&self, start: usize, end: usize) -> Option<usize> {
        if self.kind_at(start) != Some(SyntaxKind::Pipe) {
            return None;
        }
        self.find_root_kind_before(SyntaxKind::Pipe, start + 1, end)
            .map(|pipe| pipe + 1)
    }

    fn pattern_kind(&self, start: usize, end: usize) -> SyntaxKind {
        if self.find_outer_call_arg_list_start(start, end).is_some() {
            SyntaxKind::TuplePattern
        } else if self
            .find_outer_record_field_list_start(start, end)
            .is_some()
        {
            SyntaxKind::RecordPattern
        } else {
            SyntaxKind::Pattern
        }
    }

    fn find_match_arm_end(&self, start: usize, end: usize) -> usize {
        let Some(arrow) = self.find_root_kind_before(SyntaxKind::FatArrow, start, end) else {
            return self.find_argument_end(start, end);
        };
        let body_start = self.skip_trivia(arrow + 1);
        if body_start >= end {
            return end;
        }
        if self.at_kind(body_start, SyntaxKind::LBrace) {
            return self.find_matching_brace_end(body_start).min(end);
        }
        self.find_match_arm_expression_end(body_start, end)
    }

    fn find_match_arm_expression_end(&self, start: usize, end: usize) -> usize {
        let mut depth = DelimiterDepth::default();
        for cursor in start..end {
            let Some(current) = self.kind_at(cursor) else {
                break;
            };
            if depth.is_root() {
                if matches!(current, SyntaxKind::Comma | SyntaxKind::Semicolon) {
                    return cursor;
                }
                if current.is_trivia() && self.tokens[cursor].text.contains('\n') {
                    let next = self.skip_trivia(cursor + 1);
                    if next >= end || self.can_start_match_arm(next, end) {
                        return cursor;
                    }
                }
            }
            depth.bump(current);
        }
        end
    }

    fn can_start_match_arm(&self, start: usize, end: usize) -> bool {
        if !matches!(
            self.kind_at(start),
            Some(
                SyntaxKind::Ident
                    | SyntaxKind::TrueKw
                    | SyntaxKind::FalseKw
                    | SyntaxKind::NullKw
                    | SyntaxKind::String
                    | SyntaxKind::Char
                    | SyntaxKind::Bytes
                    | SyntaxKind::Int
                    | SyntaxKind::Float
            )
        ) {
            return false;
        }

        let mut depth = DelimiterDepth::default();
        for cursor in start..end {
            let Some(current) = self.kind_at(cursor) else {
                break;
            };
            if depth.is_root() {
                if current == SyntaxKind::FatArrow {
                    return true;
                }
                if matches!(current, SyntaxKind::Comma | SyntaxKind::Semicolon) {
                    return false;
                }
            }
            depth.bump(current);
        }
        false
    }

    fn find_outer_index_list_start(&self, start: usize, end: usize) -> Option<usize> {
        let mut depth = DelimiterDepth::default();
        for cursor in start..end {
            let Some(current) = self.kind_at(cursor) else {
                break;
            };
            if depth.is_root()
                && current == SyntaxKind::LBracket
                && cursor > start
                && self.find_matching_delimiter_end(
                    cursor,
                    SyntaxKind::LBracket,
                    SyntaxKind::RBracket,
                ) == Some(end)
            {
                return Some(cursor);
            }
            depth.bump(current);
        }
        None
    }

    fn has_trailing_try_suffix(&self, start: usize, end: usize) -> bool {
        end > start
            && self.kind_at(end - 1) == Some(SyntaxKind::Question)
            && self.next_significant_before(start, end - 1).is_some()
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

fn interpolated_content_range(text: &str) -> (Option<usize>, Option<usize>) {
    if text.starts_with("f\"\"\"") && text.ends_with("\"\"\"") {
        (Some(4), Some(text.len().saturating_sub(3)))
    } else if text.starts_with("f\"") && text.ends_with('"') {
        (Some(2), Some(text.len().saturating_sub(1)))
    } else {
        (None, None)
    }
}

fn find_interpolation_close(text: &str, mut cursor: usize, limit: usize) -> Option<usize> {
    let mut depth = 0_u32;
    while cursor < limit {
        if starts_with_at(text, cursor, "\"\"\"") {
            cursor = skip_until_after(text, cursor + 3, limit, "\"\"\"");
            continue;
        }
        if starts_with_at(text, cursor, "\"") {
            cursor = skip_quoted(text, cursor + 1, limit, '"');
            continue;
        }
        if starts_with_at(text, cursor, "'") {
            cursor = skip_quoted(text, cursor + 1, limit, '\'');
            continue;
        }
        if starts_with_at(text, cursor, "//") {
            cursor = skip_line_comment(text, cursor + 2, limit);
            continue;
        }
        if starts_with_at(text, cursor, "/*") {
            cursor = skip_until_after(text, cursor + 2, limit, "*/");
            continue;
        }
        if starts_with_at(text, cursor, "{") {
            depth = depth.saturating_add(1);
            cursor += 1;
            continue;
        }
        if starts_with_at(text, cursor, "}") {
            if depth == 0 {
                return Some(cursor);
            }
            depth = depth.saturating_sub(1);
            cursor += 1;
            continue;
        }
        cursor = next_char_boundary(text, cursor);
    }
    None
}

fn skip_quoted(text: &str, mut cursor: usize, limit: usize, quote: char) -> usize {
    while cursor < limit {
        let Some(ch) = text[cursor..].chars().next() else {
            return cursor;
        };
        cursor += ch.len_utf8();
        if ch == '\\' {
            cursor = next_char_boundary(text, cursor);
            continue;
        }
        if ch == quote {
            return cursor;
        }
    }
    cursor
}

fn skip_line_comment(text: &str, mut cursor: usize, limit: usize) -> usize {
    while cursor < limit {
        let Some(ch) = text[cursor..].chars().next() else {
            return cursor;
        };
        if ch == '\n' {
            return cursor;
        }
        cursor += ch.len_utf8();
    }
    cursor
}

fn skip_until_after(text: &str, mut cursor: usize, limit: usize, needle: &str) -> usize {
    while cursor < limit {
        if starts_with_at(text, cursor, needle) {
            return (cursor + needle.len()).min(limit);
        }
        cursor = next_char_boundary(text, cursor);
    }
    cursor
}

fn starts_with_at(text: &str, cursor: usize, needle: &str) -> bool {
    text.get(cursor..)
        .is_some_and(|suffix| suffix.starts_with(needle))
}

fn next_char_boundary(text: &str, cursor: usize) -> usize {
    if cursor >= text.len() {
        return cursor;
    }
    let Some(ch) = text[cursor..].chars().next() else {
        return text.len();
    };
    cursor + ch.len_utf8()
}
