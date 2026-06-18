use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value as JsonValue, json};
use vela_language_service::{
    SemanticToken, SemanticTokenDelta, SemanticTokenModifiers, SemanticTokenType, SemanticTokens,
};

use crate::protocol::{LspPosition, LspRange};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct SemanticTokenProjection {
    token_types: Vec<&'static str>,
    token_type_indices: Vec<u32>,
    token_modifiers: Vec<&'static str>,
    modifier_bits: Vec<Option<u32>>,
}

impl Default for SemanticTokenProjection {
    fn default() -> Self {
        Self::for_client(None, None)
    }
}

impl SemanticTokenProjection {
    pub(crate) fn for_client(
        token_types: Option<&[String]>,
        token_modifiers: Option<&[String]>,
    ) -> Self {
        let supported_types = token_types.map(|types| types.iter().map(String::as_str).collect());
        let supported_modifiers =
            token_modifiers.map(|modifiers| modifiers.iter().map(String::as_str).collect());
        let (token_types, token_type_indices) = projected_token_types(supported_types.as_ref());
        let (token_modifiers, modifier_bits) = projected_modifiers(supported_modifiers.as_ref());

        Self {
            token_types,
            token_type_indices,
            token_modifiers,
            modifier_bits,
        }
    }

    fn token_type_index(&self, token_type: SemanticTokenType) -> u32 {
        let service_index = usize::try_from(token_type.legend_index())
            .expect("semantic token legend index should fit usize");
        self.token_type_indices[service_index]
    }

    fn modifier_bits(&self, modifiers: SemanticTokenModifiers) -> u32 {
        let service_bits = modifiers.bits();
        self.modifier_bits
            .iter()
            .enumerate()
            .filter_map(|(index, projected)| {
                let service_bit = 1_u32 << index;
                (service_bits & service_bit != 0)
                    .then_some(*projected)
                    .flatten()
            })
            .fold(0, |bits, projected| bits | projected)
    }
}

pub(crate) fn lsp_semantic_tokens(
    tokens: &SemanticTokens,
    projection: &SemanticTokenProjection,
) -> JsonValue {
    json!({
        "resultId": tokens.result_id(),
        "data": semantic_token_data(tokens.tokens(), projection)
    })
}

pub(crate) fn lsp_semantic_tokens_range(
    tokens: &SemanticTokens,
    range: LspRange,
    projection: &SemanticTokenProjection,
) -> JsonValue {
    let filtered = tokens
        .tokens()
        .iter()
        .copied()
        .filter(|token| token_overlaps_range(*token, range))
        .collect::<Vec<_>>();
    lsp_semantic_tokens(&SemanticTokens::new(filtered), projection)
}

pub(crate) fn lsp_semantic_token_delta(
    delta: &SemanticTokenDelta,
    projection: &SemanticTokenProjection,
) -> JsonValue {
    json!({
        "resultId": delta.result_id(),
        "edits": delta.edits()
            .iter()
            .map(|edit| {
                json!({
                    "start": edit.start() * 5,
                    "deleteCount": edit.delete_count() * 5,
                    "data": semantic_token_data(edit.tokens(), projection)
                })
            })
            .collect::<Vec<_>>()
    })
}

fn semantic_token_data(tokens: &[SemanticToken], projection: &SemanticTokenProjection) -> Vec<u32> {
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
        data.push(projection.token_type_index(token.token_type()));
        data.push(projection.modifier_bits(token.modifiers()));
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

pub(crate) fn semantic_tokens_legend(projection: &SemanticTokenProjection) -> JsonValue {
    json!({
        "tokenTypes": projection.token_types,
        "tokenModifiers": projection.token_modifiers
    })
}

fn projected_token_types(supported: Option<&BTreeSet<&str>>) -> (Vec<&'static str>, Vec<u32>) {
    let mut names = Vec::new();
    let mut indexes_by_name = BTreeMap::new();
    let mut indices = Vec::with_capacity(SemanticTokenType::LEGEND.len());

    for token_type in SemanticTokenType::LEGEND {
        let name = projected_token_type_name(token_type, supported);
        let next_index = u32::try_from(names.len()).expect("semantic token legend should fit u32");
        let index = *indexes_by_name.entry(name).or_insert_with(|| {
            names.push(name);
            next_index
        });
        indices.push(index);
    }

    (names, indices)
}

fn projected_token_type_name(
    token_type: SemanticTokenType,
    supported: Option<&BTreeSet<&str>>,
) -> &'static str {
    let primary = token_type.as_str();
    supported.map_or(primary, |supported| {
        if supported.contains(primary) {
            primary
        } else {
            token_type.standard_fallback()
        }
    })
}

fn projected_modifiers(
    supported: Option<&BTreeSet<&str>>,
) -> (Vec<&'static str>, Vec<Option<u32>>) {
    let mut names = Vec::new();
    let mut indexes_by_name = BTreeMap::new();
    let mut bits = Vec::with_capacity(SemanticTokenModifiers::LEGEND.len());

    for (index, name) in SemanticTokenModifiers::LEGEND.iter().copied().enumerate() {
        let name = projected_modifier_name(index, name, supported);
        let Some(name) = name else {
            bits.push(None);
            continue;
        };
        let next_index =
            u32::try_from(names.len()).expect("semantic modifier legend should fit u32");
        let index = *indexes_by_name.entry(name).or_insert_with(|| {
            names.push(name);
            next_index
        });
        bits.push(Some(1_u32 << index));
    }

    (names, bits)
}

fn projected_modifier_name(
    index: usize,
    name: &'static str,
    supported: Option<&BTreeSet<&str>>,
) -> Option<&'static str> {
    supported.map_or(Some(name), |supported| {
        if supported.contains(name) {
            Some(name)
        } else {
            SemanticTokenModifiers::FALLBACKS[index].filter(|fallback| supported.contains(fallback))
        }
    })
}
