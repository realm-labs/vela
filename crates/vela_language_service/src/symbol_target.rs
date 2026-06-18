use vela_analysis::{
    registry::RegistryFacts,
    stdlib::{stdlib_function_completion_facts, stdlib_method_fact},
    type_fact::TypeFact,
};
use vela_common::Span;
use vela_hir::{
    binding::{BindingMap, BindingResolution, LocalBinding},
    module_graph::{Declaration, DeclarationKind, Import, ImportResolution, ModulePath},
    type_hint::ImplMetadataKind,
};

use crate::{
    LanguageServiceDatabases, QueryContext, SymbolRef, TextRange, path_calls,
    symbol_ref::{
        builtin_member_symbol, builtin_symbol, schema_member_symbol, schema_symbol,
        schema_variant_symbol, source_enum_variant_symbol, source_impl_method_symbol,
        source_member_symbol, source_module_symbol_from_segments, source_symbol_for_declaration,
    },
};

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
        let symbol = symbol_ref_for(databases, query, &text, member_receiver_fact.as_ref());
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
            return Some((span, schema_variant_symbol(&owner, variant)));
        }
        None
    }
}

fn symbol_ref_for(
    databases: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    text: &str,
    member_receiver_fact: Option<&TypeFact>,
) -> Option<SymbolRef> {
    symbol_ref_from_bindings(databases, query, text)
        .or_else(|| symbol_ref_for_source_declaration(databases, query, text))
        .or_else(|| symbol_ref_for_import(databases, query, text))
        .or_else(|| {
            member_receiver_fact
                .and_then(|receiver| script_member_symbol_ref(databases, text, receiver))
        })
        .or_else(|| fact_symbol_ref_for(databases, text, member_receiver_fact))
}

fn symbol_ref_from_bindings(
    databases: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    text: &str,
) -> Option<SymbolRef> {
    let range = query.identifier_range()?;
    let bindings = query.bindings()?;
    symbol_ref_from_resolution(databases, bindings, text, range)
        .or_else(|| local_symbol_at_range(databases, bindings, text, range))
}

fn symbol_ref_from_resolution(
    databases: &LanguageServiceDatabases,
    bindings: &BindingMap,
    text: &str,
    range: TextRange,
) -> Option<SymbolRef> {
    let graph = databases.hir_db().graph();
    let resolution = bindings
        .resolutions()
        .filter_map(|(expression, resolution)| {
            let expression = bindings.expression(expression)?;
            let start = usize::try_from(expression.span.start).ok()?;
            let end = usize::try_from(expression.span.end).ok()?;
            (start <= range.start && range.end <= end)
                .then_some((end.saturating_sub(start), resolution))
        })
        .min_by_key(|(len, _)| *len)?
        .1;
    match resolution {
        BindingResolution::Local(local) => {
            let binding = bindings.local(*local)?;
            Some(local_symbol_for_binding(databases, binding))
        }
        BindingResolution::Declaration(declaration) => graph
            .declaration(*declaration)
            .and_then(|declaration| source_symbol_for_declaration_target(graph, declaration, text)),
        BindingResolution::Import(_) | BindingResolution::QualifiedPath(_) => None,
    }
}

fn local_symbol_at_range(
    databases: &LanguageServiceDatabases,
    bindings: &BindingMap,
    text: &str,
    range: TextRange,
) -> Option<SymbolRef> {
    bindings
        .locals()
        .find(|binding| {
            binding.name == text
                && local_name_range(databases, binding)
                    .is_some_and(|name_range| contains_range(name_range, range))
        })
        .map(|binding| local_symbol_for_binding(databases, binding))
}

fn symbol_ref_for_source_declaration(
    databases: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    text: &str,
) -> Option<SymbolRef> {
    let range = query.identifier_range()?;
    let source_id = query.source_id()?;
    let graph = databases.hir_db().graph();
    graph
        .declarations()
        .find(|declaration| {
            declaration.name == text
                && declaration.span.source == source_id
                && declaration_name_range(databases, declaration)
                    .is_some_and(|name_range| contains_range(name_range, range))
        })
        .map(|declaration| source_symbol_for_declaration(graph, declaration))
}

