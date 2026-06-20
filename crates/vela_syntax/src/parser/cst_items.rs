use super::CstParser;
use crate::SyntaxKind;

impl CstParser<'_, '_> {
    pub(super) fn item(&mut self, kind: SyntaxKind, end: usize) {
        match kind {
            SyntaxKind::UseItem => self.use_item(end),
            SyntaxKind::ConstItem => self.const_item(end),
            SyntaxKind::GlobalItem => self.global_item(end),
            SyntaxKind::FunctionItem => self.function_item(end),
            SyntaxKind::StructItem => self.struct_item(end),
            SyntaxKind::EnumItem => self.enum_item(end),
            SyntaxKind::TraitItem => self.trait_item(end),
            SyntaxKind::ImplItem => self.impl_item(end),
            _ => self.raw_item(kind, end),
        }
    }

    fn raw_item(&mut self, kind: SyntaxKind, end: usize) {
        self.builder.start_node(kind);
        while self.pos < end {
            self.emit_current_token();
        }
        self.builder.finish_node();
    }

    fn use_item(&mut self, end: usize) {
        self.builder.start_node(SyntaxKind::UseItem);
        if let Some(keyword) = self.find_first_kind_before(SyntaxKind::UseKw, self.pos, end) {
            let path_start = self.skip_trivia(keyword + 1);
            let path_end = self
                .find_root_kind_before(SyntaxKind::AsKw, path_start, end)
                .or_else(|| self.find_root_kind_before(SyntaxKind::Semicolon, path_start, end))
                .unwrap_or(end);
            let path_end = self.trim_trailing_trivia(path_start, path_end);
            if path_start < path_end {
                self.emit_until(path_start);
                self.use_path(path_start, path_end);
            }
        }
        self.emit_until(end);
        self.builder.finish_node();
    }

    fn use_path(&mut self, start: usize, end: usize) {
        self.builder.start_node(SyntaxKind::UsePath);
        self.emit_tokens(start, end);
        self.builder.finish_node();
    }

    fn const_item(&mut self, end: usize) {
        self.builder.start_node(SyntaxKind::ConstItem);
        let initializer = self.find_root_kind_before(SyntaxKind::Equal, self.pos, end);
        let declaration_end = initializer
            .or_else(|| self.find_root_kind_before(SyntaxKind::Semicolon, self.pos, end))
            .unwrap_or(end);
        self.optional_type_hint_before(self.pos, declaration_end);
        if let Some(equal) = initializer {
            let value_start = self.skip_trivia(equal + 1);
            let value_end = self.statement_expression_end(value_start, end);
            self.emit_until(value_start);
            self.expression_range(value_start, value_end);
        }
        self.emit_until(end);
        self.builder.finish_node();
    }

    fn global_item(&mut self, end: usize) {
        self.builder.start_node(SyntaxKind::GlobalItem);
        let initializer = self.find_root_kind_before(SyntaxKind::Equal, self.pos, end);
        let declaration_end = initializer
            .or_else(|| self.find_root_kind_before(SyntaxKind::Semicolon, self.pos, end))
            .unwrap_or(end);
        self.optional_type_hint_before(self.pos, declaration_end);
        if let Some(equal) = initializer {
            let value_start = self.skip_trivia(equal + 1);
            let value_end = self.statement_expression_end(value_start, end);
            self.emit_until(value_start);
            self.expression_range(value_start, value_end);
        }
        self.emit_until(end);
        self.builder.finish_node();
    }

    fn optional_type_hint_before(&mut self, start: usize, end: usize) {
        if let Some(colon) = self.find_root_kind_before(SyntaxKind::Colon, start, end) {
            let type_start = self.skip_trivia(colon + 1);
            let type_end = self.trim_trailing_trivia(type_start, end);
            if type_start < type_end {
                self.emit_until(type_start);
                self.type_hint_range(type_start, type_end);
            }
        }
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

    pub(super) fn param_range(&mut self, start: usize, end: usize) {
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
        let declaration_end = self
            .find_root_kind_before(SyntaxKind::Equal, start, end)
            .unwrap_or(end);
        self.optional_type_hint_before(start, declaration_end);
        self.emit_until(end);
        self.builder.finish_node();
    }

    pub(super) fn type_hint_range(&mut self, start: usize, end: usize) {
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
}
