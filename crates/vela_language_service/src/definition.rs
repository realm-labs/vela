use vela_analysis::type_fact::TypeFact;
use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingMap, BindingResolution, LocalBinding};
use vela_hir::module_graph::{Declaration, DeclarationKind, ImportResolution, ModuleGraph};

use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, LineIndex, Position, QueryContext,
    SymbolRef, TextRange,
    callable_context::callable_facts,
    member_access, path_calls,
    symbol_ref::{
        qualified_source_declaration_name, source_enum_variant_symbol,
        source_symbol_for_declaration,
    },
    symbol_target::SymbolTarget,
};

mod source_members;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Definition {
    document_id: DocumentId,
    range: DiagnosticRange,
    symbol: Option<SymbolRef>,
}

impl Definition {
    #[must_use]
    pub fn document_id(&self) -> &DocumentId {
        &self.document_id
    }

    #[must_use]
    pub const fn range(&self) -> DiagnosticRange {
        self.range
    }

    #[must_use]
    pub fn symbol(&self) -> Option<&SymbolRef> {
        self.symbol.as_ref()
    }
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn definition(&self, document_id: &DocumentId, position: Position) -> Option<Definition> {
        let query = QueryContext::from_databases(self, document_id, position)?;
        let target = SymbolTarget::from_query(self, &query)?;
        let source_id = query.source_id()?;
        let offset = u32::try_from(target.range().start).ok()?;
        let graph = self.hir_db().graph();

        if target.is_schema_symbol()
            && let Some(definition) = target.schema_member_span(self).and_then(|span| {
                self.definition_from_span_with_symbol(span, target.symbol().cloned())
            })
        {
            return Some(definition);
        }

        if let Some(definition) = target
            .schema_variant_target(self, &query)
            .and_then(|(span, symbol)| self.definition_from_span_with_symbol(span, Some(symbol)))
        {
            return Some(definition);
        }

        if let Some(definition) = source_members::source_member_definition_for_target(self, &target)
        {
            return Some(definition);
        }

        if query.member_receiver_range().is_some() {
            return None;
        }

        for declaration in graph.declarations() {
            if declaration.span.source != source_id || !declaration.span.contains(offset) {
                continue;
            }
            let Some(bindings) = graph.bindings(declaration.id) else {
                continue;
            };
            if let Some(definition) = definition_from_resolution_at_target(bindings, &target, self)
            {
                return Some(definition);
            }
            if let Some(binding) = local_declaration_at_target(bindings, &target, self) {
                return self.definition_from_span_with_symbol(
                    binding.span,
                    Some(
                        target
                            .symbol()
                            .cloned()
                            .unwrap_or_else(|| self.definition_local_symbol_for_binding(binding)),
                    ),
                );
            }
        }

        if target.is_schema_symbol() {
            return self.schema_definition_for_target(&target);
        }

        graph
            .declarations()
            .find(|declaration| {
                declaration.span.source == source_id
                    && self.declaration_name_contains_target(declaration, &target)
            })
            .and_then(|declaration| self.definition_from_declaration(declaration))
    }

    #[must_use]
    pub fn declaration(&self, document_id: &DocumentId, position: Position) -> Option<Definition> {
        self.definition(document_id, position)
    }

    #[must_use]
    pub fn type_definition(
        &self,
        document_id: &DocumentId,
        position: Position,
    ) -> Option<Definition> {
        let query = QueryContext::from_databases(self, document_id, position)?;
        let target = SymbolTarget::from_query(self, &query)?;

        if let Some(fact) = self.member_type_fact_for_target(&target) {
            return self.type_definition_for_fact(&fact);
        }

        if let Some(fact) = query.type_fact_for_range(self, target.range())
            && let Some(definition) = self.type_definition_for_fact(&fact)
        {
            return Some(definition);
        }

        if let Some(definition) = self.call_return_type_definition(&query, &target) {
            return Some(definition);
        }

        if let Some(definition) = self.member_call_return_type_definition(&query, &target) {
            return Some(definition);
        }

        if let Some(definition) = self.source_enum_variant_owner_type_definition(&target) {
            return Some(definition);
        }

        if let Some(definition) = self.imported_source_type_definition_for_target(&query, &target) {
            return Some(definition);
        }

        if target.is_schema_symbol() {
            return self.schema_type_definition_for_name(target.text());
        }

        None
    }

