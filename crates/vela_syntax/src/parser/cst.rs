use vela_common::Diagnostic;

use crate::lexer::Lexed;
use crate::token::LosslessToken;
use crate::{SyntaxKind, SyntaxTreeBuilder};

#[path = "cst_expr.rs"]
mod cst_expr;

pub(crate) fn build_source_tree(lexed: &Lexed, builder: &mut SyntaxTreeBuilder) -> Vec<Diagnostic> {
    let mut parser = CstParser::new(&lexed.lossless_tokens, builder);
    parser.source_file();
    parser.diagnostics
}

struct CstParser<'tokens, 'builder> {
    tokens: &'tokens [LosslessToken],
    pos: usize,
    builder: &'builder mut SyntaxTreeBuilder,
    diagnostics: Vec<Diagnostic>,
}

impl<'tokens, 'builder> CstParser<'tokens, 'builder> {
    fn new(tokens: &'tokens [LosslessToken], builder: &'builder mut SyntaxTreeBuilder) -> Self {
        Self {
            tokens,
            pos: 0,
            builder,
            diagnostics: Vec::new(),
        }
    }

    fn source_file(&mut self) {
        self.builder.start_node(SyntaxKind::SourceFile);
        while !self.at_eof() {
            if self.current_kind().is_some_and(SyntaxKind::is_trivia) {
                self.emit_current_token();
            } else if let Some(item) = self.current_item() {
                self.item(item);
            } else {
                self.error_run();
            }
        }
        self.builder.finish_node();
    }

    fn item(&mut self, item: ItemBoundary) {
        match item.kind {
            SyntaxKind::FunctionItem => self.function_item(item.end),
            SyntaxKind::StructItem => self.struct_item(item.end),
            SyntaxKind::EnumItem => self.enum_item(item.end),
            SyntaxKind::TraitItem => self.trait_item(item.end),
            SyntaxKind::ImplItem => self.impl_item(item.end),
            _ => self.raw_item(item.kind, item.end),
        }
    }

    fn raw_item(&mut self, kind: SyntaxKind, end: usize) {
        self.builder.start_node(kind);
        while self.pos < end {
            self.emit_current_token();
        }
        self.builder.finish_node();
    }

    fn function_item(&mut self, end: usize) {
        self.builder.start_node(SyntaxKind::FunctionItem);
        let param_list = self.find_first_kind_before(SyntaxKind::LParen, self.pos, end);
        let param_list_end = param_list
            .and_then(|start| {
                self.find_matching_delimiter_end(start, SyntaxKind::LParen, SyntaxKind::RParen)
            })
            .unwrap_or(self.pos);
        let body = self.find_first_kind_before(SyntaxKind::LBrace, param_list_end, end);

        if let Some(param_list_start) = param_list {
            self.emit_until(param_list_start);
            self.param_list(param_list_start);
        }

        if let Some(body_start) = body {
            self.return_type(param_list_end, body_start);
        }

        if let Some(body_start) = body {
            self.emit_until(body_start);
            let body_end = self.find_matching_brace_end(body_start).min(end);
            self.block_range(body_start, body_end);
        }

        while self.pos < end {
            self.emit_current_token();
        }
        self.builder.finish_node();
    }

    fn struct_item(&mut self, end: usize) {
        self.builder.start_node(SyntaxKind::StructItem);
        let field_list = self.find_first_kind_before(SyntaxKind::LBrace, self.pos, end);

        if let Some(field_list_start) = field_list {
            self.emit_until(field_list_start);
            self.struct_field_list(field_list_start);
        }

        while self.pos < end {
            self.emit_current_token();
        }
        self.builder.finish_node();
    }

    fn enum_item(&mut self, end: usize) {
        self.builder.start_node(SyntaxKind::EnumItem);
        let variant_list = self.find_first_kind_before(SyntaxKind::LBrace, self.pos, end);

        if let Some(variant_list_start) = variant_list {
            self.emit_until(variant_list_start);
            self.enum_variant_list(variant_list_start);
        }

        while self.pos < end {
            self.emit_current_token();
        }
        self.builder.finish_node();
    }

    fn trait_item(&mut self, end: usize) {
        self.method_owner_item(SyntaxKind::TraitItem, SyntaxKind::TraitMethod, end);
    }

    fn impl_item(&mut self, end: usize) {
        self.method_owner_item(SyntaxKind::ImplItem, SyntaxKind::ImplMethod, end);
    }

    fn method_owner_item(&mut self, item_kind: SyntaxKind, method_kind: SyntaxKind, end: usize) {
        self.builder.start_node(item_kind);
        let body = self.find_first_kind_before(SyntaxKind::LBrace, self.pos, end);

        if let Some(body_start) = body {
            self.emit_until(body_start);
            self.method_body(body_start, method_kind);
        }

        while self.pos < end {
            self.emit_current_token();
        }
        self.builder.finish_node();
    }

