use super::{
    CompletionInsertFormat, CompletionItem, CompletionKind, relevance::completion_sort_text,
};
use crate::DisplayParts;

pub(super) fn statement_keyword_completions(prefix: &str) -> Vec<CompletionItem> {
    [
        (
            "for in",
            CompletionKind::Snippet,
            "for-in loop",
            "for ${1:item} in ${2:items} {\n    $0\n}",
            CompletionInsertFormat::Snippet,
        ),
        (
            "match",
            CompletionKind::Snippet,
            "match expression",
            "match ${1:value} {\n    $0\n}",
            CompletionInsertFormat::Snippet,
        ),
        (
            "let",
            CompletionKind::Keyword,
            "local binding",
            "let ",
            CompletionInsertFormat::PlainText,
        ),
        (
            "return",
            CompletionKind::Keyword,
            "return statement",
            "return ",
            CompletionInsertFormat::PlainText,
        ),
        (
            "for",
            CompletionKind::Keyword,
            "for loop",
            "for ",
            CompletionInsertFormat::PlainText,
        ),
        (
            "if",
            CompletionKind::Keyword,
            "if expression",
            "if ",
            CompletionInsertFormat::PlainText,
        ),
        (
            "break",
            CompletionKind::Keyword,
            "break statement",
            "break",
            CompletionInsertFormat::PlainText,
        ),
        (
            "continue",
            CompletionKind::Keyword,
            "continue statement",
            "continue",
            CompletionInsertFormat::PlainText,
        ),
    ]
    .into_iter()
    .filter(|(label, _, _, _, _)| label.starts_with(prefix))
    .map(|(label, kind, detail, insert_text, insert_format)| {
        let detail_parts = DisplayParts::plain(detail);
        CompletionItem {
            label: label.to_owned(),
            kind,
            detail: detail_parts.render(),
            insert_text: Some(insert_text.to_owned()),
            insert_format,
            sort_text: Some(completion_sort_text(kind, label, "")),
            metadata: Default::default(),
        }
        .with_detail_parts(detail_parts)
    })
    .collect()
}
