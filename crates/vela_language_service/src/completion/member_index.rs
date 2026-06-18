use vela_analysis::completion::{
    CompletionItem as AnalysisCompletionItem, CompletionKind as AnalysisCompletionKind,
};
use vela_analysis::registry::RegistryFacts;
use vela_analysis::stdlib::stdlib_method_facts;
use vela_analysis::type_fact::TypeFact;
use vela_hir::module_graph::ModuleGraph;

use crate::symbol_ref::{builtin_member_symbol, schema_member_symbol, schema_variant_symbol};
use crate::{CompletionSymbol, TextRange};

use super::accumulator::CompletionAccumulator;
use super::analysis_item::service_item_from_analysis_completion;
use super::source_member::source_member_completion_candidates;
use super::{CompletionItem, label_segment_matches};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct MemberCompletionIndex {
    entries: Vec<MemberCompletionEntry>,
    replace_range: TextRange,
    prefix: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct MemberCompletionEntry {
    item: CompletionItem,
    surface: MemberCompletionSurface,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum MemberCompletionSurface {
    Source,
    Schema,
    Builtin,
}

impl MemberCompletionIndex {
    pub(super) fn for_receiver(
        graph: &ModuleGraph,
        schema: &RegistryFacts,
        receiver: &TypeFact,
        replace_range: TextRange,
        prefix: &str,
    ) -> Self {
        let mut index = Self {
            entries: Vec::new(),
            replace_range,
            prefix: prefix.to_owned(),
        };
        index.extend_source(graph, schema, receiver);
        index.extend_schema(schema, receiver);
        index.extend_builtin(receiver);
        index
    }

    pub(super) fn into_items(self) -> Vec<CompletionItem> {
        let mut accumulator = CompletionAccumulator::new(self.replace_range, &self.prefix);
        for entry in self.entries {
            if label_segment_matches(entry.item.label(), &self.prefix) {
                accumulator.add(entry.item);
            }
        }
        accumulator.into_items()
    }

    fn extend_source(&mut self, graph: &ModuleGraph, schema: &RegistryFacts, receiver: &TypeFact) {
        for (item, symbol) in source_member_completion_candidates(graph, schema, receiver) {
            self.push_analysis(item, MemberCompletionSurface::Source, Some(symbol));
        }
    }

    fn extend_schema(&mut self, schema: &RegistryFacts, receiver: &TypeFact) {
        for (item, symbol) in schema_member_completion_candidates(schema, receiver) {
            self.push_analysis(item, MemberCompletionSurface::Schema, Some(symbol));
        }
    }

    fn extend_builtin(&mut self, receiver: &TypeFact) {
        for fact in stdlib_method_facts(receiver, None) {
            let item = AnalysisCompletionItem {
                label: fact.method.to_owned(),
                kind: AnalysisCompletionKind::Method,
                fact: TypeFact::function(fact.params, fact.returns),
            };
            let symbol = builtin_member_symbol(&fact.receiver.display_name(), fact.method);
            self.push_analysis(item, MemberCompletionSurface::Builtin, Some(symbol));
        }
    }

    fn push_analysis(
        &mut self,
        item: AnalysisCompletionItem,
        surface: MemberCompletionSurface,
        symbol: Option<CompletionSymbol>,
    ) {
        let mut item = service_item_from_analysis_completion(item, &self.prefix);
        if let Some(symbol) = symbol {
            item = item.with_symbol(symbol);
        }
        self.entries.push(MemberCompletionEntry { item, surface });
    }

    #[cfg(test)]
    pub(super) fn surfaces_for_label(&self, label: &str) -> Vec<MemberCompletionSurface> {
        self.entries
            .iter()
            .filter(|entry| entry.item.label() == label)
            .map(|entry| entry.surface)
            .collect()
    }
}

fn schema_member_completion_candidates(
    schema: &RegistryFacts,
    receiver: &TypeFact,
) -> Vec<(AnalysisCompletionItem, CompletionSymbol)> {
    match receiver {
        TypeFact::Host { name } | TypeFact::Record { name } => {
            owner_member_completion_candidates(schema, name)
        }
        TypeFact::Enum {
            name,
            variant: Some(variant),
        } => owner_field_completion_candidates(schema, &format!("{name}::{variant}")),
        TypeFact::Enum {
            name,
            variant: None,
        } => variant_completion_candidates(schema, name),
        TypeFact::Trait { name } => trait_method_completion_candidates(schema, name),
        _ => Vec::new(),
    }
}

fn owner_member_completion_candidates(
    schema: &RegistryFacts,
    owner: &str,
) -> Vec<(AnalysisCompletionItem, CompletionSymbol)> {
    let mut candidates = owner_field_completion_candidates(schema, owner);
    candidates.extend(
        schema
            .methods()
            .filter(|method| method.owner == owner)
            .map(|method| {
                (
                    AnalysisCompletionItem {
                        label: method.name.clone(),
                        kind: AnalysisCompletionKind::Method,
                        fact: method.fact.clone(),
                    },
                    schema_member_symbol(owner, &method.name),
                )
            }),
    );
    candidates
}

fn owner_field_completion_candidates(
    schema: &RegistryFacts,
    owner: &str,
) -> Vec<(AnalysisCompletionItem, CompletionSymbol)> {
    schema
        .fields()
        .filter(|field| field.owner == owner)
        .map(|field| {
            (
                AnalysisCompletionItem {
                    label: field.name.clone(),
                    kind: AnalysisCompletionKind::Field,
                    fact: field.fact.clone(),
                },
                schema_member_symbol(owner, &field.name),
            )
        })
        .collect()
}

fn variant_completion_candidates(
    schema: &RegistryFacts,
    owner: &str,
) -> Vec<(AnalysisCompletionItem, CompletionSymbol)> {
    schema
        .variants()
        .filter(|variant| variant.owner == owner)
        .map(|variant| {
            (
                AnalysisCompletionItem {
                    label: variant.name.clone(),
                    kind: AnalysisCompletionKind::Variant,
                    fact: variant.fact.clone(),
                },
                schema_variant_symbol(owner, &variant.name),
            )
        })
        .collect()
}

fn trait_method_completion_candidates(
    schema: &RegistryFacts,
    owner: &str,
) -> Vec<(AnalysisCompletionItem, CompletionSymbol)> {
    schema
        .trait_methods()
        .filter(|method| method.owner == owner)
        .map(|method| {
            (
                AnalysisCompletionItem {
                    label: method.name.clone(),
                    kind: AnalysisCompletionKind::Method,
                    fact: method.fact.clone(),
                },
                schema_member_symbol(owner, &method.name),
            )
        })
        .collect()
}