fn symbol_ref_for_import(
    databases: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    text: &str,
) -> Option<SymbolRef> {
    let range = query.identifier_range()?;
    let source_id = query.source_id()?;
    let graph = databases.hir_db().graph();
    let module = graph.module_id(query.module_path()?)?;
    graph.imports(module)?.iter().find_map(|import| {
        if import.span.source != source_id {
            return None;
        }
        let segment = import_segment_index(query.text(), import, text, range)?;
        if segment + 1 == import.path.len() {
            let ImportResolution::Declaration(declaration) = import.resolution?;
            let declaration = graph.declaration(declaration)?;
            return Some(source_symbol_for_declaration(graph, declaration));
        }
        let path = import.path[..=segment].to_vec();
        graph
            .module_id(&ModulePath::new(path.iter().cloned()))
            .map(|_| source_module_symbol_from_segments(path.iter()))
    })
}

fn script_member_symbol_ref(
    databases: &LanguageServiceDatabases,
    text: &str,
    receiver: &TypeFact,
) -> Option<SymbolRef> {
    script_field_symbol_ref(databases, text, receiver)
        .or_else(|| script_method_symbol_ref(databases, text, receiver))
        .or_else(|| script_trait_method_symbol_ref(databases, text, receiver))
}

fn script_field_symbol_ref(
    databases: &LanguageServiceDatabases,
    text: &str,
    receiver: &TypeFact,
) -> Option<SymbolRef> {
    let graph = databases.hir_db().graph();
    let owner_names = record_owner_names(receiver);
    graph.declarations().find_map(|declaration| {
        if declaration.kind != DeclarationKind::Struct
            || !owner_names
                .iter()
                .any(|owner| declaration_name_matches(graph, declaration, owner))
        {
            return None;
        }
        graph
            .struct_shape(declaration.id)?
            .fields
            .iter()
            .any(|field| field.name == text)
            .then(|| source_member_symbol(graph, declaration.id, text))?
    })
}

fn script_method_symbol_ref(
    databases: &LanguageServiceDatabases,
    text: &str,
    receiver: &TypeFact,
) -> Option<SymbolRef> {
    let graph = databases.hir_db().graph();
    let owner_names = record_owner_names(receiver);
    graph.declarations().find_map(|declaration| {
        if declaration.kind != DeclarationKind::Impl {
            return None;
        }
        let metadata = graph.impl_metadata(declaration.id)?;
        if !matches!(metadata.kind, ImplMetadataKind::Inherent)
            || !owner_names
                .iter()
                .any(|owner| impl_target_matches(&metadata.target_path, owner))
        {
            return None;
        }
        metadata
            .methods
            .iter()
            .any(|method| method.name == text)
            .then(|| source_impl_method_symbol(graph, declaration.id, text))?
    })
}

fn script_trait_method_symbol_ref(
    databases: &LanguageServiceDatabases,
    text: &str,
    receiver: &TypeFact,
) -> Option<SymbolRef> {
    let graph = databases.hir_db().graph();
    let owner_names = trait_owner_names(receiver);
    graph.declarations().find_map(|declaration| {
        if declaration.kind != DeclarationKind::Trait
            || !owner_names
                .iter()
                .any(|owner| declaration_name_matches(graph, declaration, owner))
        {
            return None;
        }
        graph
            .trait_shape(declaration.id)?
            .methods
            .iter()
            .any(|method| method.name == text)
            .then(|| source_member_symbol(graph, declaration.id, text))?
    })
}

fn source_symbol_for_declaration_target(
    graph: &vela_hir::module_graph::ModuleGraph,
    declaration: &Declaration,
    text: &str,
) -> Option<SymbolRef> {
    graph
        .enum_shape(declaration.id)
        .and_then(|shape| {
            shape
                .variants
                .iter()
                .find(|variant| variant.name == text)
                .and_then(|variant| {
                    source_enum_variant_symbol(graph, declaration.id, &variant.name)
                })
        })
        .or_else(|| Some(source_symbol_for_declaration(graph, declaration)))
}

