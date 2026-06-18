use vela_analysis::completion::CompletionItem as AnalysisCompletionItem;

use super::{
    CompletionInsertFormat, CompletionItem, CompletionKind, CompletionLabelDetails,
    display_type_detail_parts, relevance::completion_sort_text,
};

pub(super) fn type_completion_item(
    item: AnalysisCompletionItem,
    qualified_name: &str,
    prefix: &str,
) -> CompletionItem {
    let kind = CompletionKind::from(item.kind);
    let (owner, label) = owner_and_short_label(qualified_name);
    let detail_parts = display_type_detail_parts(item.fact.display_name());
    let mut completion = CompletionItem {
        label: label.clone(),
        kind,
        detail: detail_parts.render(),
        insert_text: Some(label.clone()),
        insert_format: CompletionInsertFormat::PlainText,
        sort_text: Some(completion_sort_text(kind, &label, prefix)),
        metadata: Default::default(),
    }
    .with_detail_parts(detail_parts);
    completion.metadata.lookup = Some(qualified_name.to_owned());
    completion.metadata.filter_text = Some(qualified_name.to_owned());
    completion.metadata.label_details = CompletionLabelDetails {
        detail: None,
        description: owner,
    };
    completion
}

pub(super) fn owner_and_short_label(qualified_name: &str) -> (Option<String>, String) {
    qualified_name.rsplit_once("::").map_or_else(
        || (None, qualified_name.to_owned()),
        |(owner, label)| (Some(owner.to_owned()), label.to_owned()),
    )
}
