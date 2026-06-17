use vela_analysis::completion::member_completions;
use vela_analysis::facts::AnalysisFacts;
use vela_analysis::hints::type_fact_from_hint;
use vela_analysis::registry::RegistryFacts;
use vela_analysis::type_fact::TypeFact;
use vela_common::Span;
use vela_hir::binding::{BindingMap, BindingResolution, LocalBinding};
use vela_hir::module_graph::ModuleGraph;
use vela_hir::type_hint::HirTypeHint;

use crate::QueryContext;
use crate::{DocumentId, LanguageServiceDatabases, Position, TextRange};

mod accumulator;
mod analysis_item;
mod context;
mod expression;
mod item;
mod lambda_parameter;
mod local;
mod map_key;
mod model;
mod module_path;
mod named_argument;
mod pattern;
mod record_field;
mod relevance;
mod schema_function;
mod schema_type;
mod source_declaration;
mod source_module;
mod statement;
mod stdlib_function;
mod type_hint;

pub use model::{
    CompletionContext, CompletionContextKind, CompletionInsertFormat, CompletionItem,
    CompletionItemMetadata, CompletionKind, CompletionLabelDetails, CompletionList,
    CompletionResolvePayload, CompletionSymbol, CompletionTextEdit,
};
pub use relevance::CompletionRelevance;

use context::completion_context;
use expression::{
    expression_completion_items as expression_context_completion_items,
    statement_expression_completion_items as statement_expression_context_completion_items,
};
use item::item_keyword_completions;
use lambda_parameter::lambda_parameter_completion_items;
use map_key::map_key_completion_items as map_key_context_completion_items;
use model::{CallArgumentContext, MemberReceiver};
use module_path::module_path_completion_items as module_path_context_completion_items;
use named_argument::script_function_parameter_completions;
use pattern::pattern_completion_items as pattern_context_completion_items;
use record_field::record_field_completion_items as record_field_context_completion_items;
use statement::statement_keyword_completions;
use type_hint::type_hint_completion_items;

use accumulator::CompletionAccumulator;
use analysis_item::service_item_from_analysis_completion;

impl LanguageServiceDatabases {
    #[must_use]
    pub fn completion_items(&self, document_id: &DocumentId, position: Position) -> CompletionList {
        let Some(query) = QueryContext::from_databases(self, document_id, position) else {
            return empty_completion_list(CompletionContext::expression(0, ""));
        };
        let context = completion_context(&query);
        let items = match context.kind {
            CompletionContextKind::Expression => self.expression_completion_items(&query, &context),
            CompletionContextKind::Item => self.item_completion_items(&context),
            CompletionContextKind::Statement => self.statement_completion_items(&query, &context),
            CompletionContextKind::ModulePath => self.module_path_completion_items(&context),
            CompletionContextKind::Member => self.member_completion_items(document_id, &context),
            CompletionContextKind::RecordField => self.record_field_completion_items(&context),
            CompletionContextKind::MapKey => self.map_key_completion_items(&context),
            CompletionContextKind::Pattern => self.pattern_completion_items(&query, &context),
            CompletionContextKind::NamedArgument => self.named_argument_completion_items(&context),
            CompletionContextKind::LambdaParameter => {
                self.lambda_parameter_completion_items(document_id, &context)
            }
            CompletionContextKind::TypeHint => self.type_hint_completion_items(&context),
        };
        CompletionList { context, items }
    }

