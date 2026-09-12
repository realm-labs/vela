use crate::DisplayParts;
use crate::QueryContext;
use crate::{DocumentId, GenerationToken, LanguageServiceDatabases, Position, TextRange};

mod accumulator;
mod analysis;
mod analysis_item;
#[cfg(test)]
mod analysis_tests;
mod builtin_type;
mod builtin_value;
#[cfg(test)]
mod call_expression_tests;
#[cfg(test)]
mod call_parameter_tests;
#[cfg(test)]
mod callable_import_tests;
mod context;
mod expression;
#[cfg(test)]
mod expression_ownership_tests;
#[cfg(test)]
mod import_alias_tests;
mod imports;
mod item;
mod lambda_parameter;
mod local;
mod map_key;
mod member_index;
#[cfg(test)]
mod member_index_tests;
#[cfg(test)]
mod member_matrix_tests;
#[cfg(test)]
mod member_tests;
mod model;
mod module_path;
mod named_argument;
#[cfg(test)]
mod named_argument_matrix_tests;
mod pattern;
mod record_field;
mod record_field_source;
#[cfg(test)]
mod record_field_tests;
mod relevance;
mod schema_function;
mod schema_type;
#[cfg(test)]
mod service_parameter_tests;
mod service_path;
#[cfg(test)]
mod service_path_tests;
mod source_declaration;
mod source_member;
mod source_module;
mod statement;
mod stdlib_function;
mod struct_field;
#[cfg(test)]
mod struct_field_tests;
#[cfg(test)]
mod tuple_destructuring_tests;
mod type_display;
mod type_hint;
#[cfg(test)]
mod type_label_tests;
#[cfg(test)]
mod type_matrix_tests;
#[cfg(test)]
mod type_ownership_tests;
mod type_paths;

pub use model::{
    CompletionContext, CompletionContextKind, CompletionInsertFormat, CompletionItem,
    CompletionItemMetadata, CompletionKind, CompletionLabelDetails, CompletionList,
    CompletionResolvePayload, CompletionSymbol, CompletionTextEdit,
};
pub use relevance::CompletionRelevance;

use analysis::completion_analysis;
pub use analysis::{
    CompletionAnalysis, CompletionAnalysisKind, CompletionCallArgumentContext,
    CompletionDeclaration, CompletionDeclarationKind, DotAccess, PathCompletionCtx,
    PathCompletionKind, PatternContext, RecordFieldContext, StatementContext, TypeLocation,
};
use context::completion_context;
use expression::{
    expression_completion_items as expression_context_completion_items,
    statement_expression_completion_items as statement_expression_context_completion_items,
};
use item::item_keyword_completions;
use lambda_parameter::lambda_parameter_completion_items;
use map_key::map_key_completion_items as map_key_context_completion_items;
use member_index::MemberCompletionIndex;
use model::{CallArgumentContext, MemberReceiver};
use module_path::module_path_completion_items as module_path_context_completion_items;
use named_argument::script_function_parameter_completions;
use pattern::pattern_completion_items as pattern_context_completion_items;
use record_field::record_field_completion_items as record_field_context_completion_items;
use statement::statement_keyword_completions;
use struct_field::struct_field_completion_items as struct_field_context_completion_items;
use type_hint::type_hint_completion_items;

use accumulator::CompletionAccumulator;

impl LanguageServiceDatabases {
    #[must_use]
    pub fn completion_items(&self, document_id: &DocumentId, position: Position) -> CompletionList {
        self.completion_items_with_optional_token(document_id, position, None)
            .unwrap_or_else(|| empty_completion_list(CompletionContext::expression(0, "")))
    }

    #[must_use]
    pub fn cancellable_completion_items(
        &self,
        document_id: &DocumentId,
        position: Position,
        token: &GenerationToken,
    ) -> Option<CompletionList> {
        self.completion_items_with_optional_token(document_id, position, Some(token))
    }

