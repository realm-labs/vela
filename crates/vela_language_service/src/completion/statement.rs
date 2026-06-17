use super::{CompletionInsertFormat, CompletionItem, CompletionKind};

pub(super) fn statement_keyword_completions(prefix: &str) -> Vec<CompletionItem> {
    [
        ("let", "local binding", "let "),
        ("return", "return statement", "return "),
        ("for", "for loop", "for "),
        ("if", "if expression", "if "),
        ("match", "match expression", "match "),
        ("break", "break statement", "break"),
        ("continue", "continue statement", "continue"),
    ]
    .into_iter()
    .filter(|(label, _, _)| label.starts_with(prefix))
    .map(|(label, detail, insert_text)| CompletionItem {
        label: label.to_owned(),
        kind: CompletionKind::Keyword,
        detail: detail.to_owned(),
        insert_text: Some(insert_text.to_owned()),
        insert_format: CompletionInsertFormat::PlainText,
        sort_text: Some(keyword_sort_text(label)),
    })
    .collect()
}

fn keyword_sort_text(label: &str) -> String {
    format!("0000_00_{label}")
}