fn local_symbol_for_binding(
    databases: &LanguageServiceDatabases,
    binding: &LocalBinding,
) -> SymbolRef {
    let Some(source) = databases
        .source_db()
        .records()
        .values()
        .find(|source| source.source_id() == binding.span.source)
    else {
        return SymbolRef::local(binding.name.clone());
    };
    SymbolRef::local_from_span(
        binding.name.clone(),
        source.document_id().clone(),
        source.text(),
        binding.span,
    )
}

fn local_name_range(
    databases: &LanguageServiceDatabases,
    binding: &LocalBinding,
) -> Option<TextRange> {
    let source = databases
        .source_db()
        .records()
        .values()
        .find(|source| source.source_id() == binding.span.source)?;
    let span_range = span_text_range(binding.span)?;
    name_range_in_text(source.text(), span_range, &binding.name)
}

fn declaration_name_range(
    databases: &LanguageServiceDatabases,
    declaration: &Declaration,
) -> Option<TextRange> {
    let source = databases
        .source_db()
        .records()
        .values()
        .find(|source| source.source_id() == declaration.span.source)?;
    let span_range = span_text_range(declaration.span)?;
    name_range_in_text(source.text(), span_range, &declaration.name)
}

fn fact_symbol_ref_for(
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
            return Some(schema_member_symbol(&owner, text));
        }
        if stdlib_method_fact(receiver_fact, text, None).is_some() {
            return Some(builtin_member_symbol(&receiver_fact.display_name(), text));
        }
    }
    schema_symbol_ref(schema, text).or_else(|| stdlib_function_symbol_ref(text))
}

fn span_text_range(span: Span) -> Option<TextRange> {
    let start = usize::try_from(span.start).ok()?;
    let end = usize::try_from(span.end).ok()?;
    Some(TextRange::new(start, end))
}

fn name_range_in_text(text: &str, range: TextRange, name: &str) -> Option<TextRange> {
    let slice = text.get(range.start..range.end)?;
    let relative = slice.find(name)?;
    let start = range.start + relative;
    Some(TextRange::new(start, start + name.len()))
}

fn contains_range(container: TextRange, contained: TextRange) -> bool {
    container.start <= contained.start && contained.end <= container.end
}

fn import_segment_index(
    text: &str,
    import: &Import,
    segment_text: &str,
    range: TextRange,
) -> Option<usize> {
    let import_range = span_text_range(import.span)?;
    if !contains_range(import_range, range) {
        return None;
    }
    let import_text = text.get(import_range.start..import_range.end)?;
    let mut search_start = 0usize;
    for (index, segment) in import.path.iter().enumerate() {
        if segment != segment_text {
            search_start = search_start.saturating_add(segment.len() + "::".len());
            continue;
        }
        let relative = import_text.get(search_start..)?.find(segment)? + search_start;
        let segment_range = TextRange::new(
            import_range.start + relative,
            import_range.start + relative + segment.len(),
        );
        if contains_range(segment_range, range) {
            return Some(index);
        }
        search_start = relative + segment.len();
    }
    None
}

fn record_owner_names(fact: &TypeFact) -> Vec<String> {
    let mut names = Vec::new();
    collect_record_owner_names(fact, &mut names);
    names
}

fn collect_record_owner_names(fact: &TypeFact, names: &mut Vec<String>) {
    match fact {
        TypeFact::Record { name } => push_owner_names(names, name),
        TypeFact::Union(facts) => {
            for fact in facts {
                collect_record_owner_names(fact, names);
            }
        }
        TypeFact::Unknown
        | TypeFact::Never
        | TypeFact::Any
        | TypeFact::Primitive(_)
        | TypeFact::Range
        | TypeFact::Array { .. }
        | TypeFact::Map { .. }
        | TypeFact::Set { .. }
        | TypeFact::Iterator { .. }
        | TypeFact::Option { .. }
        | TypeFact::OptionSome { .. }
        | TypeFact::OptionNone
        | TypeFact::Result { .. }
        | TypeFact::ResultOk { .. }
        | TypeFact::ResultErr { .. }
        | TypeFact::Function { .. }
        | TypeFact::Enum { .. }
        | TypeFact::Host { .. }
        | TypeFact::Trait { .. }
        | TypeFact::Module { .. } => {}
    }
}