    fn definition_from_span_with_symbol(
        &self,
        span: Span,
        symbol: Option<SymbolRef>,
    ) -> Option<Definition> {
        let source = self.source_record_for(span.source)?;
        let start = usize::try_from(span.start).ok()?;
        let end = usize::try_from(span.end).ok()?;
        let range = diagnostic_range(source.text(), TextRange::new(start, end));
        Some(Definition {
            document_id: source.document_id().clone(),
            range,
            symbol,
        })
    }

    fn definition_from_declaration(&self, declaration: &Declaration) -> Option<Definition> {
        let source = self.source_record_for(declaration.span.source)?;
        let start = usize::try_from(declaration.span.start).ok()?;
        let end = usize::try_from(declaration.span.end).ok()?;
        let range = name_range_in_text(
            source.text(),
            TextRange::new(start, end),
            declaration.name.as_str(),
        )
        .unwrap_or(TextRange::new(start, end));
        Some(Definition {
            document_id: source.document_id().clone(),
            range: diagnostic_range(source.text(), range),
            symbol: Some(source_symbol_for_declaration(
                self.hir_db().graph(),
                declaration,
            )),
        })
    }

    fn declaration_name_contains_target(
        &self,
        declaration: &Declaration,
        target: &SymbolTarget,
    ) -> bool {
        let Some(source) = self.source_record_for(declaration.span.source) else {
            return false;
        };
        let Ok(start) = usize::try_from(declaration.span.start) else {
            return false;
        };
        let Ok(end) = usize::try_from(declaration.span.end) else {
            return false;
        };
        let Some(name_range) = name_range_in_text(
            source.text(),
            TextRange::new(start, end),
            declaration.name.as_str(),
        ) else {
            return false;
        };
        name_range.start <= target.range().start && target.range().end <= name_range.end
    }

    fn source_record_for(&self, source_id: SourceId) -> Option<&crate::SourceRecord> {
        self.source_db()
            .records()
            .values()
            .find(|record| record.source_id() == source_id)
    }

    fn schema_definition_for_target(&self, target: &SymbolTarget) -> Option<Definition> {
        if !target.is_schema_symbol() {
            return None;
        }
        target
            .schema_symbol_span(self)
            .and_then(|span| self.definition_from_span_with_symbol(span, target.symbol().cloned()))
    }

    fn member_type_fact_for_target(&self, target: &SymbolTarget) -> Option<TypeFact> {
        source_members::source_field_type_fact_for_target(self, target)
            .or_else(|| self.schema_field_type_fact_for_target(target))
    }

    fn schema_field_type_fact_for_target(&self, target: &SymbolTarget) -> Option<TypeFact> {
        let owner = target.member_receiver_fact().and_then(fact_owner_name)?;
        self.schema_db()
            .facts()
            .field_fact(&owner, target.text())
            .cloned()
    }

    fn call_return_type_definition(
        &self,
        query: &QueryContext<'_>,
        target: &SymbolTarget,
    ) -> Option<Definition> {
        let parsed = query.syntax_parse()?;
        let call_site = path_calls::path_call_sites(parsed)
            .into_iter()
            .find(|site| site.segment_range == target.range())?;
        let callee = call_site.path.join("::");
        callable_facts(self, &callee)
            .iter()
            .find_map(|callable| self.type_definition_for_fact(callable.returns()))
    }

