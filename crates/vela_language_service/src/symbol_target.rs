use vela_analysis::{
    registry::RegistryFacts,
    stdlib::{stdlib_function_completion_facts, stdlib_method_fact},
    type_fact::TypeFact,
};
use vela_common::Span;

use crate::{LanguageServiceDatabases, QueryContext, SymbolRef, TextRange, path_calls};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct SymbolTarget {
    text: String,
    range: TextRange,
    member_receiver_fact: Option<TypeFact>,
    symbol: Option<SymbolRef>,
}

impl SymbolTarget {
    pub(crate) fn from_query(
        databases: &LanguageServiceDatabases,
        query: &QueryContext<'_>,
    ) -> Option<Self> {
        let range = query.identifier_range()?;
        let text = query.text().get(range.start..range.end)?.to_owned();
        let member_receiver = query.member_receiver_range();
        let member_receiver_fact =
            member_receiver.and_then(|range| query.type_fact_for_range(databases, range));
        let symbol = symbol_ref_for(databases, &text, member_receiver_fact.as_ref());
        Some(Self {
            text,
            range,
            member_receiver_fact,
            symbol,
        })
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) const fn range(&self) -> TextRange {
        self.range
    }

    pub(crate) fn member_receiver_fact(&self) -> Option<&TypeFact> {
        self.member_receiver_fact.as_ref()
    }

    pub(crate) fn symbol(&self) -> Option<&SymbolRef> {
        self.symbol.as_ref()
    }

    pub(crate) fn is_schema_symbol(&self) -> bool {
        matches!(self.symbol, Some(SymbolRef::Schema(_)))
    }

    pub(crate) fn schema_symbol_span(&self, databases: &LanguageServiceDatabases) -> Option<Span> {
        let locations = databases.schema_db().source_locations();
        locations
            .type_span(&self.text)
            .or_else(|| locations.trait_span(&self.text))
            .or_else(|| locations.function_span(&self.text))
    }

    pub(crate) fn schema_member_span(&self, databases: &LanguageServiceDatabases) -> Option<Span> {
        let owner = self
            .member_receiver_fact
            .as_ref()
            .and_then(fact_owner_name)?;
        let locations = databases.schema_db().source_locations();
        locations
            .field_span(&owner, &self.text)
            .or_else(|| locations.method_span(&owner, &self.text))
            .or_else(|| locations.trait_method_span(&owner, &self.text))
    }

    pub(crate) fn schema_variant_target(
        &self,
        databases: &LanguageServiceDatabases,
        query: &QueryContext<'_>,
    ) -> Option<(Span, SymbolRef)> {
        let text = query.text();
        let source = query.source_record()?;
        let parsed = databases.parse_db().parsed_source(source.document_id())?;
        for site in path_calls::path_expression_sites(parsed, text) {
            if site.segment_range != self.range {
                continue;
            }
            let Some((variant, owner_segments)) = site.path.split_last() else {
                continue;
            };
            let Some(owner) =
                schema_variant_owner(databases.schema_db().facts(), owner_segments, variant)
            else {
                continue;
            };
            let span = databases
                .schema_db()
                .source_locations()
                .variant_span(&owner, variant)?;
            return Some((span, SymbolRef::Schema(format!("{owner}::{variant}"))));
        }
        None
    }
}

fn symbol_ref_for(
    databases: &LanguageServiceDatabases,
    text: &str,
    member_receiver_fact: Option<&TypeFact>,
) -> Option<SymbolRef> {
    let schema = databases.schema_db().facts();
    if let Some(receiver_fact) = member_receiver_fact {
        if let Some(owner) = fact_owner_name(receiver_fact)
            && (schema.field_fact(&owner, text).is_some()
                || schema.method_fact(&owner, text).is_some()
                || schema.trait_method_fact(&owner, text).is_some())
        {
            return Some(SymbolRef::Schema(format!("{owner}.{text}")));
        }
        if stdlib_method_fact(receiver_fact, text, None).is_some() {
            return Some(SymbolRef::Builtin(format!(
                "{}.{text}",
                receiver_fact.display_name()
            )));
        }
    }
    schema_symbol_ref(schema, text).or_else(|| stdlib_function_symbol_ref(text))
}

fn schema_symbol_ref(schema: &RegistryFacts, text: &str) -> Option<SymbolRef> {
    if schema.type_fact(text).is_some()
        || schema.trait_fact(text).is_some()
        || schema.function_fact(text).is_some()
    {
        return Some(SymbolRef::Schema(text.to_owned()));
    }
    if let Some((owner, variant)) = text.rsplit_once("::")
        && schema.variant_fact(owner, variant).is_some()
    {
        return Some(SymbolRef::Schema(text.to_owned()));
    }
    let mut variants = schema.variants().filter(|variant| variant.name == text);
    let variant = variants.next()?;
    variants
        .next()
        .is_none()
        .then(|| SymbolRef::Schema(format!("{}::{}", variant.owner, variant.name)))
}

fn stdlib_function_symbol_ref(text: &str) -> Option<SymbolRef> {
    stdlib_function_completion_facts()
        .into_iter()
        .find(|function| {
            function.name == text
                || function
                    .name
                    .rsplit("::")
                    .next()
                    .is_some_and(|segment| segment == text)
        })
        .map(|function| SymbolRef::Builtin(function.name.to_owned()))
}

fn fact_owner_name(fact: &TypeFact) -> Option<String> {
    match fact {
        TypeFact::Host { name }
        | TypeFact::Record { name }
        | TypeFact::Enum { name, .. }
        | TypeFact::Trait { name } => Some(name.clone()),
        _ => None,
    }
}

fn schema_variant_owner(
    schema: &RegistryFacts,
    owner_segments: &[String],
    variant: &str,
) -> Option<String> {
    if owner_segments.is_empty() {
        return None;
    }
    let owner = owner_segments.join("::");
    if schema.variant_fact(&owner, variant).is_some() {
        return Some(owner);
    }
    if owner.contains("::") {
        return None;
    }
    let mut matches = schema.variants().filter_map(|candidate| {
        (candidate.name == variant
            && candidate
                .owner
                .rsplit("::")
                .next()
                .is_some_and(|short| short == owner))
        .then_some(candidate.owner)
    });
    let matched = matches.next()?;
    matches.next().is_none().then_some(matched)
}
