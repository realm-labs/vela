use vela_analysis::completion::CompletionItem as AnalysisCompletionItem;
use vela_analysis::registry::RegistryFacts;

use crate::TextRange;

use super::{
    CompletionInsertFormat, CompletionItem, CompletionKind, CompletionSymbol,
    accumulator::CompletionAccumulator,
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
        detail: item.fact.display_name(),
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

pub(super) fn completion_sort_text(kind: CompletionKind, label: &str, prefix: &str) -> String {
    format!(
        "{:04}_{:02}_{}",
        completion_kind_rank(kind),
        completion_match_rank(label, prefix),
        label
    )
}

fn completion_kind_rank(kind: CompletionKind) -> u16 {
    match kind {
        CompletionKind::Parameter => 0,
        CompletionKind::Keyword => 0,
        CompletionKind::Binding => 1,
        CompletionKind::Const => 10,
        CompletionKind::Module => 20,
        CompletionKind::Type | CompletionKind::Trait => 30,
        CompletionKind::Function | CompletionKind::Method => 40,
        CompletionKind::Field => 50,
        CompletionKind::Variant => 60,
    }
}

fn completion_match_rank(label: &str, prefix: &str) -> u8 {
    if prefix.is_empty() || label.starts_with(prefix) {
        return 0;
    }
    if label
        .rsplit("::")
        .next()
        .is_some_and(|segment| segment.starts_with(prefix))
    {
        return 1;
    }
    2
}