    fn member_call_return_type_definition(
        &self,
        query: &QueryContext<'_>,
        target: &SymbolTarget,
    ) -> Option<Definition> {
        let parsed = query.syntax_parse()?;
        let call_site = member_access::member_call_sites(parsed)
            .into_iter()
            .find(|site| site.member_range == target.range())?;
        let args_prefix = query.call_args_prefix_text().unwrap_or("");
        query
            .member_callable_facts(
                self,
                call_site.receiver_range,
                &call_site.member,
                args_prefix,
            )
            .iter()
            .find_map(|callable| self.type_definition_for_fact(callable.returns()))
    }

    fn type_definition_for_fact(&self, fact: &TypeFact) -> Option<Definition> {
        match fact {
            TypeFact::Record { name } => self
                .source_type_definition_for_name(name, DeclarationKind::Struct)
                .or_else(|| self.schema_type_definition_for_name(name)),
            TypeFact::Enum { name, .. } => self
                .source_type_definition_for_name(name, DeclarationKind::Enum)
                .or_else(|| self.schema_type_definition_for_name(name)),
            TypeFact::Host { name } => self.schema_type_definition_for_name(name),
            TypeFact::Trait { name } => self
                .source_type_definition_for_name(name, DeclarationKind::Trait)
                .or_else(|| self.schema_trait_definition_for_name(name)),
            TypeFact::Union(facts) => facts
                .iter()
                .find_map(|fact| self.type_definition_for_fact(fact)),
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
            | TypeFact::Module { .. } => None,
        }
    }

    fn source_enum_variant_owner_type_definition(
        &self,
        target: &SymbolTarget,
    ) -> Option<Definition> {
        let symbol = target.symbol()?;
        let graph = self.hir_db().graph();
        graph
            .declarations()
            .filter(|declaration| declaration.kind == DeclarationKind::Enum)
            .find_map(|declaration| {
                let shape = graph.enum_shape(declaration.id)?;
                shape
                    .variants
                    .iter()
                    .any(|variant| {
                        source_enum_variant_symbol(graph, declaration.id, &variant.name).as_ref()
                            == Some(symbol)
                    })
                    .then(|| self.definition_from_declaration(declaration))?
            })
    }

    fn imported_source_type_definition_for_target(
        &self,
        query: &QueryContext<'_>,
        target: &SymbolTarget,
    ) -> Option<Definition> {
        if matches!(target.symbol(), Some(SymbolRef::Local(_))) {
            return None;
        }

        let graph = self.hir_db().graph();
        let module = graph.module_id(query.module_path()?)?;
        graph.imports(module)?.iter().find_map(|import| {
            let binding_name = import
                .alias
                .as_deref()
                .or_else(|| import.path.last().map(String::as_str))?;
            if binding_name != target.text() {
                return None;
            }
            let ImportResolution::Declaration(declaration_id) = import.resolution?;
            let declaration = graph.declaration(declaration_id)?;
            matches!(
                declaration.kind,
                DeclarationKind::Struct | DeclarationKind::Enum | DeclarationKind::Trait
            )
            .then(|| self.definition_from_declaration(declaration))?
        })
    }

    fn source_type_definition_for_name(
        &self,
        name: &str,
        kind: DeclarationKind,
    ) -> Option<Definition> {
        let declaration = source_declaration_for_fact_name(self.hir_db().graph(), name, kind)?;
        self.definition_from_declaration(declaration)
    }

    fn schema_type_definition_for_name(&self, name: &str) -> Option<Definition> {
        self.schema_db()
            .source_locations()
            .type_span(name)
            .or_else(|| {
                short_name(name)
                    .and_then(|short| self.schema_db().source_locations().type_span(short))
            })
            .and_then(|span| {
                self.definition_from_span_with_symbol(span, Some(SymbolRef::Schema(name.into())))
            })
    }

    fn schema_trait_definition_for_name(&self, name: &str) -> Option<Definition> {
        self.schema_db()
            .source_locations()
            .trait_span(name)
            .or_else(|| {
                short_name(name)
                    .and_then(|short| self.schema_db().source_locations().trait_span(short))
            })
            .and_then(|span| {
                self.definition_from_span_with_symbol(span, Some(SymbolRef::Schema(name.into())))
            })
    }