    fn expression_completion_items(
        &self,
        query: &QueryContext<'_>,
        context: &CompletionContext,
    ) -> Vec<CompletionItem> {
        expression_context_completion_items(
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
            self.hir_db().graph(),
            self.schema_db().facts(),
            query,
            context,
        ));
        dedupe_and_filter_service_items(items, context.replace_range(), context.prefix(), |item| {
            label_segment_matches(item.label(), context.prefix())
        })
    }

    fn module_path_completion_items(&self, context: &CompletionContext) -> Vec<CompletionItem> {
        module_path_context_completion_items(
            self.hir_db().graph(),
            self.schema_db().facts(),
            context,
        )
    }

    fn member_completion_items(
        &self,
        document_id: &DocumentId,
        context: &CompletionContext,
    ) -> Vec<CompletionItem> {
        let Some(receiver) = context.member_receiver.as_ref() else {
            return Vec::new();
        };
        let Some(receiver_fact) = self.member_receiver_fact(document_id, receiver) else {
            return Vec::new();
        };
        let schema = self.schema_db().facts();
        let owner = schema_completion_owner(&receiver_fact);
        let items = member_completions(schema, &receiver_fact)
            .into_iter()
            .filter(|item| label_segment_matches(&item.label, context.prefix()))
            .map(|item| {
                let completion = service_item_from_analysis_completion(item, context.prefix());
                owner.as_deref().map_or(completion.clone(), |owner| {
                    enrich_schema_member_completion_item(completion, schema, owner)
                })
            })
            .collect::<Vec<_>>();
        dedupe_and_filter_service_items(items, context.replace_range(), context.prefix(), |_| true)
    }

    fn member_receiver_fact(
        &self,
        document_id: &DocumentId,
        receiver: &MemberReceiver,
    ) -> Option<TypeFact> {
        let source = self.source_db().records().get(document_id)?;
        let source_id = source.source_id();
        let start = u32::try_from(receiver.range.start).ok()?;
        let end = u32::try_from(receiver.range.end).ok()?;
        let receiver_span = Span::new(source_id, start, end);
        let graph = self.hir_db().graph();
        let facts = AnalysisFacts::from_module_graph(graph);

        graph.declarations().find_map(|declaration| {
            if declaration.span.source != source_id || !declaration.span.contains(start) {
                return None;
            }
            let bindings = graph.bindings(declaration.id)?;
            let resolution = bindings.resolution_at_span(receiver_span)?;
            type_fact_for_resolution(resolution, bindings, &facts, self.schema_db().facts())
        })
    }

    fn record_field_completion_items(&self, context: &CompletionContext) -> Vec<CompletionItem> {
        record_field_context_completion_items(
            self.hir_db().graph(),
            self.schema_db().facts(),
            context,
        )
    }

    fn named_argument_completion_items(&self, context: &CompletionContext) -> Vec<CompletionItem> {
        let Some(call) = context.call_arguments.as_ref() else {
            return Vec::new();
        };
        let used_names = call
            .used_names
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let items = script_function_parameter_completions(
            self.hir_db().graph(),
            self.schema_db().facts(),
            &call.callee,
            &used_names,
        );
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
        document_id: &DocumentId,
        context: &CompletionContext,
    ) -> Vec<CompletionItem> {
        let Some(lambda_parameter) = context.lambda_parameter.as_ref() else {
            return Vec::new();
        };
        let Some(receiver_fact) =
            self.member_receiver_fact(document_id, &lambda_parameter.receiver)
        else {
            return Vec::new();
        };
        lambda_parameter_completion_items(&receiver_fact, lambda_parameter, context.prefix())
    }

    fn type_hint_completion_items(&self, context: &CompletionContext) -> Vec<CompletionItem> {
        type_hint_completion_items(
            self.hir_db().graph(),
            self.schema_db().facts(),
            context.replace_range(),
            context.prefix(),
            context.module_base(),
        )
    }
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn enrich_schema_member_completion_item(
    item: CompletionItem,
    schema: &RegistryFacts,
    owner: &str,
) -> CompletionItem {
    let label = item.label().to_owned();
    match item.kind() {
        CompletionKind::Field if schema.field_fact(owner, &label).is_some() => item
            .with_documentation(schema.field_docs(owner, &label))
            .with_symbol(CompletionSymbol::Schema(format!("{owner}.{label}"))),
        CompletionKind::Method if schema.method_fact(owner, &label).is_some() => item
            .with_documentation(schema.method_docs(owner, &label))
            .with_symbol(CompletionSymbol::Schema(format!("{owner}.{label}"))),
        CompletionKind::Method if schema.trait_method_fact(owner, &label).is_some() => item
            .with_documentation(schema.trait_method_docs(owner, &label))
            .with_symbol(CompletionSymbol::Schema(format!("{owner}.{label}"))),
        CompletionKind::Variant if schema.variant_fact(owner, &label).is_some() => item
            .with_documentation(schema.variant_docs(owner, &label))
            .with_symbol(CompletionSymbol::Schema(format!("{owner}::{label}"))),
        _ => item,
    }
}

fn schema_completion_owner(fact: &TypeFact) -> Option<String> {
    match fact {
        TypeFact::Host { name } | TypeFact::Record { name } | TypeFact::Trait { name } => {
            Some(name.clone())
        }
        TypeFact::Enum {
            name,
            variant: Some(variant),
        } => Some(format!("{name}::{variant}")),
        TypeFact::Enum {
            name,
            variant: None,
        } => Some(name.clone()),
        _ => None,
    }
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

fn completion_type_fact(
    graph: &ModuleGraph,
    hint: &HirTypeHint,
    schema: &vela_analysis::registry::RegistryFacts,
) -> TypeFact {
    let fact = type_fact_from_hint(graph, hint);
    if matches!(fact, TypeFact::Unknown) {
        schema_fact_for_hint(hint, schema).unwrap_or(TypeFact::Unknown)
    } else {
        fact
    }
}

fn label_segment_matches(label: &str, prefix: &str) -> bool {
    prefix.is_empty()
        || label.starts_with(prefix)
        || label
            .rsplit("::")
            .next()
            .is_some_and(|segment| segment.starts_with(prefix))
}

fn type_fact_for_resolution(
    resolution: &BindingResolution,
    bindings: &BindingMap,
    facts: &AnalysisFacts,
    schema: &vela_analysis::registry::RegistryFacts,
) -> Option<TypeFact> {
    match resolution {
        BindingResolution::Local(local) => {
            let binding = bindings.local(*local)?;
            let fact = facts.local(*local).cloned();
            fact.filter(|fact| !matches!(fact, TypeFact::Unknown))
                .or_else(|| schema_fact_for_local_hint(binding, schema))
        }
        BindingResolution::Declaration(declaration) => facts.declaration(*declaration).cloned(),
        BindingResolution::Import(_) | BindingResolution::QualifiedPath(_) => None,
    }
}

fn schema_fact_for_local_hint(
    binding: &LocalBinding,
    schema: &vela_analysis::registry::RegistryFacts,
) -> Option<TypeFact> {
    let hint = binding.type_hint.as_ref()?;
    schema_fact_for_hint(hint, schema)
}

fn schema_fact_for_hint(
    hint: &HirTypeHint,
    schema: &vela_analysis::registry::RegistryFacts,
) -> Option<TypeFact> {
    if hint.args.is_empty() {
        let qualified = hint.path.join("::");
        schema
            .type_fact(&qualified)
            .or_else(|| schema.trait_fact(&qualified))
            .or_else(|| hint.path.last().and_then(|name| schema.type_fact(name)))
            .or_else(|| hint.path.last().and_then(|name| schema.trait_fact(name)))
            .cloned()
    } else {
        None
    }
}

fn empty_completion_list(context: CompletionContext) -> CompletionList {
    CompletionList {
        context,
        items: Vec::new(),
    }
}

#[cfg(test)]
mod tests;
