use super::{CstParser, DelimiterDepth};
use crate::SyntaxKind;

impl CstParser<'_, '_> {
    pub(super) fn find_function_item_end(&self, start: usize) -> usize {
        let parameters = self.find_first_kind_before(SyntaxKind::LParen, start, self.tokens.len());
        if parameters.is_some_and(|open| {
            self.find_matching_delimiter_end(open, SyntaxKind::LParen, SyntaxKind::RParen)
                .is_none()
        }) && let Some(body) =
            self.find_first_kind_before(SyntaxKind::LBrace, start, self.tokens.len())
        {
            // function_item already uses this brace as the recovered body when
            // parameters have no closing delimiter. Its unclosed paren must not
            // hide that body boundary and consume later source declarations.
            return self.find_matching_brace_end(body);
        }
        let keyword = if self.at_kind(start, SyntaxKind::AsyncKw) {
            self.skip_trivia(start + 1)
        } else {
            start
        };
        let mut depth = DelimiterDepth::default();
        for cursor in keyword + 1..self.tokens.len() {
            let Some(kind) = self.kind_at(cursor) else {
                break;
            };
            if kind == SyntaxKind::Eof {
                return cursor;
            }
            if depth.is_root() {
                if kind == SyntaxKind::LBrace {
                    return self.find_matching_brace_end(cursor);
                }
                let state_item = self.at_ident_text(cursor, "state")
                    && matches!(
                        self.kind_at(self.skip_trivia(cursor + 1)),
                        Some(SyntaxKind::Ident | SyntaxKind::Colon)
                    );
                if self.at_attribute_start(cursor)
                    || state_item
                    || matches!(
                        kind,
                        SyntaxKind::UseKw
                            | SyntaxKind::ConstKw
                            | SyntaxKind::FnKw
                            | SyntaxKind::StructKw
                            | SyntaxKind::EnumKw
                            | SyntaxKind::TraitKw
                            | SyntaxKind::ImplKw
                            | SyntaxKind::ExternKw
                            | SyntaxKind::PubKw
                            | SyntaxKind::AsyncKw
                    )
                {
                    return self.trim_trailing_trivia(start, cursor);
                }
            }
            depth.bump(kind);
        }
        self.tokens.len()
    }
}