fn trait_owner_names(fact: &TypeFact) -> Vec<String> {
    let mut names = Vec::new();
    collect_trait_owner_names(fact, &mut names);
    names
}

fn collect_trait_owner_names(fact: &TypeFact, names: &mut Vec<String>) {
    match fact {
        TypeFact::Trait { name } => push_owner_names(names, name),
        TypeFact::Union(facts) => {
            for fact in facts {
                collect_trait_owner_names(fact, names);
            }
        }
        TypeFact::Unknown
        | TypeFact::Never
        | TypeFact::Any
        | TypeFact::Primitive(_)
        | TypeFact::Range
        | TypeFact::Array { .. }
        | TypeFact::Map { .. }
        | TypeFact::Set { .. }
        | TypeFact::Iterator { .. }
        | TypeFact::Option { .. }
        | TypeFact::OptionSome { .. }
        | TypeFact::OptionNone
        | TypeFact::Result { .. }
        | TypeFact::ResultOk { .. }
        | TypeFact::ResultErr { .. }
        | TypeFact::Function { .. }
        | TypeFact::Enum { .. }
        | TypeFact::Host { .. }
        | TypeFact::Record { .. }
        | TypeFact::Module { .. } => {}
    }
}

fn push_owner_names(names: &mut Vec<String>, name: &str) {
    if !names.iter().any(|owner| owner == name) {
        names.push(name.to_owned());
    }
    if let Some(short) = name.rsplit("::").next()
        && short != name
        && !names.iter().any(|owner| owner == short)
    {
        names.push(short.to_owned());
    }
}

fn declaration_name_matches(
    graph: &vela_hir::module_graph::ModuleGraph,
    declaration: &Declaration,
    owner: &str,
) -> bool {
    declaration.name == owner
        || graph
            .module_path(declaration.module)
            .map(|path| {
                let module = path.join();
                if module.is_empty() {
                    declaration.name.clone()
                } else {
                    format!("{module}::{}", declaration.name)
                }
            })
            .is_some_and(|qualified| qualified == owner)
}

fn impl_target_matches(path: &[String], owner: &str) -> bool {
    path.last().is_some_and(|name| name == owner) || path.join("::") == owner
}

