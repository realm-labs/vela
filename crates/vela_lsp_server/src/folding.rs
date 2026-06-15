use serde_json::{Value as JsonValue, json};
use vela_language_service::{FoldingRange, FoldingRangeKind};

pub(crate) fn lsp_folding_ranges(ranges: &[FoldingRange]) -> JsonValue {
    JsonValue::Array(ranges.iter().map(lsp_folding_range).collect())
}

fn lsp_folding_range(range: &FoldingRange) -> JsonValue {
    json!({
        "startLine": range.start().line,
        "startCharacter": range.start().character,
        "endLine": range.end().line,
        "endCharacter": range.end().character,
        "kind": lsp_folding_range_kind(range.kind())
    })
}

fn lsp_folding_range_kind(kind: FoldingRangeKind) -> &'static str {
    match kind {
        FoldingRangeKind::Imports => "imports",
        FoldingRangeKind::Region => "region",
    }
}
