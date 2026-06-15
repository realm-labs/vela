use serde_json::{Value as JsonValue, json};
use vela_language_service::{SemanticTokenType, SemanticTokens};

pub(crate) fn lsp_semantic_tokens(tokens: &SemanticTokens) -> JsonValue {
    let mut data = Vec::with_capacity(tokens.tokens().len() * 5);
    let mut previous_line = 0usize;
    let mut previous_start = 0usize;

    for token in tokens.tokens() {
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

    json!({ "data": data })
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
