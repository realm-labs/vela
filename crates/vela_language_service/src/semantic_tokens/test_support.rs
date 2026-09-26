use crate::matrix_fixture::{Document, semantic_tokens as oracle};
use crate::{SemanticToken, SemanticTokenModifiers};

pub(super) fn rows(document: &Document, tokens: &[SemanticToken]) -> Vec<oracle::Row> {
    tokens
        .iter()
        .map(|token| {
            oracle::row(
                document,
                (token.start().line, token.start().character, token.length()),
                token.token_type().as_str(),
                SemanticTokenModifiers::LEGEND
                    .iter()
                    .enumerate()
                    .filter(|(bit, _)| token.modifiers().bits() & (1 << bit) != 0)
                    .map(|(_, name)| (*name).to_owned())
                    .collect(),
                false,
            )
        })
        .collect()
}

mod scenarios;
pub(super) use scenarios::assert_fixture;