    fn completion_items_with_optional_token(
        &self,
        document_id: &DocumentId,
        position: Position,
        token: Option<&GenerationToken>,
    ) -> Option<CompletionList> {
        self.completion_query_is_current(token).then_some(())?;
        let Some(query) = QueryContext::from_databases(self, document_id, position) else {
            return Some(empty_completion_list(CompletionContext::expression(0, "")));
        };
        self.completion_query_is_current(token).then_some(())?;
        let context = completion_context(&query);
        let analysis = completion_analysis(self, &query, &context);
        let items = if let Some(items) = service_path::completion_items(self, &query, &context) {
            items
        } else {
            match analysis.context_kind() {
                CompletionContextKind::Expression => {
                    self.expression_completion_items(&query, &context)
                }
                CompletionContextKind::Item => self.item_completion_items(&context),
                CompletionContextKind::Statement => {
                    self.statement_completion_items(&query, &context)
                }
                CompletionContextKind::ModulePath => {
                    self.module_path_completion_items(&query, &context)
                }
                CompletionContextKind::Member => self.member_completion_items(&query, &context),
                CompletionContextKind::RecordField => self.record_field_completion_items(&context),
                CompletionContextKind::StructFieldDeclaration => {
                    self.struct_field_completion_items(&context)
                }
                CompletionContextKind::MapKey => self.map_key_completion_items(&context),
                CompletionContextKind::Pattern => self.pattern_completion_items(&query, &context),
                CompletionContextKind::NamedArgument => {
                    self.named_argument_completion_items(&query, &context)
                }
                CompletionContextKind::LambdaParameter => {
                    self.lambda_parameter_completion_items(&query, &context)
                }
                CompletionContextKind::TypeHint => {
                    self.type_hint_completion_items(&query, &context)
                }
            }
        };
        self.completion_query_is_current(token).then_some(())?;
        Some(CompletionList {
            context,
            analysis,
            items,
        })
    }

    fn completion_query_is_current(&self, token: Option<&GenerationToken>) -> bool {
        token.is_none_or(|token| token.generation() == self.generation() && !token.is_cancelled())
    }

    fn expression_completion_items(
        &self,
        query: &QueryContext<'_>,
        context: &CompletionContext,
    ) -> Vec<CompletionItem> {
        expression_context_completion_items(
            self,
            self.hir_db().graph(),
            self.schema_db().facts(),
            query,
            context,
        )
    }

    fn item_completion_items(&self, context: &CompletionContext) -> Vec<CompletionItem> {
        dedupe_and_filter_service_items(
            item_keyword_completions(context.prefix()),
            context.replace_range(),
            context.prefix(),
            |item| label_segment_matches(item.label(), context.prefix()),
        )
    }

    fn statement_completion_items(
        &self,
        query: &QueryContext<'_>,
        context: &CompletionContext,
    ) -> Vec<CompletionItem> {
        let mut items = statement_keyword_completions(context.prefix());
        items.extend(statement_expression_context_completion_items(
            self,
            self.hir_db().graph(),
            self.schema_db().facts(),
            query,
            context,
        ));
        dedupe_and_filter_service_items(items, context.replace_range(), context.prefix(), |item| {
            label_segment_matches(item.label(), context.prefix())
        })
    }

    fn module_path_completion_items(
        &self,
        query: &QueryContext<'_>,
        context: &CompletionContext,
    ) -> Vec<CompletionItem> {
        module_path_context_completion_items(self, query, context)
    }

    fn member_completion_items(
        &self,
        query: &QueryContext<'_>,
        context: &CompletionContext,
    ) -> Vec<CompletionItem> {
        let Some(receiver) = context.member_receiver.as_ref() else {
            return Vec::new();
        };
        let Some(receiver_fact) = query.type_fact_for_range(self, receiver.range) else {
            return Vec::new();
        };
        let index = MemberCompletionIndex::for_receiver(
            self.hir_db().graph(),
            self.schema_db().facts(),
            &receiver_fact,
            query.source_type_for_range(self, receiver.range),
            context.replace_range(),
            context.prefix(),
        );
        index.into_items()
    }

    fn record_field_completion_items(&self, context: &CompletionContext) -> Vec<CompletionItem> {
        record_field_context_completion_items(
            self.hir_db().graph(),
            self.schema_db().facts(),
            context,
        )
    }

