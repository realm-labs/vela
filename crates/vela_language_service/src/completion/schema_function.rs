use vela_analysis::completion::{
    CompletionItem as AnalysisCompletionItem, CompletionKind as AnalysisCompletionKind,
};

use crate::{LanguageServiceDatabases, QueryContext, TextRange};

use super::{
    CompletionItem, analysis_item::dedupe_and_filter_analysis_items, label_segment_matches,
};

pub(super) fn schema_function_completion_items(
    databases: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    replace_range: TextRange,
    prefix: &str,
) -> Vec<CompletionItem> {
    let schema = databases.schema_db().facts();
    let scope = super::imports::ImportScope::new(databases, query);
    dedupe_and_filter_analysis_items(
        schema
            .functions()
            .filter(|function| scope.external_function_available(&function.name))
            .map(|function| AnalysisCompletionItem {
                label: function.name,
                kind: AnalysisCompletionKind::Function,
                fact: function.fact,
            })
            .collect(),
        replace_range,
        prefix,
        Some(schema),
        |item| label_segment_matches(&item.label, prefix),
    )
}
