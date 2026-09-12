use vela_analysis::completion::{
    CompletionItem as AnalysisCompletionItem, CompletionKind as AnalysisCompletionKind,
};
use vela_analysis::registry::RegistryFacts;
use vela_analysis::stdlib::stdlib_method_facts;
use vela_analysis::type_fact::TypeFact;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};

use crate::symbol_ref::{builtin_member_symbol, schema_member_symbol, schema_variant_symbol};
use crate::{CompletionSymbol, TextRange};

use super::accumulator::CompletionAccumulator;
use super::analysis_item::service_item_from_analysis_completion;
use super::source_member::{
    SourceMemberCompletion, declaration_name_matches, source_member_completion_candidates,
};
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
        source_origins: Option<&vela_analysis::semantic_facts::ScriptTypeOrigins>,
        replace_range: TextRange,
        prefix: &str,
    ) -> Self {
        let mut index = Self {
            entries: Vec::new(),
            replace_range,
            prefix: prefix.to_owned(),
        };
        index.extend_source(graph, schema, receiver, source_origins);
        index.extend_schema(graph, schema, receiver);
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

    fn extend_source(
        &mut self,
        graph: &ModuleGraph,
        schema: &RegistryFacts,
        receiver: &TypeFact,
        source_origins: Option<&vela_analysis::semantic_facts::ScriptTypeOrigins>,
    ) {
        let candidates = match source_origins.filter(|origins| !origins.possible().is_empty()) {
            Some(origins) => origins
                .possible()
                .iter()
                .flat_map(|owner| {
                    source_member_completion_candidates(
                        graph,
                        schema,
                        receiver,
                        Some(owner.declaration),
                    )
                })
                .collect(),
            None => source_member_completion_candidates(graph, schema, receiver, None),
        };
        let mut merged: Vec<(SourceMemberCompletion, Vec<String>)> = Vec::new();
        for candidate in candidates {
            let detail = candidate.detail();
            if let Some((prior, details)) = merged.iter_mut().find(|(prior, _)| {
                prior.item.label == candidate.item.label
                    && prior.item.kind == candidate.item.kind
                    && prior.symbol == candidate.symbol
            }) {
                let mut facts = match &prior.item.fact {
                    TypeFact::Union(facts) => facts.clone(),
                    fact => vec![fact.clone()],
                };
                facts.push(candidate.item.fact);
                facts.sort_by_key(TypeFact::display_name);
                prior.item.fact = TypeFact::union(facts);
                details.push(detail);
                details.sort();
                details.dedup();
            } else {
                merged.push((candidate, vec![detail]));
            }
        }
        for (candidate, details) in merged {
            let method = candidate.item.kind == AnalysisCompletionKind::Method;
            let item = self.push_analysis(
                candidate.item,
                MemberCompletionSurface::Source,
                Some(candidate.symbol),
                None,
            );
            if method {
                let mut parts = crate::DisplayParts::new();
                for (index, detail) in details.iter().enumerate() {
                    if index != 0 {
                        parts.extend(crate::DisplayParts::plain(" | "));
                    }
                    let detail = if let Some(detail) = detail.strip_prefix("async ") {
                        parts.extend(crate::DisplayParts::plain("async "));
                        detail
                    } else {
                        detail
                    };
                    parts.extend(super::display_type_detail_parts(detail));
                }
                item.set_detail_parts(parts);
            }
        }
    }

    fn extend_schema(&mut self, graph: &ModuleGraph, schema: &RegistryFacts, receiver: &TypeFact) {
        let source_owner = match receiver {
            TypeFact::Record { name } => Some((name, DeclarationKind::Struct)),
            TypeFact::Trait { name } => Some((name, DeclarationKind::Trait)),
            TypeFact::Enum { name, .. } => Some((name, DeclarationKind::Enum)),
            _ => None,
        };
        if source_owner.is_some_and(|(name, kind)| {
            graph.declarations().any(|declaration| {
                declaration.kind == kind && declaration_name_matches(graph, declaration.id, name)
            })
        }) {
            return;
        }
        for (item, symbol) in schema_member_completion_candidates(schema, receiver) {
            let resource = match receiver {
                TypeFact::Host { name } | TypeFact::Record { name } => {
                    schema.method_scoped_resource(name, &item.label)
                }
                _ => None,
            };
            let signature = match receiver {
                TypeFact::Host { name } | TypeFact::Record { name } => {
                    schema.method_signature_fact(name, &item.label)
                }
                TypeFact::Trait { name } => schema.trait_method_signature_fact(name, &item.label),
                _ => None,
            };
            let completion = self.push_analysis(
                item,
                MemberCompletionSurface::Schema,
                Some(symbol),
                resource,
            );
            if signature.is_some_and(|signature| signature.asyncness.is_async()) {
                let mut parts = crate::DisplayParts::plain("async ");
                parts.extend(completion.detail_parts());
                completion.set_detail_parts(parts);
            }
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
            self.push_analysis(item, MemberCompletionSurface::Builtin, Some(symbol), None);
        }
    }

    fn push_analysis(
        &mut self,
        item: AnalysisCompletionItem,
        surface: MemberCompletionSurface,
        symbol: Option<CompletionSymbol>,
        scoped_resource: Option<vela_analysis::registry::ScopedResourceReturnDef>,
    ) -> &mut CompletionItem {
        let scoped_detail = scoped_resource.and_then(|resource| match &item.fact {
            TypeFact::Function { returns, .. } => Some(format!(
                "{}; {}",
                crate::callable_context::scoped_resource_display_name(returns, resource),
                crate::callable_context::scoped_resource_detail(resource)
            )),
            _ => None,
        });
        let mut item = service_item_from_analysis_completion(item, &self.prefix);
        item.insert_text.get_or_insert_with(|| item.label.clone());
        if let Some(detail) = scoped_detail {
            item = item.with_detail_parts(crate::DisplayParts::plain(detail));
        }
        if let Some(symbol) = symbol {
            item = item.with_symbol(symbol);
        }
        self.entries.push(MemberCompletionEntry { item, surface });
        &mut self.entries.last_mut().expect("inserted completion").item
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
