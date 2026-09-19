use super::{CstParser, DelimiterDepth};
use crate::SyntaxKind;

impl CstParser<'_, '_> {
    pub(super) fn record_expression_body(&mut self, start: usize, end: usize) {
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
        let close = self.braced_contents_end(start, fields_end);
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

    pub(super) fn record_pattern_body(&mut self, start: usize, end: usize) {
        let Some(fields_start) = self.find_outer_record_field_list_start(start, end) else {
            self.emit_until(end);
            return;
        };
        self.emit_until(fields_start + 1);
        let close = self.braced_contents_end(fields_start, end);
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

    pub(super) fn find_outer_record_field_list_start(
        &self,
        start: usize,
        end: usize,
    ) -> Option<usize> {
        let mut depth = DelimiterDepth::default();
        for cursor in start..end {
            let Some(current) = self.kind_at(cursor) else {
                break;
            };
            if depth.is_root()
                && current == SyntaxKind::LBrace
                && cursor > start
                && self
                    .find_matching_delimiter_end(cursor, SyntaxKind::LBrace, SyntaxKind::RBrace)
                    .is_none_or(|close| close == end)
            {
                return Some(cursor);
            }
            depth.bump(current);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ast::{AstNode, SyntaxRecordExpr},
        parse::parse_source,
    };

    #[test]
    fn recovered_record_fields_preserve_labels_values_and_delimiters() {
        for (expression, label, value, closed) in [
            ("Box {", None, None, false),
            ("Box { item", Some("item"), None, false),
            ("Box { item:", Some("item"), None, false),
            ("Box { item: 1", Some("item"), Some("1"), false),
            ("Box { item: 1 }", Some("item"), Some("1"), true),
        ] {
            for newline in ["\n", "\r\n"] {
                let source = format!("fn run() {{{newline}/* 中😀 */ {expression}");
                let parsed = parse_source(&source);
                assert_eq!(parsed.tree().syntax().text().to_string(), source);
                let record = parsed
                    .tree()
                    .syntax()
                    .descendants()
                    .find_map(SyntaxRecordExpr::cast)
                    .expect("record after recovery");
                assert_eq!(record.path_text().as_deref(), Some("Box"));
                assert_eq!(record.r_brace_token().is_some(), closed, "{expression}");
                let fields = record.fields();
                assert_eq!(fields.len(), usize::from(label.is_some()));
                if let Some(field) = fields.first() {
                    assert_eq!(field.label_text().as_deref(), label);
                    assert_eq!(
                        field
                            .expression()
                            .map(|value| value.syntax().text().to_string())
                            .as_deref(),
                        value
                    );
                    assert_eq!(field.is_shorthand(), expression == "Box { item");
                }
            }
        }
    }

    #[test]
    fn incomplete_record_patterns_keep_field_labels_and_binding_tokens() {
        use crate::ast::SyntaxRecordPattern;
        for tail in ["item:", "item: bound"] {
            for newline in ["\n", "\r\n"] {
                let source =
                    format!("fn run(value) {{{newline}match value {{ Choice::Record {{ {tail}");
                let parsed = parse_source(&source);
                assert_eq!(parsed.tree().syntax().text().to_string(), source);
                let pattern = parsed
                    .tree()
                    .syntax()
                    .descendants()
                    .find_map(SyntaxRecordPattern::cast)
                    .expect("record pattern after recovery");
                assert_eq!(pattern.path_text().as_deref(), Some("Choice::Record"));
                assert!(pattern.r_brace_token().is_none());
                let fields = pattern.fields().collect::<Vec<_>>();
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].label_text().as_deref(), Some("item"));
                assert!(fields[0].colon_token().is_some());
                assert!(!fields[0].is_shorthand());
                assert_eq!(
                    fields[0]
                        .pattern()
                        .map(|pattern| pattern.syntax().text().to_string())
                        .as_deref(),
                    if tail.ends_with("bound") {
                        Some("bound")
                    } else {
                        None
                    }
                );
            }
        }
    }

    #[test]
    fn incomplete_outer_record_keeps_the_inner_records_closing_delimiter() {
        let source = "fn run() { Box { item: Cell { value: 1 }";
        let parsed = parse_source(source);
        assert_eq!(parsed.tree().syntax().text().to_string(), source);
        let records = parsed
            .tree()
            .syntax()
            .descendants()
            .filter_map(SyntaxRecordExpr::cast)
            .collect::<Vec<_>>();
        assert_eq!(records.len(), 2);
        assert!(records[0].r_brace_token().is_none());
        assert!(records[1].r_brace_token().is_some());
        assert_eq!(
            records[0].fields()[0]
                .expression()
                .expect("inner")
                .syntax()
                .text(),
            "Cell { value: 1 }"
        );
        assert_eq!(
            records[1].fields()[0].label_text().as_deref(),
            Some("value")
        );
    }
}
