use serde_json::{Value as JsonValue, json};
use vela_language_service::{SemanticToken, SemanticTokenDelta, SemanticTokenType, SemanticTokens};

use crate::protocol::{LspPosition, LspRange};

pub(crate) fn lsp_semantic_tokens(tokens: &SemanticTokens) -> JsonValue {
    json!({
        "resultId": tokens.result_id(),
        "data": semantic_token_data(tokens.tokens())
    })
}

pub(crate) fn lsp_semantic_tokens_range(tokens: &SemanticTokens, range: LspRange) -> JsonValue {
    let filtered = tokens
        .tokens()
        .iter()
        .copied()
        .filter(|token| token_overlaps_range(*token, range))
        .collect::<Vec<_>>();
    lsp_semantic_tokens(&SemanticTokens::new(filtered))
}

pub(crate) fn lsp_semantic_token_delta(delta: &SemanticTokenDelta) -> JsonValue {
    json!({
        "resultId": delta.result_id(),
        "edits": delta.edits()
            .iter()
            .map(|edit| {
                json!({
                    "start": edit.start() * 5,
                    "deleteCount": edit.delete_count() * 5,
                    "data": semantic_token_data(edit.tokens())
                })
            })
            .collect::<Vec<_>>()
    })
}

fn semantic_token_data(tokens: &[SemanticToken]) -> Vec<u32> {
    let mut data = Vec::with_capacity(tokens.len() * 5);
    let mut previous_line = 0usize;
    let mut previous_start = 0usize;

    for token in tokens {
        let start = token.start();
        let delta_line = start.line.saturating_sub(previous_line);
        let delta_start = if delta_line == 0 {
            start.character.saturating_sub(previous_start)
        } else {
            start.character
        };
        data.push(u32::try_from(delta_line).expect("semantic token line delta should fit u32"));
        data.push(u32::try_from(delta_start).expect("semantic token start delta should fit u32"));
        data.push(u32::try_from(token.length()).expect("semantic token length should fit u32"));
        data.push(token.token_type().legend_index());
        data.push(token.modifiers().bits());
        previous_line = start.line;
        previous_start = start.character;
    }

    data
}

fn token_overlaps_range(token: SemanticToken, range: LspRange) -> bool {
    let start = token.start();
    let end = LspPosition {
        line: u32::try_from(start.line).unwrap_or(u32::MAX),
        character: u32::try_from(start.character.saturating_add(token.length()))
            .unwrap_or(u32::MAX),
    };
    let start = LspPosition {
        line: u32::try_from(start.line).unwrap_or(u32::MAX),
        character: u32::try_from(start.character).unwrap_or(u32::MAX),
    };
    position_before(start, range.end) && position_before(range.start, end)
}

fn position_before(left: LspPosition, right: LspPosition) -> bool {
    left.line < right.line || (left.line == right.line && left.character < right.character)
}

pub(crate) fn semantic_tokens_legend() -> JsonValue {
    json!({
        "tokenTypes": SemanticTokenType::LEGEND
            .iter()
            .map(|token_type| token_type.as_str())
            .collect::<Vec<_>>(),
        "tokenModifiers": vela_language_service::SemanticTokenModifiers::LEGEND
    })
}
