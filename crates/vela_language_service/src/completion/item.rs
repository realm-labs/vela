use super::{CompletionInsertFormat, CompletionItem, CompletionKind};

pub(super) fn item_keyword_completions(prefix: &str) -> Vec<CompletionItem> {
    [
        ("fn", "function declaration", "fn "),
        ("struct", "struct declaration", "struct "),
        ("enum", "enum declaration", "enum "),
        ("trait", "trait declaration", "trait "),
        ("impl", "implementation block", "impl "),
        ("use", "import declaration", "use "),
        ("const", "constant declaration", "const "),
        ("global", "host-bound global declaration", "global "),
        ("pub", "public visibility", "pub "),
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
