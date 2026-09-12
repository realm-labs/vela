use vela_analysis::type_fact::TypeFact;
use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingMap, BindingResolution, LocalBinding};
use vela_hir::body::HirPathKind;
use vela_hir::module_graph::{Declaration, DeclarationKind, ImportResolution, ModuleGraph};

use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, LineIndex, Position, QueryContext,
    SymbolRef, TextRange,
    callable_context::{CallableOrigin, callable_facts, source_callable_facts_for_declaration},
    hir_path_sites,
    query_context::binding_resolution_for_source_range,
    symbol_ref::{
        qualified_source_declaration_name, source_enum_variant_symbol,
        source_symbol_for_declaration,
    },
    symbol_target::SymbolTarget,
};

mod impl_headers;
mod imports;
mod named_arguments;
mod owned_declarations;
mod record_fields;
mod source_callables;
mod source_members;
mod source_variants;
mod type_hints;

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

        if target.is_module_symbol(self) {
            return self.schema_definition_for_target(&target);
        }
        if let Some(navigation) = self.source_declaration_navigation(&query, &target) {
            return navigation.definition;
        }
        if let Some(declaration) = self.source_import_declaration(&query, &target) {
            return declaration
                .and_then(|declaration| self.definition_from_declaration(declaration));
        }
        if let Some(definition) = self.impl_header_definition(&query, &target) {
            return definition;
        }
        if let Some(definition) = self.source_type_hint_definition(&query, &target) {
            return definition;
        }
        if let Some(navigation) = self.record_field_navigation(&query, &target) {
            return navigation.definition;
        }
        if let Some(navigation) = self.named_argument_navigation(&query, &target) {
            return navigation.definition;
        }

        if let Some(navigation) = self.source_variant_navigation(&query, &target) {
            return navigation.definition;
        }

        if target.is_schema_symbol()
            && let Some(definition) = target.schema_member_span(self).and_then(|span| {
                self.definition_from_span_with_symbol(span, target.symbol().cloned())
            })
        {
            return Some(definition);
        }

        if let Some((owner, variant)) = target.schema_variant_identity(self, &query) {
            return self
                .schema_db()
                .source_locations()
                .variant_span(&owner, &variant)
                .and_then(|span| {
                    self.definition_from_span_with_symbol(
                        span,
                        Some(crate::symbol_ref::schema_variant_symbol(&owner, &variant)),
                    )
                });
        }

        if let Some(definition) = source_members::source_member_definition_for_target(self, &target)
        {
            return Some(definition);
        }

        if query.member_receiver_range().is_some() {
            return None;
        }

        if let Some(bindings) = query.bindings()
            && let Some(definition) =
                definition_from_resolution_at_target(bindings, &target, self, &query)
        {
            return Some(definition);
        }

        if target.is_schema_symbol() {
            return self.schema_definition_for_target(&target);
        }

        None
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

        if target.is_module_symbol(self) {
            return None;
        }
        if let Some(navigation) = self.source_declaration_navigation(&query, &target) {
            return navigation.type_definition;
        }
        if let Some(declaration) = self.source_import_declaration(&query, &target) {
            let declaration = declaration?;
            return match declaration.kind {
                DeclarationKind::Struct | DeclarationKind::Enum | DeclarationKind::Trait => {
                    self.definition_from_declaration(declaration)
                }
                _ => self
                    .graph_analysis_facts()
                    .declaration(declaration.id)
                    .and_then(|fact| self.type_definition_for_fact(fact)),
            };
        }
        if let Some(definition) = self.impl_header_definition(&query, &target) {
            return definition;
        }
        if let Some(definition) = self.source_type_hint_definition(&query, &target) {
            return definition;
        }

        if let Some(navigation) = self.record_field_navigation(&query, &target) {
            return navigation.type_definition;
        }
        if let Some(navigation) = self.named_argument_navigation(&query, &target) {
            return navigation.type_definition;
        }

        if let Some(navigation) = self.source_variant_navigation(&query, &target) {
            return navigation.type_definition;
        }

        if let Some((owner, variant)) = target.schema_variant_identity(self, &query) {
            return self
                .schema_db()
                .facts()
                .variant_fact(&owner, &variant)
                .and_then(|_| self.schema_type_definition_for_name(&owner));
        }

        if let Some(fact) = self.member_type_fact_for_target(&target) {
            return self.type_definition_for_fact(&fact);
        }

        if let Some(definition) = self.member_call_return_type_definition(&query, &target) {
            return definition;
        }

        if let Some(fact) = query.type_fact_for_range(self, target.range())
            && let Some(definition) = self.type_definition_for_fact(&fact)
        {
            return Some(definition);
        }

        if let Some(definition) = self.call_return_type_definition(&query, &target) {
            return Some(definition);
        }

        if let Some(definition) = self.source_enum_variant_owner_type_definition(&target) {
            return Some(definition);
        }

        if let Some(definition) = self.imported_source_type_definition_for_target(&query, &target) {
            return Some(definition);
        }

        if let Some(SymbolRef::Schema(name)) = target.symbol() {
            return self.schema_type_definition_for_name(name);
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
        // Metadata spans can outlive their source text. Reject invalid bounds
        // and UTF-8 splits before projecting them to editor positions.
        source.text().get(start..end)?;
        let range = diagnostic_range(source.text(), TextRange::new(start, end));
        Some(Definition {
            document_id: source.document_id().clone(),
            range,
            symbol,
        })
    }

    fn definition_from_declaration(&self, declaration: &Declaration) -> Option<Definition> {
        let source = self.source_record_for(declaration.name_span.source)?;
        let range = text_range_for_span(declaration.name_span)?;
        Some(Definition {
            document_id: source.document_id().clone(),
            range: diagnostic_range(source.text(), range),
            symbol: Some(source_symbol_for_declaration(
                self.hir_db().graph(),
                declaration,
            )),
        })
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
        let source = query.source_id()?;
        let call_site = self
            .hir_db()
            .graph()
            .paths_in_source_by_kind(source, HirPathKind::Callee)
            .filter_map(hir_path_sites::site)
            .find(|site| site.segment_range == target.range())?;
        let callee = call_site.path.join("::");
        let graph = self.hir_db().graph();
        if let Some(resolution) = query.bindings().and_then(|bindings| {
            binding_resolution_for_source_range(graph, bindings, target.range())
        }) {
            match resolution {
                BindingResolution::Declaration(id) => {
                    let callable = source_callable_facts_for_declaration(
                        graph,
                        self.schema_db().facts(),
                        self.graph_analysis_facts(),
                        graph.declaration(*id)?,
                    )?;
                    return self.type_definition_for_fact(callable.returns());
                }
                BindingResolution::Local(_) => {
                    let TypeFact::Function { returns, .. } =
                        query.type_fact_for_range(self, target.range())?
                    else {
                        return None;
                    };
                    return self.type_definition_for_fact(&returns);
                }
                BindingResolution::Import(_) | BindingResolution::QualifiedPath(_) => {}
            }
        }
        callable_facts(self, &callee)
            .iter()
            .filter(|callable| {
                !matches!(
                    callable.origin(),
                    CallableOrigin::Source
                        | CallableOrigin::SourceMethod
                        | CallableOrigin::SourceVariant
                )
            })
            .find_map(|callable| self.type_definition_for_fact(callable.returns()))
    }

    fn member_call_return_type_definition(
        &self,
        query: &QueryContext<'_>,
        target: &SymbolTarget,
    ) -> Option<Option<Definition>> {
        let source_id = query.source_id()?;
        let target_span = Span::new(
            source_id,
            u32::try_from(target.range().start).ok()?,
            u32::try_from(target.range().end).ok()?,
        );
        let graph = self.hir_db().graph();
        let call_field = graph
            .member_calls_in_source(source_id)
            .find(|field| field.member_origin.span == target_span)?;
        let receiver_range = graph
            .expression_span(call_field.receiver)
            .and_then(text_range_for_span)?;
        let args_prefix = query.call_args_prefix_text().unwrap_or("");
        let callables =
            query.member_callable_facts(self, receiver_range, &call_field.name, args_prefix);
        if callables
            .iter()
            .any(|callable| callable.origin() == CallableOrigin::SourceMethod)
        {
            // Source method selection already applies inherent precedence and
            // rejects ambiguous trait implementations. Reuse its exact target
            // before consulting return facts, which may include all candidates.
            let definition = (|| {
                let callee = source_members::source_member_definition_for_target(self, target)?;
                let (signature, _) = self.source_signature_for_navigation(&callee)?;
                let hint = signature.return_type.as_ref()?;
                let fact = crate::callable_context::query_type_fact_from_hint(
                    graph,
                    hint,
                    self.schema_db().facts(),
                );
                self.type_definition_for_fact(&fact)
            })();
            return Some(definition);
        }
        Some(
            callables
                .iter()
                .find_map(|callable| self.type_definition_for_fact(callable.returns())),
        )
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
            | TypeFact::ArrayView { .. }
            | TypeFact::ArrayMut { .. }
            | TypeFact::Map { .. }
            | TypeFact::MapView { .. }
            | TypeFact::MapMut { .. }
            | TypeFact::Set { .. }
            | TypeFact::SetView { .. }
            | TypeFact::SetMut { .. }
            | TypeFact::Iterator { .. }
            | TypeFact::ScopedIterator { .. }
            | TypeFact::Option { .. }
            | TypeFact::OptionSome { .. }
            | TypeFact::OptionNone
            | TypeFact::Result { .. }
            | TypeFact::ResultOk { .. }
            | TypeFact::ResultErr { .. }
            | TypeFact::Function { .. }
            | TypeFact::Closure
            | TypeFact::Tuple { .. }
            | TypeFact::LogicalRecord(_)
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
        let module = graph.module_id(query.module_key()?)?;
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
            .and_then(|span| {
                self.definition_from_span_with_symbol(span, Some(SymbolRef::Schema(name.into())))
            })
    }

    fn schema_trait_definition_for_name(&self, name: &str) -> Option<Definition> {
        self.schema_db()
            .source_locations()
            .trait_span(name)
            .and_then(|span| {
                self.definition_from_span_with_symbol(span, Some(SymbolRef::Schema(name.into())))
            })
    }

    fn definition_local_symbol_for_binding(&self, binding: &LocalBinding) -> SymbolRef {
        let Some(source) = self.source_record_for(binding.span.source) else {
            return SymbolRef::local(binding.name.clone());
        };
        SymbolRef::local_for_binding(binding, source.document_id().clone())
    }
}

