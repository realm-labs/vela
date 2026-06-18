use super::{
    CompletionInsertFormat, CompletionItem, CompletionKind, relevance::completion_sort_text,
};
use crate::DisplayParts;

pub(super) fn struct_field_completion_items(prefix: &str) -> Vec<CompletionItem> {
    [
        ("field", "struct field", "${1:name}: ${2:Type}"),
        (
            "field default",
            "struct field with default",
            "${1:name}: ${2:Type} = ${3:value}",
        ),
    ]
    .into_iter()
    .filter(|(label, _, _)| label.starts_with(prefix))
    .map(|(label, detail, insert_text)| {
        let detail_parts = DisplayParts::plain(detail);
        CompletionItem {
            label: label.to_owned(),
            kind: CompletionKind::Snippet,
            detail: detail_parts.render(),
            insert_text: Some(insert_text.to_owned()),
            insert_format: CompletionInsertFormat::Snippet,
            sort_text: Some(completion_sort_text(CompletionKind::Snippet, label, "")),
            metadata: Default::default(),
        }
        .with_detail_parts(detail_parts)
    })
    .collect()
}