    fn return_type(&mut self, start: usize, end: usize) {
        let Some(arrow) = self.find_root_kind_before(SyntaxKind::Arrow, start, end) else {
            return;
        };
        let type_start = self.skip_trivia(arrow + 1);
        let type_end = self.trim_trailing_trivia(type_start, end);
        if type_start >= type_end {
            return;
        }

        self.emit_until(type_start);
        self.type_hint_range(type_start, type_end);
    }

    fn param_list(&mut self, start: usize) {
        self.param_list_with_kind(start, SyntaxKind::ParamList);
    }

    fn param_list_with_kind(&mut self, start: usize, list_kind: SyntaxKind) {
        let Some(end) =
            self.find_matching_delimiter_end(start, SyntaxKind::LParen, SyntaxKind::RParen)
        else {
            self.node_range(list_kind, start, self.pos.saturating_add(1));
            return;
        };

        self.builder.start_node(list_kind);
        self.emit_current_token();
        let close = end.saturating_sub(1);
        let mut param_start = self.pos;
        while self.pos < close {
            if self.current_kind() == Some(SyntaxKind::Comma)
                && self.range_is_at_delimiter_root(param_start, self.pos)
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

    fn param_range(&mut self, start: usize, end: usize) {
        self.pos = start;
        if !self.has_significant_tokens(start, end) {
            self.emit_tokens(start, end);
            return;
        }
        self.builder.start_node(SyntaxKind::Param);

        if let Some(colon) = self.find_root_kind_before(SyntaxKind::Colon, start, end) {
            let value_end = self
                .find_root_kind_before(SyntaxKind::Equal, colon + 1, end)
                .unwrap_or(end);
            let type_start = self.skip_trivia(colon + 1);
            let type_end = self.trim_trailing_trivia(type_start, value_end);
            if type_start < type_end {
                self.emit_until(type_start);
                self.type_hint_range(type_start, type_end);
            }
        }

        self.emit_until(end);
        self.builder.finish_node();
    }

    fn struct_field_list(&mut self, start: usize) {
        self.field_list_with_kind(start, SyntaxKind::StructFieldList);
    }

    fn field_list_with_kind(&mut self, start: usize, list_kind: SyntaxKind) {
        let Some(end) =
            self.find_matching_delimiter_end(start, SyntaxKind::LBrace, SyntaxKind::RBrace)
        else {
            self.node_range(list_kind, start, self.pos.saturating_add(1));
            return;
        };

        self.builder.start_node(list_kind);
        self.emit_current_token();
        let close = end.saturating_sub(1);
        let mut field_start = self.skip_trivia(self.pos);
        self.emit_until(field_start);

        while self.pos < close {
            if matches!(
                self.current_kind(),
                Some(SyntaxKind::Comma | SyntaxKind::Semicolon)
            ) && self.range_is_at_delimiter_root(field_start, self.pos)
            {
                let field_end = self.trim_trailing_trivia(field_start, self.pos);
                self.struct_field_range(field_start, field_end);
                self.emit_current_token();
                field_start = self.skip_trivia(self.pos);
                self.emit_until(field_start);
            } else if self
                .current_kind()
                .is_some_and(|kind| kind.is_trivia() && self.current_token_text_contains('\n'))
                && self.range_is_at_delimiter_root(field_start, self.pos)
                && self.member_range_has_name(field_start, self.pos)
                && self.next_significant_before(self.pos + 1, close).is_some()
            {
                let field_end = self.trim_trailing_trivia(field_start, self.pos);
                self.struct_field_range(field_start, field_end);
                field_start = self.skip_trivia(self.pos);
                self.emit_until(field_start);
            } else {
                self.pos += 1;
            }
        }

        let field_end = self.trim_trailing_trivia(field_start, close);
        self.struct_field_range(field_start, field_end);
        self.emit_until(end);
        self.builder.finish_node();
    }

    fn enum_variant_list(&mut self, start: usize) {
        let Some(end) =
            self.find_matching_delimiter_end(start, SyntaxKind::LBrace, SyntaxKind::RBrace)
        else {
            self.node_range(
                SyntaxKind::EnumVariantList,
                start,
                self.pos.saturating_add(1),
            );
            return;
        };

        self.builder.start_node(SyntaxKind::EnumVariantList);
        self.emit_current_token();
        let close = end.saturating_sub(1);
        let mut variant_start = self.skip_trivia(self.pos);
        self.emit_until(variant_start);

        while self.pos < close {
            if matches!(
                self.current_kind(),
                Some(SyntaxKind::Comma | SyntaxKind::Semicolon)
            ) && self.range_is_at_delimiter_root(variant_start, self.pos)
            {
                let variant_end = self.trim_trailing_trivia(variant_start, self.pos);
                self.enum_variant_range(variant_start, variant_end);
                self.emit_current_token();
                variant_start = self.skip_trivia(self.pos);
                self.emit_until(variant_start);
            } else if self
                .current_kind()
                .is_some_and(|kind| kind.is_trivia() && self.current_token_text_contains('\n'))
                && self.range_is_at_delimiter_root(variant_start, self.pos)
                && self.member_range_has_name(variant_start, self.pos)
                && self.next_significant_before(self.pos + 1, close).is_some()
            {
                let variant_end = self.trim_trailing_trivia(variant_start, self.pos);
                self.enum_variant_range(variant_start, variant_end);
                variant_start = self.skip_trivia(self.pos);
                self.emit_until(variant_start);
            } else {
                self.pos += 1;
            }
        }

        let variant_end = self.trim_trailing_trivia(variant_start, close);
        self.enum_variant_range(variant_start, variant_end);
        self.emit_until(end);
        self.builder.finish_node();
    }

    fn enum_variant_range(&mut self, start: usize, end: usize) {
        self.pos = start;
        if !self.has_significant_tokens(start, end) {
            self.emit_tokens(start, end);
            return;
        }
        self.builder.start_node(SyntaxKind::EnumVariant);

        let name_end = self.member_name_end(start, end);
        let tuple_start = self.find_root_kind_before(SyntaxKind::LParen, name_end, end);
        let record_start = self.find_root_kind_before(SyntaxKind::LBrace, name_end, end);
        match (tuple_start, record_start) {
            (Some(tuple), Some(record)) if tuple < record => {
                self.emit_until(tuple);
                self.param_list_with_kind(tuple, SyntaxKind::TupleFieldList);
            }
            (Some(tuple), None) => {
                self.emit_until(tuple);
                self.param_list_with_kind(tuple, SyntaxKind::TupleFieldList);
            }
            (_, Some(record)) => {
                self.emit_until(record);
                self.field_list_with_kind(record, SyntaxKind::RecordFieldList);
            }
            (None, None) => {}
        }

        self.emit_until(end);
        self.builder.finish_node();
    }

    fn method_body(&mut self, start: usize, method_kind: SyntaxKind) {
        let Some(end) =
            self.find_matching_delimiter_end(start, SyntaxKind::LBrace, SyntaxKind::RBrace)
        else {
            self.emit_current_token();
            return;
        };

        self.emit_current_token();
        let close = end.saturating_sub(1);
        while self.pos < close {
            let candidate = self.skip_trivia(self.pos);
            self.emit_until(candidate);
            if candidate >= close {
                break;
            }

            if self.method_keyword_pos(candidate, close).is_some() {
                let method_end = self.find_method_end(candidate, close);
                self.method_range(method_kind, candidate, method_end);
            } else {
                self.emit_current_token();
            }
        }
        self.emit_until(end);
    }

    fn method_range(&mut self, method_kind: SyntaxKind, start: usize, end: usize) {
        self.pos = start;
        if !self.has_significant_tokens(start, end) {
            self.emit_tokens(start, end);
            return;
        }
        self.builder.start_node(method_kind);

        let signature_start = self.method_keyword_pos(start, end).unwrap_or(start);
        let param_list = self.find_first_kind_before(SyntaxKind::LParen, signature_start, end);
        let param_list_end = param_list
            .and_then(|start| {
                self.find_matching_delimiter_end(start, SyntaxKind::LParen, SyntaxKind::RParen)
            })
            .unwrap_or(signature_start);
        let body = self.find_root_kind_before(SyntaxKind::LBrace, param_list_end, end);
        let signature_end = body
            .or_else(|| self.find_root_kind_before(SyntaxKind::Semicolon, param_list_end, end))
            .or_else(|| self.find_root_newline_before(param_list_end, end))
            .unwrap_or(end);

        if let Some(param_list_start) = param_list {
            self.emit_until(param_list_start);
            self.param_list(param_list_start);
        }

        self.return_type(param_list_end, signature_end);

        if let Some(body_start) = body {
            self.emit_until(body_start);
            let body_end = self.find_matching_brace_end(body_start).min(end);
            self.block_range(body_start, body_end);
        }

        self.emit_until(end);
        self.builder.finish_node();
    }

    fn struct_field_range(&mut self, start: usize, end: usize) {
        self.pos = start;
        if !self.has_significant_tokens(start, end) {
            self.emit_tokens(start, end);
            return;
        }
        self.builder.start_node(SyntaxKind::StructField);

        if let Some(colon) = self.find_root_kind_before(SyntaxKind::Colon, start, end) {
            let value_end = self
                .find_root_kind_before(SyntaxKind::Equal, colon + 1, end)
                .unwrap_or(end);
            let type_start = self.skip_trivia(colon + 1);
            let type_end = self.trim_trailing_trivia(type_start, value_end);
            if type_start < type_end {
                self.emit_until(type_start);
                self.type_hint_range(type_start, type_end);
            }
        }

        self.emit_until(end);
        self.builder.finish_node();
    }

    fn block_range(&mut self, start: usize, end: usize) {
        self.pos = start;
        if self.kind_at(start) != Some(SyntaxKind::LBrace) {
            self.node_range(SyntaxKind::Block, start, end);
            return;
        }

        self.builder.start_node(SyntaxKind::Block);
        self.emit_current_token();
        let close = end.saturating_sub(1);
        while self.pos < close {
            let statement_start = self.skip_trivia(self.pos);
            self.emit_until(statement_start);
            if statement_start >= close {
                break;
            }

            if let Some(kind) = self.statement_kind_at(statement_start, close) {
                let statement_end = self.find_statement_end(kind, statement_start, close);
                self.statement_range(kind, statement_start, statement_end);
            } else {
                self.emit_current_token();
            }
        }
        self.emit_until(end);
        self.builder.finish_node();
    }

    fn statement_range(&mut self, kind: SyntaxKind, start: usize, end: usize) {
        self.pos = start;
        if !self.has_significant_tokens(start, end) {
            self.emit_tokens(start, end);
            return;
        }

        self.builder.start_node(kind);
        match kind {
            SyntaxKind::LetStmt => self.let_statement_body(start, end),
            SyntaxKind::ReturnStmt => self.return_statement_body(start, end),
            SyntaxKind::ForStmt => self.for_statement_body(start, end),
            SyntaxKind::IfExpr => self.if_expression_body(start, end),
            SyntaxKind::MatchExpr => self.match_expression_body(start, end),
            SyntaxKind::ExprStmt => self.expr_statement_body(start, end),
            _ => self.emit_until(end),
        }
        self.builder.finish_node();
    }

    fn let_statement_body(&mut self, start: usize, end: usize) {
        let initializer = self.find_root_kind_before(SyntaxKind::Equal, start, end);
        if let Some(colon) = self.find_root_kind_before(SyntaxKind::Colon, start, end) {
            let value_end = initializer.unwrap_or(end);
            let type_start = self.skip_trivia(colon + 1);
            let type_end = self.trim_trailing_trivia(type_start, value_end);
            if type_start < type_end {
                self.emit_until(type_start);
                self.type_hint_range(type_start, type_end);
            }
        }

        if let Some(equal) = initializer {
            let value_start = self.skip_trivia(equal + 1);
            let value_end = self.statement_expression_end(value_start, end);
            self.emit_until(value_start);
            self.expression_range(value_start, value_end);
        }
        self.emit_until(end);
    }

    fn return_statement_body(&mut self, start: usize, end: usize) {
        let Some(keyword) = self.find_root_kind_before(SyntaxKind::ReturnKw, start, end) else {
            self.emit_until(end);
            return;
        };
        let value_start = self.skip_trivia(keyword + 1);
        let value_end = self.statement_expression_end(value_start, end);
        self.emit_until(value_start);
        self.expression_range(value_start, value_end);
        self.emit_until(end);
    }

    fn expr_statement_body(&mut self, start: usize, end: usize) {
        let expression_end = self.statement_expression_end(start, end);
        self.expression_range(start, expression_end);
        self.emit_until(end);
    }

    fn for_statement_body(&mut self, start: usize, end: usize) {
        let Some(in_kw) = self.find_root_kind_before(SyntaxKind::InKw, start, end) else {
            self.statement_with_body_block(start, end);
            return;
        };
        let Some(body_start) = self.find_for_body_start(in_kw + 1, end) else {
            self.emit_until(end);
            return;
        };

        let pattern_start = self.skip_trivia(start + 1);
        let pattern_end = self.trim_trailing_trivia(pattern_start, in_kw);
        self.emit_until(pattern_start);
        if pattern_start < pattern_end {
            if let Some(comma) =
                self.find_root_kind_before(SyntaxKind::Comma, pattern_start, pattern_end)
            {
                self.pattern_range(pattern_start, comma);
                self.emit_until(comma + 1);
                let value_pattern_start = self.skip_trivia(comma + 1);
                self.pattern_range(value_pattern_start, pattern_end);
            } else {
                self.pattern_range(pattern_start, pattern_end);
            }
        }

        self.emit_until(in_kw + 1);
        let iterable_start = self.skip_trivia(in_kw + 1);
        self.emit_until(iterable_start);
        if iterable_start < body_start {
            self.expression_range(iterable_start, body_start);
        }

        self.emit_until(body_start);
        let body_end = self.find_matching_brace_end(body_start).min(end);
        self.block_range(body_start, body_end);
        self.emit_until(end);
    }

    fn statement_with_body_block(&mut self, start: usize, end: usize) {
        if let Some(body_start) = self.find_root_kind_before(SyntaxKind::LBrace, start, end) {
            self.emit_until(body_start);
            let body_end = self.find_matching_brace_end(body_start).min(end);
            self.block_range(body_start, body_end);
        }
        self.emit_until(end);
    }

    fn if_expression_body(&mut self, start: usize, end: usize) {
        let Some(body_start) = self.find_root_kind_before(SyntaxKind::LBrace, start, end) else {
            self.emit_until(end);
            return;
        };
        self.emit_until(body_start);
        let body_end = self.find_matching_brace_end(body_start).min(end);
        self.block_range(body_start, body_end);

        let else_start = self.skip_trivia(body_end);
        if else_start < end && self.at_kind(else_start, SyntaxKind::ElseKw) {
            let else_body = self.skip_trivia(else_start + 1);
            if else_body < end && self.at_kind(else_body, SyntaxKind::IfKw) {
                self.emit_until(else_body);
                let else_if_end = self.find_if_expression_end(else_body, end);
                self.statement_range(SyntaxKind::IfExpr, else_body, else_if_end);
            } else if else_body < end && self.at_kind(else_body, SyntaxKind::LBrace) {
                self.emit_until(else_body);
                let else_block_end = self.find_matching_brace_end(else_body).min(end);
                self.block_range(else_body, else_block_end);
            }
        }

        self.emit_until(end);
    }

    fn type_hint_range(&mut self, start: usize, end: usize) {
        self.builder.start_node(SyntaxKind::TypeHint);
        if let Some(args_start) = self.find_root_kind_before(SyntaxKind::Less, start, end) {
            self.emit_until(args_start);
            self.type_arg_list(args_start, end);
        }
        self.emit_until(end);
        self.builder.finish_node();
    }

    fn type_arg_list(&mut self, start: usize, end: usize) {
        let args_end = self
            .find_matching_delimiter_end(start, SyntaxKind::Less, SyntaxKind::Greater)
            .filter(|candidate| *candidate <= end)
            .unwrap_or(end);
        self.node_range(SyntaxKind::TypeArgList, start, args_end);
    }

    fn find_first_kind_before(&self, kind: SyntaxKind, start: usize, end: usize) -> Option<usize> {
        (start..end).find(|cursor| self.kind_at(*cursor) == Some(kind))
    }

    fn find_root_kind_before(&self, kind: SyntaxKind, start: usize, end: usize) -> Option<usize> {
        let mut depth = DelimiterDepth::default();
        for cursor in start..end {
            let Some(current) = self.kind_at(cursor) else {
                break;
            };
            if depth.is_root() && current == kind {
                return Some(cursor);
            }
            depth.bump(current);
        }
        None
    }

    fn find_statement_term_end(&self, start: usize, end: usize) -> usize {
        let mut depth = DelimiterDepth::default();
        for cursor in start..end {
            let Some(current) = self.kind_at(cursor) else {
                break;
            };
            if depth.is_root() {
                if current == SyntaxKind::Semicolon {
                    return cursor + 1;
                }
                if current.is_trivia() && self.tokens[cursor].text.contains('\n') {
                    return cursor;
                }
            }
            depth.bump(current);
        }
        end
    }

    fn find_statement_end(&self, kind: SyntaxKind, start: usize, end: usize) -> usize {
        match kind {
            SyntaxKind::ForStmt => self
                .find_root_kind_before(SyntaxKind::InKw, start, end)
                .and_then(|in_kw| self.find_for_body_start(in_kw + 1, end))
                .map(|body| self.find_matching_brace_end(body).min(end))
                .unwrap_or_else(|| self.find_statement_term_end(start, end)),
            SyntaxKind::IfExpr => self.find_if_expression_end(start, end),
            SyntaxKind::MatchExpr => self
                .find_root_kind_before(SyntaxKind::LBrace, start, end)
                .map(|body| self.find_matching_brace_end(body).min(end))
                .unwrap_or_else(|| self.find_statement_term_end(start, end)),
            _ => self.find_statement_term_end(start, end),
        }
    }

    fn find_if_expression_end(&self, start: usize, end: usize) -> usize {
        let Some(body_start) = self.find_root_kind_before(SyntaxKind::LBrace, start, end) else {
            return self.find_statement_term_end(start, end);
        };
        let body_end = self.find_matching_brace_end(body_start).min(end);
        let else_start = self.skip_trivia(body_end);
        if else_start >= end || !self.at_kind(else_start, SyntaxKind::ElseKw) {
            return body_end;
        }

        let else_body = self.skip_trivia(else_start + 1);
        if else_body >= end {
            return else_start + 1;
        }
        if self.at_kind(else_body, SyntaxKind::IfKw) {
            self.find_if_expression_end(else_body, end)
        } else if self.at_kind(else_body, SyntaxKind::LBrace) {
            self.find_matching_brace_end(else_body).min(end)
        } else {
            self.find_statement_term_end(else_body, end)
        }
    }

    fn find_for_body_start(&self, start: usize, end: usize) -> Option<usize> {
        let mut depth = DelimiterDepth::default();
        for cursor in start..end {
            let Some(current) = self.kind_at(cursor) else {
                break;
            };
            if depth.is_root() && current == SyntaxKind::LBrace {
                let body_end = self.find_matching_brace_end(cursor).min(end);
                let next = self.skip_trivia(body_end);
                if next >= end || self.at_explicit_statement_start(next) {
                    return Some(cursor);
                }
            }
            depth.bump(current);
        }
        None
    }

    fn at_explicit_statement_start(&self, cursor: usize) -> bool {
        matches!(
            self.kind_at(cursor),
            Some(
                SyntaxKind::LetKw
                    | SyntaxKind::ReturnKw
                    | SyntaxKind::BreakKw
                    | SyntaxKind::ContinueKw
                    | SyntaxKind::ForKw
                    | SyntaxKind::IfKw
                    | SyntaxKind::MatchKw
            )
        )
    }

    fn find_root_newline_before(&self, start: usize, end: usize) -> Option<usize> {
        let mut depth = DelimiterDepth::default();
        for cursor in start..end {
            let Some(current) = self.kind_at(cursor) else {
                break;
            };
            if depth.is_root() && current.is_trivia() && self.tokens[cursor].text.contains('\n') {
                return Some(cursor);
            }
            depth.bump(current);
        }
        None
    }

    fn find_matching_delimiter_end(
        &self,
        open: usize,
        open_kind: SyntaxKind,
        close_kind: SyntaxKind,
    ) -> Option<usize> {
        if self.kind_at(open) != Some(open_kind) {
            return None;
        }

        let mut cursor = open;
        let mut depth = 0_u32;
        while let Some(kind) = self.kind_at(cursor) {
            if kind == open_kind {
                depth = depth.saturating_add(1);
            } else if kind == close_kind {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(cursor + 1);
                }
            } else if kind == SyntaxKind::Eof {
                return None;
            }
            cursor += 1;
        }
        None
    }

    fn emit_until(&mut self, end: usize) {
        while self.pos < end {
            self.emit_current_token();
        }
    }

    fn emit_tokens(&mut self, start: usize, end: usize) {
        for token in &self.tokens[start..end] {
            if token.kind != SyntaxKind::Eof {
                self.builder.token(token.kind, &token.text);
            }
        }
        self.pos = end;
    }

    fn node_range(&mut self, kind: SyntaxKind, start: usize, end: usize) {
        self.builder.start_node(kind);
        self.emit_tokens(start, end);
        self.builder.finish_node();
    }

    fn has_significant_tokens(&self, start: usize, end: usize) -> bool {
        self.tokens[start..end]
            .iter()
            .any(|token| !token.kind.is_trivia() && token.kind != SyntaxKind::Eof)
    }

    fn range_is_at_delimiter_root(&self, start: usize, end: usize) -> bool {
        let mut depth = DelimiterDepth::default();
        for token in &self.tokens[start..end] {
            depth.bump(token.kind);
        }
        depth.is_root()
    }

    fn error_run(&mut self) {
        let start = self.pos;
        while !self.at_eof() {
            if self.current_kind().is_some_and(SyntaxKind::is_trivia) {
                break;
            }
            if self.pos != start && self.current_item().is_some() {
                break;
            }
            self.pos += 1;
        }

        if self.pos == start {
            self.emit_current_token();
            return;
        }

        if let Some(span) = self.tokens.get(start).map(|token| token.span) {
            self.diagnostics.push(
                Diagnostic::error("expected item")
                    .with_code("E_PARSE")
                    .with_span(span),
            );
        }

        self.builder.start_node(SyntaxKind::Error);
        for token in &self.tokens[start..self.pos] {
            self.builder.token(token.kind, &token.text);
        }
        self.builder.finish_node();
    }

    fn current_item(&self) -> Option<ItemBoundary> {
        self.item_boundary_at(self.pos)
    }

    fn item_boundary_at(&self, start: usize) -> Option<ItemBoundary> {
        let mut cursor = start;
        loop {
            cursor = self.skip_trivia(cursor);
            if self.at_attribute_start(cursor) {
                cursor = self.skip_attribute(cursor);
                continue;
            }
            break;
        }

        cursor = self.skip_trivia(cursor);
        if self.at_kind(cursor, SyntaxKind::PubKw) {
            cursor = self.skip_trivia(cursor + 1);
        }

        let kind = match self.kind_at(cursor)? {
            SyntaxKind::UseKw => SyntaxKind::UseItem,
            SyntaxKind::ConstKw => SyntaxKind::ConstItem,
            SyntaxKind::GlobalKw => SyntaxKind::GlobalItem,
            SyntaxKind::FnKw => SyntaxKind::FunctionItem,
            SyntaxKind::StructKw => SyntaxKind::StructItem,
            SyntaxKind::EnumKw => SyntaxKind::EnumItem,
            SyntaxKind::TraitKw => SyntaxKind::TraitItem,
            SyntaxKind::ImplKw => SyntaxKind::ImplItem,
            _ => return None,
        };
        let end = self.find_item_end(kind, cursor);
        Some(ItemBoundary { kind, end })
    }

    fn find_item_end(&self, kind: SyntaxKind, keyword_pos: usize) -> usize {
        match kind {
            SyntaxKind::UseItem | SyntaxKind::GlobalItem | SyntaxKind::ConstItem => {
                self.find_semicolon_item_end(keyword_pos)
            }
            SyntaxKind::FunctionItem
            | SyntaxKind::StructItem
            | SyntaxKind::EnumItem
            | SyntaxKind::TraitItem
            | SyntaxKind::ImplItem => self.find_braced_item_end(keyword_pos),
            _ => keyword_pos.saturating_add(1),
        }
    }

    fn find_semicolon_item_end(&self, start: usize) -> usize {
        let mut cursor = start;
        let mut depth = DelimiterDepth::default();
        while let Some(kind) = self.kind_at(cursor) {
            if kind == SyntaxKind::Eof {
                return cursor;
            }
            if depth.is_root() {
                if kind == SyntaxKind::Semicolon {
                    return cursor + 1;
                }
                if kind.is_trivia()
                    && self.tokens[cursor].text.contains('\n')
                    && self.next_significant_starts_item(cursor + 1)
                {
                    return cursor;
                }
            }
            depth.bump(kind);
            cursor += 1;
        }
        self.tokens.len()
    }

    fn find_braced_item_end(&self, start: usize) -> usize {
        let mut cursor = start;
        while let Some(kind) = self.kind_at(cursor) {
            if kind == SyntaxKind::Eof {
                return cursor;
            }
            if kind == SyntaxKind::LBrace {
                return self.find_matching_brace_end(cursor);
            }
            cursor += 1;
        }
        self.tokens.len()
    }

    fn find_matching_brace_end(&self, open_brace: usize) -> usize {
        let mut cursor = open_brace;
        let mut depth = 0_u32;
        while let Some(kind) = self.kind_at(cursor) {
            match kind {
                SyntaxKind::LBrace => depth = depth.saturating_add(1),
                SyntaxKind::RBrace => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return cursor + 1;
                    }
                }
                SyntaxKind::Eof => return cursor,
                _ => {}
            }
            cursor += 1;
        }
        self.tokens.len()
    }

    fn skip_attribute(&self, hash: usize) -> usize {
        let mut cursor = self.skip_trivia(hash + 1);
        let mut bracket_depth = 0_u32;
        while let Some(kind) = self.kind_at(cursor) {
            match kind {
                SyntaxKind::LBracket => bracket_depth = bracket_depth.saturating_add(1),
                SyntaxKind::RBracket => {
                    bracket_depth = bracket_depth.saturating_sub(1);
                    cursor += 1;
                    if bracket_depth == 0 {
                        return cursor;
                    }
                    continue;
                }
                SyntaxKind::Eof => return cursor,
                _ => {}
            }
            cursor += 1;
        }
        self.tokens.len()
    }

    fn at_attribute_start(&self, hash: usize) -> bool {
        self.at_kind(hash, SyntaxKind::Hash)
            && self.at_kind(self.skip_trivia(hash + 1), SyntaxKind::LBracket)
    }

    fn next_significant_starts_item(&self, cursor: usize) -> bool {
        let next = self.skip_trivia(cursor);
        self.item_boundary_at(next).is_some()
    }

    fn next_significant_before(&self, cursor: usize, end: usize) -> Option<usize> {
        let next = self.skip_trivia(cursor);
        (next < end).then_some(next)
    }

    fn member_range_has_name(&self, start: usize, end: usize) -> bool {
        self.member_name_end(start, end) > start
    }

    fn member_name_end(&self, start: usize, end: usize) -> usize {
        let mut cursor = start;
        loop {
            cursor = self.skip_trivia(cursor);
            if cursor >= end {
                return start;
            }
            if self.at_attribute_start(cursor) {
                cursor = self.skip_attribute(cursor);
                continue;
            }
            return if self.at_kind(cursor, SyntaxKind::Ident) {
                cursor + 1
            } else {
                start
            };
        }
    }

    fn statement_kind_at(&self, start: usize, end: usize) -> Option<SyntaxKind> {
        let mut cursor = start;
        loop {
            cursor = self.skip_trivia(cursor);
            if cursor >= end {
                return None;
            }
            if self.at_attribute_start(cursor) {
                cursor = self.skip_attribute(cursor);
                continue;
            }
            break;
        }

        Some(match self.kind_at(cursor)? {
            SyntaxKind::LetKw => SyntaxKind::LetStmt,
            SyntaxKind::ReturnKw => SyntaxKind::ReturnStmt,
            SyntaxKind::BreakKw => SyntaxKind::BreakStmt,
            SyntaxKind::ContinueKw => SyntaxKind::ContinueStmt,
            SyntaxKind::ForKw => SyntaxKind::ForStmt,
            SyntaxKind::IfKw => SyntaxKind::IfExpr,
            SyntaxKind::MatchKw => SyntaxKind::MatchExpr,
            _ => SyntaxKind::ExprStmt,
        })
    }

    fn method_keyword_pos(&self, start: usize, end: usize) -> Option<usize> {
        let mut cursor = start;
        loop {
            cursor = self.skip_trivia(cursor);
            if cursor >= end {
                return None;
            }
            if self.at_attribute_start(cursor) {
                cursor = self.skip_attribute(cursor);
                continue;
            }
            return self.at_kind(cursor, SyntaxKind::FnKw).then_some(cursor);
        }
    }

    fn find_method_end(&self, start: usize, end: usize) -> usize {
        let Some(signature_start) = self.method_keyword_pos(start, end) else {
            return start.saturating_add(1).min(end);
        };
        let param_list_end = self
            .find_first_kind_before(SyntaxKind::LParen, signature_start, end)
            .and_then(|start| {
                self.find_matching_delimiter_end(start, SyntaxKind::LParen, SyntaxKind::RParen)
            })
            .unwrap_or(signature_start);
        let body = self.find_root_kind_before(SyntaxKind::LBrace, param_list_end, end);
        let semicolon = self.find_root_kind_before(SyntaxKind::Semicolon, param_list_end, end);

        match (body, semicolon) {
            (Some(body_start), Some(semicolon_pos)) if body_start < semicolon_pos => {
                self.find_matching_brace_end(body_start).min(end)
            }
            (Some(body_start), None) => self.find_matching_brace_end(body_start).min(end),
            (_, Some(semicolon_pos)) => semicolon_pos.saturating_add(1).min(end),
            (None, None) => self
                .find_root_newline_before(param_list_end, end)
                .unwrap_or(end),
        }
    }

    fn skip_trivia(&self, mut cursor: usize) -> usize {
        while self.kind_at(cursor).is_some_and(SyntaxKind::is_trivia) {
            cursor += 1;
        }
        cursor
    }

    fn trim_trailing_trivia(&self, start: usize, mut end: usize) -> usize {
        while end > start
            && self
                .kind_at(end.saturating_sub(1))
                .is_some_and(SyntaxKind::is_trivia)
        {
            end = end.saturating_sub(1);
        }
        end
    }

    fn emit_current_token(&mut self) {
        if let Some(token) = self.tokens.get(self.pos) {
            if token.kind != SyntaxKind::Eof {
                self.builder.token(token.kind, &token.text);
            }
            self.pos += 1;
        }
    }

    fn at_eof(&self) -> bool {
        self.current_kind()
            .is_none_or(|kind| kind == SyntaxKind::Eof)
    }

    fn current_kind(&self) -> Option<SyntaxKind> {
        self.kind_at(self.pos)
    }

    fn current_token_text_contains(&self, needle: char) -> bool {
        self.tokens
            .get(self.pos)
            .is_some_and(|token| token.text.contains(needle))
    }

    fn at_kind(&self, cursor: usize, kind: SyntaxKind) -> bool {
        self.kind_at(cursor) == Some(kind)
    }

    fn kind_at(&self, cursor: usize) -> Option<SyntaxKind> {
        self.tokens.get(cursor).map(|token| token.kind)
    }
}

#[derive(Clone, Copy)]
struct ItemBoundary {
    kind: SyntaxKind,
    end: usize,
}

#[derive(Default)]
struct DelimiterDepth {
    paren: u32,
    bracket: u32,
    brace: u32,
}

impl DelimiterDepth {
    fn is_root(&self) -> bool {
        self.paren == 0 && self.bracket == 0 && self.brace == 0
    }

    fn bump(&mut self, kind: SyntaxKind) {
        match kind {
            SyntaxKind::LParen => self.paren = self.paren.saturating_add(1),
            SyntaxKind::RParen => self.paren = self.paren.saturating_sub(1),
            SyntaxKind::LBracket => self.bracket = self.bracket.saturating_add(1),
            SyntaxKind::RBracket => self.bracket = self.bracket.saturating_sub(1),
            SyntaxKind::LBrace => self.brace = self.brace.saturating_add(1),
            SyntaxKind::RBrace => self.brace = self.brace.saturating_sub(1),
            _ => {}
        }
    }
}
