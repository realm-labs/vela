use vela_analysis::type_fact::TypeFact;

use super::{
    CompletionInsertFormat, CompletionItem, CompletionKind, relevance::completion_sort_text,
};

pub(super) fn builtin_value_completion_items(prefix: &str) -> Vec<CompletionItem> {
    [
        ("false", TypeFact::BOOL),
        ("null", TypeFact::NULL),
        ("true", TypeFact::BOOL),
    ]
    .into_iter()
    .filter(|(label, _)| label.starts_with(prefix))
    .map(|(label, fact)| CompletionItem {
        label: label.to_owned(),
        kind: CompletionKind::Value,
        detail: fact.display_name(),
        insert_text: None,
        insert_format: CompletionInsertFormat::PlainText,
        sort_text: Some(completion_sort_text(CompletionKind::Value, label, prefix)),
        metadata: Default::default(),
    })
    .collect()
}
