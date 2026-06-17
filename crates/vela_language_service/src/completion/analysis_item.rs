use vela_analysis::completion::CompletionItem as AnalysisCompletionItem;
use vela_analysis::registry::RegistryFacts;

use crate::TextRange;

use super::{
    CompletionInsertFormat, CompletionItem, CompletionKind, CompletionSymbol,
    accumulator::CompletionAccumulator, display_type_detail, relevance::completion_sort_text,
};

pub(super) fn dedupe_and_filter_analysis_items(
    items: Vec<AnalysisCompletionItem>,
    replace_range: TextRange,
    prefix: &str,
    schema: Option<&RegistryFacts>,
    matches_context: impl Fn(&AnalysisCompletionItem) -> bool,
) -> Vec<CompletionItem> {
    let mut accumulator = CompletionAccumulator::new(replace_range, prefix);
    for item in items.into_iter().filter(matches_context) {
        let completion = service_item_from_analysis_completion(item, prefix);
        accumulator.add(enrich_analysis_completion_item(completion, schema));
    }
    accumulator.into_items()
}

pub(super) fn service_item_from_analysis_completion(
    item: AnalysisCompletionItem,
    prefix: &str,
) -> CompletionItem {
    let kind = item.kind.into();
    let insert_text = callable_insert_text(kind, &item.label);
    let insert_format = completion_insert_format(insert_text.as_ref());
    CompletionItem {
        sort_text: Some(completion_sort_text(kind, &item.label, prefix)),
        metadata: Default::default(),
        label: item.label,
        kind,
        detail: display_type_detail(item.fact.display_name()),
        insert_text,
        insert_format,
    }
}

fn enrich_analysis_completion_item(
    item: CompletionItem,
    schema: Option<&RegistryFacts>,
) -> CompletionItem {
    let Some(schema) = schema else {
        return item;
    };
    let label = item.label().to_owned();
    match item.kind() {
        CompletionKind::Type if schema.type_fact(&label).is_some() => item
            .with_documentation(schema.type_docs(&label))
            .with_symbol(CompletionSymbol::Schema(label)),
        CompletionKind::Trait if schema.trait_fact(&label).is_some() => item
            .with_documentation(schema.trait_docs(&label))
            .with_symbol(CompletionSymbol::Schema(label)),
        CompletionKind::Function if schema.function_fact(&label).is_some() => item
            .with_documentation(schema.function_docs(&label))
            .with_symbol(CompletionSymbol::Schema(label)),
        _ => item,
    }
}

pub(super) fn callable_insert_text(kind: CompletionKind, label: &str) -> Option<String> {
    matches!(kind, CompletionKind::Function | CompletionKind::Method)
        .then(|| format!("{label}($0)"))
}

pub(super) fn completion_insert_format(insert_text: Option<&String>) -> CompletionInsertFormat {
    if insert_text.is_some_and(|text| text.contains("$0")) {
        CompletionInsertFormat::Snippet
    } else {
        CompletionInsertFormat::PlainText
    }
}
