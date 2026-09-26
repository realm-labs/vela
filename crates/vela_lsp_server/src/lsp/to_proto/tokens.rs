use vela_language_service::{Position, SemanticToken, SemanticTokenDelta, SemanticTokens};

use crate::{line_index::LineIndex, semantic_tokens::SemanticTokenProjection};

pub(crate) fn full(
    tokens: &SemanticTokens,
    projection: &SemanticTokenProjection,
    text: &str,
) -> Result<lsp_types::SemanticTokensResult, String> {
    Ok(lsp_types::SemanticTokensResult::Tokens(
        lsp_types::SemanticTokens {
            result_id: Some(tokens.result_id().to_owned()),
            data: data(tokens.tokens(), projection, &LineIndex::new(text))?,
        },
    ))
}

pub(crate) fn range(
    tokens: &SemanticTokens,
    projection: &SemanticTokenProjection,
    text: &str,
) -> Result<lsp_types::SemanticTokensRangeResult, String> {
    Ok(lsp_types::SemanticTokensRangeResult::Tokens(
        lsp_types::SemanticTokens {
            result_id: Some(tokens.result_id().to_owned()),
            data: data(tokens.tokens(), projection, &LineIndex::new(text))?,
        },
    ))
}

pub(crate) fn delta(
    delta: &SemanticTokenDelta,
    projection: &SemanticTokenProjection,
    text: &str,
) -> Result<lsp_types::SemanticTokensFullDeltaResult, String> {
    let index = LineIndex::new(text);
    Ok(lsp_types::SemanticTokensFullDeltaResult::TokensDelta(
        lsp_types::SemanticTokensDelta {
            result_id: Some(delta.result_id().to_owned()),
            edits: delta
                .edits()
                .iter()
                .map(|edit| {
                    Ok(lsp_types::SemanticTokensEdit {
                        start: encoded_units(edit.start())?,
                        delete_count: encoded_units(edit.delete_count())?,
                        data: Some(data(edit.tokens(), projection, &index)?),
                    })
                })
                .collect::<Result<_, String>>()?,
        },
    ))
}

fn encoded_units(count: usize) -> Result<u32, String> {
    count
        .checked_mul(5)
        .and_then(|count| u32::try_from(count).ok())
        .ok_or_else(|| "semantic token edit is too large".to_owned())
}

fn data(
    tokens: &[SemanticToken],
    projection: &SemanticTokenProjection,
    index: &LineIndex<'_>,
) -> Result<Vec<lsp_types::SemanticToken>, String> {
    let mut data = Vec::with_capacity(tokens.len());
    let mut previous_line = 0;
    let mut previous_start = 0;
    for token in tokens {
        let start = index.lsp_position(token.start())?;
        let end_column = token
            .start()
            .character
            .checked_add(token.length())
            .ok_or_else(|| "semantic token end is too large".to_owned())?;
        let end = index.lsp_position(Position::new(token.start().line, end_column))?;
        let length = end
            .character
            .checked_sub(start.character)
            .filter(|length| *length > 0)
            .ok_or_else(|| "semantic token must be nonempty".to_owned())?;
        let delta_line = start
            .line
            .checked_sub(previous_line)
            .ok_or_else(|| "semantic tokens are out of order".to_owned())?;
        let delta_start = if delta_line == 0 {
            start
                .character
                .checked_sub(previous_start)
                .ok_or_else(|| "semantic tokens are out of order".to_owned())?
        } else {
            start.character
        };
        data.push(lsp_types::SemanticToken {
            delta_line,
            delta_start,
            length,
            token_type: projection.token_type_index(token.token_type()),
            token_modifiers_bitset: projection.modifier_bits(token.modifiers()),
        });
        previous_line = start.line;
        previous_start = start.character;
    }
    Ok(data)
}
