use vela_analysis::type_fact::TypeFact;

use super::{
    CompletionInsertFormat, CompletionItem, CompletionKind, display_type_detail_parts,
    relevance::completion_sort_text,
};

pub(super) fn builtin_value_completion_items(prefix: &str) -> Vec<CompletionItem> {
    [
        ("false", TypeFact::BOOL),
        ("null", TypeFact::NULL),
        ("true", TypeFact::BOOL),
    ]
    .into_iter()
    .filter(|(label, _)| label.starts_with(prefix))
    .map(|(label, fact)| {
        let detail_parts = display_type_detail_parts(fact.display_name());
        CompletionItem {
            label: label.to_owned(),
            kind: CompletionKind::Value,
            detail: detail_parts.render(),
            insert_text: None,
            insert_format: CompletionInsertFormat::PlainText,
            sort_text: Some(completion_sort_text(CompletionKind::Value, label, prefix)),
            metadata: Default::default(),
        }
        .with_detail_parts(detail_parts)
    })
    .collect()
}