fn definition_from_resolution_at_target(
    bindings: &BindingMap,
    target: &SymbolTarget,
    databases: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
) -> Option<Definition> {
    let graph = databases.hir_db().graph();
    let resolution = binding_resolution_for_source_range(graph, bindings, target.range())?;

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
        BindingResolution::QualifiedPath(path) => {
            databases.definition_from_imported_path(query, path)
        }
        BindingResolution::Import(_) => {
            let site = query
                .body()?
                .paths
                .iter()
                .filter_map(hir_path_sites::site)
                .find(|site| site.segment_range == target.range())?;
            databases.definition_from_imported_path(query, site.path)
        }
    }
}

fn diagnostic_range(text: &str, range: TextRange) -> DiagnosticRange {
    let line_index = LineIndex::new(text);
    DiagnosticRange::new(
        line_index.position(range.start),
        line_index.position(range.end),
    )
}

fn text_range_for_span(span: Span) -> Option<TextRange> {
    Some(TextRange::new(
        usize::try_from(span.start).ok()?,
        usize::try_from(span.end).ok()?,
    ))
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
            if name.contains("::") {
                return None;
            }
            let mut matches = graph
                .declarations()
                .filter(|declaration| declaration.kind == kind && declaration.name == name);
            let declaration = matches.next()?;
            matches.next().is_none().then_some(declaration)
        })
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
mod tuple_destructuring_tests;
#[cfg(test)]
mod type_tests;

#[cfg(test)]
mod matrix_tests;
#[cfg(test)]
mod schema_lifecycle_tests;
#[cfg(test)]
mod source_lifecycle_tests;