    fn definition_local_symbol_for_binding(&self, binding: &LocalBinding) -> SymbolRef {
        let Some(source) = self.source_record_for(binding.span.source) else {
            return SymbolRef::local(binding.name.clone());
        };
        SymbolRef::local_from_span(
            binding.name.clone(),
            source.document_id().clone(),
            source.text(),
            binding.span,
        )
    }
}

fn definition_from_resolution_at_target(
    bindings: &BindingMap,
    target: &SymbolTarget,
    databases: &LanguageServiceDatabases,
) -> Option<Definition> {
    let graph = databases.hir_db().graph();
    let resolution = bindings
        .resolutions()
        .filter_map(|(expression, resolution)| {
            let expression = bindings.expression(expression)?;
            let start = usize::try_from(expression.span.start).ok()?;
            let end = usize::try_from(expression.span.end).ok()?;
            (start <= target.range().start && target.range().end <= end)
                .then_some((end.saturating_sub(start), resolution))
        })
        .min_by_key(|(len, _)| *len)?
        .1;

    match resolution {
        BindingResolution::Local(local) => {
            let binding = bindings.local(*local)?;
            let symbol = target
                .symbol()
                .cloned()
                .unwrap_or_else(|| databases.definition_local_symbol_for_binding(binding));
            databases.definition_from_span_with_symbol(binding.span, Some(symbol))
        }
        BindingResolution::Declaration(declaration) => {
            let declaration = graph.declaration(*declaration)?;
            let mut definition = databases.definition_from_declaration(declaration)?;
            let declaration_symbol = source_symbol_for_declaration(graph, declaration);
            if target.symbol() == Some(&declaration_symbol) {
                definition.symbol = Some(declaration_symbol);
            }
            Some(definition)
        }
        BindingResolution::Import(_) | BindingResolution::QualifiedPath(_) => None,
    }
}

fn local_declaration_at_target<'a>(
    bindings: &'a BindingMap,
    target: &SymbolTarget,
    databases: &LanguageServiceDatabases,
) -> Option<&'a LocalBinding> {
    bindings.locals().find(|binding| {
        let Ok(start) = usize::try_from(binding.span.start) else {
            return false;
        };
        let Ok(end) = usize::try_from(binding.span.end) else {
            return false;
        };
        let Some(source) = databases.source_record_for(binding.span.source) else {
            return false;
        };
        let Some(name_range) =
            name_range_in_text(source.text(), TextRange::new(start, end), &binding.name)
        else {
            return false;
        };
        name_range.start <= target.range().start && target.range().end <= name_range.end
    })
}

fn diagnostic_range(text: &str, range: TextRange) -> DiagnosticRange {
    let line_index = LineIndex::new(text);
    DiagnosticRange::new(
        line_index.position(range.start),
        line_index.position(range.end),
    )
}

fn name_range_in_text(text: &str, range: TextRange, name: &str) -> Option<TextRange> {
    let slice = text.get(range.start..range.end)?;
    let relative = slice.find(name)?;
    let start = range.start + relative;
    Some(TextRange::new(start, start + name.len()))
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

fn source_declaration_for_fact_name<'a>(
    graph: &'a ModuleGraph,
    name: &str,
    kind: DeclarationKind,
) -> Option<&'a Declaration> {
    graph
        .declarations()
        .find(|declaration| {
            declaration.kind == kind
                && qualified_source_declaration_name(graph, declaration) == name
        })
        .or_else(|| {
            let short = short_name(name).unwrap_or(name);
            let mut matches = graph
                .declarations()
                .filter(|declaration| declaration.kind == kind && declaration.name == short);
            let declaration = matches.next()?;
            matches.next().is_none().then_some(declaration)
        })
}

fn short_name(name: &str) -> Option<&str> {
    name.rsplit("::").next().filter(|short| *short != name)
}

#[cfg(test)]
mod dynamic_tests;
#[cfg(test)]
mod schema_return_tests;
#[cfg(test)]
mod source_return_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod type_tests;