    fn struct_field_completion_items(&self, context: &CompletionContext) -> Vec<CompletionItem> {
        dedupe_and_filter_service_items(
            struct_field_context_completion_items(context.prefix()),
            context.replace_range(),
            context.prefix(),
            |item| label_segment_matches(item.label(), context.prefix()),
        )
    }

    fn named_argument_completion_items(
        &self,
        query: &QueryContext<'_>,
        context: &CompletionContext,
    ) -> Vec<CompletionItem> {
        let Some(call) = context.call_arguments.as_ref() else {
            return Vec::new();
        };
        let callables = query.call_target_facts(self);
        let mut items = script_function_parameter_completions(&callables, query, call.has_equal);
        if query
            .call_argument_position()
            .is_some_and(|position| position.allows_positional_expression(callables.first()))
        {
            items.extend(self.expression_completion_items(query, context));
        }
        dedupe_and_filter_service_items(items, context.replace_range(), context.prefix(), |item| {
            label_segment_matches(item.label(), context.prefix())
        })
    }

    fn map_key_completion_items(&self, context: &CompletionContext) -> Vec<CompletionItem> {
        let Some(map_key) = context.map_key.as_ref() else {
            return Vec::new();
        };
        map_key_context_completion_items(
            self.hir_db().graph(),
            self.schema_db().facts(),
            map_key,
            context.replace_range(),
            context.prefix(),
        )
    }

    fn pattern_completion_items(
        &self,
        query: &QueryContext<'_>,
        context: &CompletionContext,
    ) -> Vec<CompletionItem> {
        let current_module = query
            .module_path()
            .map(|module| module.segments().to_vec())
            .unwrap_or_default();
        let graph = self.hir_db().graph();
        pattern_context_completion_items(
            graph,
            self.schema_db().facts(),
            &current_module,
            context.replace_range(),
            context.prefix(),
        )
    }

    fn lambda_parameter_completion_items(
        &self,
        query: &QueryContext<'_>,
        context: &CompletionContext,
    ) -> Vec<CompletionItem> {
        let Some(lambda_parameter) = context.lambda_parameter.as_ref() else {
            return Vec::new();
        };
        let Some(receiver_fact) = query.type_fact_for_range(self, lambda_parameter.receiver.range)
        else {
            return Vec::new();
        };
        lambda_parameter_completion_items(&receiver_fact, lambda_parameter, context.prefix())
    }

    fn type_hint_completion_items(
        &self,
        query: &QueryContext<'_>,
        context: &CompletionContext,
    ) -> Vec<CompletionItem> {
        type_hint_completion_items(self, query, context)
    }
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn dedupe_and_filter_service_items(
    items: Vec<CompletionItem>,
    replace_range: TextRange,
    prefix: &str,
    matches_context: impl Fn(&CompletionItem) -> bool,
) -> Vec<CompletionItem> {
    let mut accumulator = CompletionAccumulator::new(replace_range, prefix);
    accumulator.add_many_matching(items, matches_context);
    accumulator.into_items()
}

fn label_segment_matches(label: &str, prefix: &str) -> bool {
    prefix.is_empty()
        || label.starts_with(prefix)
        || label
            .rsplit("::")
            .next()
            .is_some_and(|segment| segment.starts_with(prefix))
}

fn empty_completion_list(context: CompletionContext) -> CompletionList {
    let analysis = CompletionAnalysis::from_context_only(&context);
    CompletionList {
        context,
        analysis,
        items: Vec::new(),
    }
}

pub(super) fn display_type_detail_parts(text: impl AsRef<str>) -> DisplayParts {
    DisplayParts::type_name(text.as_ref())
}

pub(super) fn display_qualified_detail(owner: &str, member: &str) -> String {
    display_qualified_detail_parts(owner, member).render()
}

pub(super) fn display_qualified_detail_parts(owner: &str, member: &str) -> DisplayParts {
    DisplayParts::qualified(owner, member)
}

#[cfg(test)]
mod tests;