fn schema_symbol_ref(schema: &RegistryFacts, text: &str) -> Option<SymbolRef> {
    if schema.type_fact(text).is_some()
        || schema.trait_fact(text).is_some()
        || schema.function_fact(text).is_some()
    {
        return Some(schema_symbol(text));
    }
    if let Some((owner, variant)) = text.rsplit_once("::")
        && schema.variant_fact(owner, variant).is_some()
    {
        return Some(schema_variant_symbol(owner, variant));
    }
    let mut variants = schema.variants().filter(|variant| variant.name == text);
    let variant = variants.next()?;
    variants
        .next()
        .is_none()
        .then(|| schema_variant_symbol(&variant.owner, &variant.name))
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
        .map(|function| builtin_symbol(function.name))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DocumentId, Position, SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot,
        assemble_project_sources,
    };

    #[test]
    fn target_resolves_local_symbol_from_bindings() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub fn main(amount: i64) -> i64 { return amount }";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
        let query = QueryContext::from_databases(
            &databases,
            &document,
            Position::new(0, text.rfind("amount").expect("local use should exist")),
        )
        .expect("query should exist");

        let target = SymbolTarget::from_query(&databases, &query).expect("target should resolve");

        assert_eq!(
            target.symbol(),
            Some(&SymbolRef::local_at(
                "amount",
                document,
                TextRange::new(12, 18)
            ))
        );
    }

    #[test]
    fn target_resolves_source_declaration_symbol_from_bindings() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "fn grant() { return 1 }\nfn main() { return grant() }";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
        let query = QueryContext::from_databases(
            &databases,
            &document,
            Position::new(
                1,
                text.lines()
                    .nth(1)
                    .expect("call line should exist")
                    .find("grant")
                    .expect("grant call should exist"),
            ),
        )
        .expect("query should exist");

        let target = SymbolTarget::from_query(&databases, &query).expect("target should resolve");

        assert_eq!(
            target.symbol(),
            Some(&SymbolRef::Source("game::main::grant".to_owned()))
        );
    }

    #[test]
    fn target_resolves_source_enum_variant_symbol_from_bindings() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "enum Reward { Coins(amount: i64) }\nfn main() { return Reward::Coins(1) }";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
        let query = QueryContext::from_databases(
            &databases,
            &document,
            Position::new(
                1,
                text.lines()
                    .nth(1)
                    .expect("call line should exist")
                    .find("Coins")
                    .expect("variant call should exist"),
            ),
        )
        .expect("query should exist");

        let target = SymbolTarget::from_query(&databases, &query).expect("target should resolve");

        assert_eq!(
            target.symbol(),
            Some(&SymbolRef::Source("game::main::Reward::Coins".to_owned()))
        );
    }

    #[test]
    fn target_resolves_imported_declaration_symbol() {
        let main = DocumentId::from("/workspace/scripts/game/main.vela");
        let reward = DocumentId::from("/workspace/scripts/game/reward.vela");
        let main_text = "use game::reward::grant\nfn main() { return grant() }";
        let databases = databases_for(vec![
            SourceFileSnapshot::new(main.clone(), main_text),
            SourceFileSnapshot::new(reward, "pub fn grant() -> i64 { return 1 }"),
        ]);
        let query = QueryContext::from_databases(
            &databases,
            &main,
            Position::new(0, main_text.find("grant").expect("import should exist")),
        )
        .expect("query should exist");

        let target = SymbolTarget::from_query(&databases, &query).expect("target should resolve");

        assert_eq!(
            target.symbol(),
            Some(&SymbolRef::Source("game::reward::grant".to_owned()))
        );
    }

    #[test]
    fn target_resolves_import_module_segment_symbol() {
        let main = DocumentId::from("/workspace/scripts/game/main.vela");
        let reward = DocumentId::from("/workspace/scripts/game/reward.vela");
        let main_text = "use game::reward::grant\nfn main() { return grant() }";
        let databases = databases_for(vec![
            SourceFileSnapshot::new(main.clone(), main_text),
            SourceFileSnapshot::new(reward, "pub fn grant() -> i64 { return 1 }"),
        ]);
        let query = QueryContext::from_databases(
            &databases,
            &main,
            Position::new(0, main_text.find("reward").expect("module segment")),
        )
        .expect("query should exist");

        let target = SymbolTarget::from_query(&databases, &query).expect("target should resolve");

        assert_eq!(
            target.symbol(),
            Some(&SymbolRef::Source("game::reward".to_owned()))
        );
    }

    #[test]
    fn target_resolves_script_field_symbol_from_receiver_fact() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"struct Player {
    level: i64,
}
fn main(player: Player) {
    return player.level
}"#;
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
        let use_line = text.lines().nth(4).expect("field use line should exist");
        let query = QueryContext::from_databases(
            &databases,
            &document,
            Position::new(4, use_line.find("level").expect("field use should exist")),
        )
        .expect("query should exist");

        let target = SymbolTarget::from_query(&databases, &query).expect("target should resolve");

        assert_eq!(
            target.symbol(),
            Some(&SymbolRef::Source("game::main::Player.level".to_owned()))
        );
    }

    #[test]
    fn target_resolves_script_method_symbol_from_receiver_fact() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"struct Player {
    level: i64,
}
impl Player {
    fn grant(amount: i64) -> bool {
        return amount > 0
    }
}
fn main(player: Player) {
    return player.grant(3)
}"#;
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
        let use_line = text.lines().nth(9).expect("method use line should exist");
        let query = QueryContext::from_databases(
            &databases,
            &document,
            Position::new(9, use_line.find("grant").expect("method use should exist")),
        )
        .expect("query should exist");

        let target = SymbolTarget::from_query(&databases, &query).expect("target should resolve");

        assert_eq!(
            target.symbol(),
            Some(&SymbolRef::Source("game::main::Player.grant".to_owned()))
        );
    }

    fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        databases
    }
}
