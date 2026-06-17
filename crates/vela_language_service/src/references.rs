use std::collections::HashSet;

use vela_analysis::type_fact::TypeFact;
use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingMap, BindingResolution, LocalBinding};
use vela_hir::ids::{HirDeclId, HirLocalId};
use vela_hir::module_graph::{Declaration, DeclarationKind, ImportResolution, ModuleGraph};
use vela_hir::type_hint::ImplMetadataKind;
use vela_syntax::lexer::lex;
use vela_syntax::token::TokenKind;

use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, LineIndex, Position, QueryContext,
    TextRange, path_calls,
};

mod fields;
mod methods;
mod modules;
mod record_fields;
mod record_variant_patterns;
pub(crate) mod schema;
mod variant_fields;

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReferenceKind {
    Declaration,
    Import,
    Call,
    Pattern,
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DocumentHighlightKind {
    Text,
    Call,
    Read,
    Write,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Reference {
    document_id: DocumentId,
    range: DiagnosticRange,
    kind: ReferenceKind,
}

impl Reference {
    #[must_use]
    pub fn document_id(&self) -> &DocumentId {
        &self.document_id
    }

    #[must_use]
    pub const fn range(&self) -> DiagnosticRange {
        self.range
    }

    #[must_use]
    pub const fn kind(&self) -> ReferenceKind {
        self.kind
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DocumentHighlight {
    range: DiagnosticRange,
    kind: DocumentHighlightKind,
}

impl DocumentHighlight {
    #[must_use]
    pub const fn range(&self) -> DiagnosticRange {
        self.range
    }

    #[must_use]
    pub const fn kind(&self) -> DocumentHighlightKind {
        self.kind
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ReferenceToken {
    range: TextRange,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct EnumVariantReferenceTarget {
    owner: HirDeclId,
    variant: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct TraitReferenceTarget {
    owner: HirDeclId,
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn document_highlights(
        &self,
        document_id: &DocumentId,
        position: Position,
    ) -> Vec<DocumentHighlight> {
        self.references(document_id, position, true)
            .into_iter()
            .filter(|reference| reference.document_id() == document_id)
            .map(|reference| DocumentHighlight {
                range: reference.range(),
                kind: document_highlight_kind(reference.kind()),
            })
            .collect()
    }

    #[must_use]
    pub fn references(
        &self,
        document_id: &DocumentId,
        position: Position,
        include_declaration: bool,
    ) -> Vec<Reference> {
        let Some(query) = QueryContext::from_databases(self, document_id, position) else {
            return Vec::new();
        };
        let Some(source) = query.source_record() else {
            return Vec::new();
        };
        let Some(range) = query.identifier_range() else {
            return Vec::new();
        };
        let token = ReferenceToken { range };
        let source_id = source.source_id();
        let Ok(offset) = u32::try_from(token.range.start) else {
            return Vec::new();
        };
        let graph = self.hir_db().graph();

        if let Some(target) = trait_declaration_target(graph, source_id, source.text(), &token) {
            return self.trait_references(&target, include_declaration);
        }
        if let Some(target) =
            fields::script_field_declaration_target(graph, source_id, source.text(), &token)
        {
            return fields::script_field_references(self, &target, include_declaration);
        }
        if let Some(target) =
            enum_variant_declaration_target(graph, source_id, source.text(), &token)
        {
            return self.enum_variant_references(&target, include_declaration);
        }
        if let Some(target) = variant_fields::script_variant_field_declaration_target(
            graph,
            source_id,
            source.text(),
            &token,
        ) {
            return variant_fields::script_variant_field_references(
                self,
                &target,
                include_declaration,
            );
        }
        if let Some(target) = schema::schema_variant_declaration_target(self, source_id, &token) {
            return schema::schema_variant_references(self, &target, include_declaration);
        }
        if let Some(target) =
            methods::script_method_declaration_target(graph, source_id, source.text(), &token)
        {
            return methods::script_method_references(self, &target, include_declaration);
        }
        if let Some(target) = schema::schema_method_declaration_target(self, source_id, &token) {
            return schema::schema_method_references(self, &target, include_declaration);
        }
        if let Some(target) = schema::schema_field_declaration_target(self, source_id, &token) {
            return schema::schema_field_references(self, &target, include_declaration);
        }
        if let Some(target) = modules::import_module_target(graph, source_id, source.text(), &token)
        {
            return modules::import_module_references(self, &target);
        }

        for declaration in graph.declarations() {
            if declaration.span.source != source_id || !declaration.span.contains(offset) {
                continue;
            }
            let Some(bindings) = graph.bindings(declaration.id) else {
                continue;
            };
            if let Some(local) = local_reference_target(source.text(), bindings, &token) {
                return self.local_references(bindings, local, include_declaration);
            }
            if let Some(target) = enum_variant_use_target(graph, bindings, source.text(), &token) {
                return self.enum_variant_references(&target.target, include_declaration);
            }
            if let Some(target) = schema::schema_variant_use_target(self, source.text(), &token) {
                return schema::schema_variant_references(self, &target, include_declaration);
            }
            if let Some(parsed) = self.parse_db().parsed_source(document_id)
                && let Some(target) =
                    schema::schema_record_field_use_target(self, parsed, source.text(), &token)
            {
                return schema::schema_field_references(self, &target, include_declaration);
            }
            if let Some(parsed) = self.parse_db().parsed_source(document_id)
                && let Some(target) =
                    fields::script_record_field_use_target(graph, parsed, source.text(), &token)
            {
                return fields::script_field_references(self, &target, include_declaration);
            }
            if let Some(parsed) = self.parse_db().parsed_source(document_id)
                && let Some(target) = variant_fields::script_variant_field_use_target(
                    graph,
                    parsed,
                    source.text(),
                    &token,
                )
            {
                return variant_fields::script_variant_field_references(
                    self,
                    &target,
                    include_declaration,
                );
            }
            if let Some(declaration) = declaration_reference_target(bindings, &token) {
                return self.declaration_references(declaration, include_declaration);
            }
            let member_receiver = query
                .member_receiver_range()
                .or_else(|| query.call_member_receiver_range())
                .and_then(|receiver| query.type_fact_for_range(self, receiver));
            if let Some(target) = member_receiver.as_ref().and_then(|receiver| {
                token_text(source.text(), token.range).and_then(|field| {
                    fields::script_field_target_for_receiver_fact(graph, receiver, field)
                })
            }) {
                return fields::script_field_references(self, &target, include_declaration);
            }
            if let Some(target) = member_receiver.as_ref().and_then(|receiver| {
                token_text(source.text(), token.range)
                    .filter(|_| is_call_callee(source.text(), token.range))
                    .and_then(|method| {
                        methods::script_method_target_for_receiver_fact(graph, receiver, method)
                    })
            }) {
                return methods::script_method_references(self, &target, include_declaration);
            }
            if let Some(target) = member_receiver.as_ref().and_then(|receiver| {
                token_text(source.text(), token.range)
                    .filter(|_| is_call_callee(source.text(), token.range))
                    .and_then(|method| {
                        schema::schema_method_target_for_receiver_fact(
                            self.schema_db().facts(),
                            receiver,
                            method,
                        )
                    })
            }) {
                return schema::schema_method_references(self, &target, include_declaration);
            }
            if let Some(target) = member_receiver.as_ref().and_then(|receiver| {
                token_text(source.text(), token.range).and_then(|field| {
                    schema::schema_field_target_for_receiver_fact(
                        self.schema_db().facts(),
                        receiver,
                        field,
                    )
                })
            }) {
                return schema::schema_field_references(self, &target, include_declaration);
            }
        }

        if let Some(target) = trait_impl_use_target(graph, source_id, source.text(), &token) {
            return self.trait_references(&target, include_declaration);
        }

        if let Some(declaration) = graph.declarations().find(|declaration| {
            declaration.span.source == source_id
                && declaration.span.contains(offset)
                && token_text(source.text(), token.range) == Some(declaration.name.as_str())
        }) {
            return self.declaration_references(declaration.id, include_declaration);
        }

        Vec::new()
    }

    fn local_references(
        &self,
        bindings: &BindingMap,
        local: HirLocalId,
        include_declaration: bool,
    ) -> Vec<Reference> {
        let mut references = Vec::new();

        if include_declaration
            && let Some(binding) = bindings.local(local)
            && let Some(reference) = self.reference_for_local_binding(binding)
        {
            references.push(reference);
        }

        references.extend(
            bindings
                .resolutions()
                .filter_map(|(expression, resolution)| match resolution {
                    BindingResolution::Local(resolved) if *resolved == local => {
                        let expression = bindings.expression(expression)?;
                        self.reference_for_resolved_use_span(expression.span)
                    }
                    BindingResolution::Local(_)
                    | BindingResolution::Declaration(_)
                    | BindingResolution::Import(_)
                    | BindingResolution::QualifiedPath(_) => None,
                }),
        );

        references.sort_by_key(|reference| {
            let start = reference.range.start();
            (
                reference.document_id.as_str().to_owned(),
                start.line,
                start.character,
            )
        });
        references
    }

    fn declaration_references(
        &self,
        declaration: HirDeclId,
        include_declaration: bool,
    ) -> Vec<Reference> {
        let graph = self.hir_db().graph();
        let mut references = Vec::new();

        if include_declaration
            && let Some(declaration) = graph.declaration(declaration)
            && let Some(reference) =
                self.reference_for_declaration(declaration, ReferenceKind::Declaration)
        {
            references.push(reference);
        }

        for module in graph.module_ids() {
            if let Some(imports) = graph.imports(module) {
                references.extend(imports.iter().filter_map(|import| {
                    match import.resolution {
                        Some(ImportResolution::Declaration(resolved))
                            if resolved == declaration =>
                        {
                            self.reference_for_import(
                                import.span,
                                import
                                    .alias
                                    .as_deref()
                                    .or_else(|| import.path.last().map(String::as_str)),
                            )
                        }
                        Some(ImportResolution::Declaration(_)) | None => None,
                    }
                }));
            }
        }

        for owner in graph.declarations() {
            let Some(bindings) = graph.bindings(owner.id) else {
                continue;
            };
            references.extend(
                bindings
                    .resolutions()
                    .filter_map(|(expression, resolution)| match resolution {
                        BindingResolution::Declaration(resolved) if *resolved == declaration => {
                            let expression = bindings.expression(expression)?;
                            self.reference_for_resolved_use_span(expression.span)
                        }
                        BindingResolution::Declaration(_)
                        | BindingResolution::Local(_)
                        | BindingResolution::Import(_)
                        | BindingResolution::QualifiedPath(_) => None,
                    }),
            );
        }

        references.sort_by_key(|reference| {
            let start = reference.range.start();
            (
                reference.document_id.as_str().to_owned(),
                start.line,
                start.character,
                reference.kind,
            )
        });
        references
    }

    fn trait_references(
        &self,
        target: &TraitReferenceTarget,
        include_declaration: bool,
    ) -> Vec<Reference> {
        let graph = self.hir_db().graph();
        let mut references = Vec::new();

        if include_declaration
            && let Some(declaration) = graph.declaration(target.owner)
            && let Some(reference) =
                self.reference_for_declaration(declaration, ReferenceKind::Declaration)
        {
            references.push(reference);
        }

        for source in self.source_db().records().values() {
            references.extend(trait_impl_use_references_for_source(graph, source, target));
        }

        references.sort_by_key(|reference| {
            let start = reference.range.start();
            (
                reference.document_id.as_str().to_owned(),
                start.line,
                start.character,
                reference.kind,
            )
        });
        references
    }

    fn enum_variant_references(
        &self,
        target: &EnumVariantReferenceTarget,
        include_declaration: bool,
    ) -> Vec<Reference> {
        let graph = self.hir_db().graph();
        let mut references = Vec::new();

        if include_declaration
            && let Some(reference) = self.reference_for_enum_variant_declaration(target)
        {
            references.push(reference);
        }

        for source in self.source_db().records().values() {
            references.extend(enum_variant_use_references_for_source(
                self, graph, source, target,
            ));
        }

        references.sort_by_key(|reference| {
            let start = reference.range.start();
            (
                reference.document_id.as_str().to_owned(),
                start.line,
                start.character,
                reference.kind,
            )
        });
        references
    }

    fn reference_for_declaration(
        &self,
        declaration: &Declaration,
        kind: ReferenceKind,
    ) -> Option<Reference> {
        let source = self.source_record_for_reference(declaration.span.source)?;
        let span_range = span_text_range(declaration.span)?;
        let name_range =
            name_range_in_text(source.text(), span_range, &declaration.name).unwrap_or(span_range);
        let range = diagnostic_range(source.text(), name_range);
        Some(Reference {
            document_id: source.document_id().clone(),
            range,
            kind,
        })
    }

    fn reference_for_local_binding(&self, binding: &LocalBinding) -> Option<Reference> {
        let source = self.source_record_for_reference(binding.span.source)?;
        let span_range = span_text_range(binding.span)?;
        let name_range =
            name_range_in_text(source.text(), span_range, &binding.name).unwrap_or(span_range);
        Some(Reference {
            document_id: source.document_id().clone(),
            range: diagnostic_range(source.text(), name_range),
            kind: ReferenceKind::Declaration,
        })
    }

    fn reference_for_import(&self, span: Span, name: Option<&str>) -> Option<Reference> {
        let source = self.source_record_for_reference(span.source)?;
        let span_range = span_text_range(span)?;
        let range = name
            .and_then(|name| name_range_in_text(source.text(), span_range, name))
            .unwrap_or(span_range);
        Some(Reference {
            document_id: source.document_id().clone(),
            range: diagnostic_range(source.text(), range),
            kind: ReferenceKind::Import,
        })
    }

    fn reference_for_enum_variant_declaration(
        &self,
        target: &EnumVariantReferenceTarget,
    ) -> Option<Reference> {
        let graph = self.hir_db().graph();
        let variant = graph
            .enum_shape(target.owner)?
            .variants
            .iter()
            .find(|variant| variant.name == target.variant)?;
        let source = self.source_record_for_reference(variant.span.source)?;
        let span_range = span_text_range(variant.span)?;
        let name_range =
            name_range_in_text(source.text(), span_range, &variant.name).unwrap_or(span_range);
        Some(Reference {
            document_id: source.document_id().clone(),
            range: diagnostic_range(source.text(), name_range),
            kind: ReferenceKind::Declaration,
        })
    }

    fn reference_for_resolved_use_span(&self, span: Span) -> Option<Reference> {
        let source = self.source_record_for_reference(span.source)?;
        let range = span_text_range(span)?;
        let kind = resolved_use_reference_kind(source.text(), range);
        Some(Reference {
            document_id: source.document_id().clone(),
            range: diagnostic_range(source.text(), range),
            kind,
        })
    }

    fn source_record_for_reference(&self, source_id: SourceId) -> Option<&crate::SourceRecord> {
        self.source_db()
            .records()
            .values()
            .find(|record| record.source_id() == source_id)
    }
}

fn trait_declaration_target(
    graph: &ModuleGraph,
    source_id: SourceId,
    text: &str,
    token: &ReferenceToken,
) -> Option<TraitReferenceTarget> {
    let start = u32::try_from(token.range.start).ok()?;
    graph
        .declarations()
        .find(|declaration| {
            declaration.kind == DeclarationKind::Trait
                && declaration.span.source == source_id
                && declaration.span.contains(start)
                && token_text(text, token.range) == Some(declaration.name.as_str())
        })
        .map(|declaration| TraitReferenceTarget {
            owner: declaration.id,
        })
}

fn enum_variant_declaration_target(
    graph: &ModuleGraph,
    source_id: SourceId,
    text: &str,
    token: &ReferenceToken,
) -> Option<EnumVariantReferenceTarget> {
    let start = u32::try_from(token.range.start).ok()?;
    for declaration in graph.declarations() {
        if declaration.kind != DeclarationKind::Enum
            || declaration.span.source != source_id
            || !declaration.span.contains(start)
        {
            continue;
        }
        let shape = graph.enum_shape(declaration.id)?;
        for variant in &shape.variants {
            let span_range = span_text_range(variant.span)?;
            let name_range = name_range_in_text(text, span_range, &variant.name)?;
            if name_range.start <= token.range.start && token.range.end <= name_range.end {
                return Some(EnumVariantReferenceTarget {
                    owner: declaration.id,
                    variant: variant.name.clone(),
                });
            }
        }
    }
    None
}

fn trait_impl_use_target(
    graph: &ModuleGraph,
    source_id: SourceId,
    text: &str,
    token: &ReferenceToken,
) -> Option<TraitReferenceTarget> {
    graph.declarations().find_map(|declaration| {
        let metadata = graph.impl_metadata(declaration.id)?;
        let ImplMetadataKind::Trait { trait_path } = &metadata.kind else {
            return None;
        };
        if declaration.span.source != source_id {
            return None;
        }
        let span_range = span_text_range(declaration.span)?;
        let name_range = trait_path_name_range_in_text(text, span_range, trait_path)?;
        if !(name_range.start <= token.range.start && token.range.end <= name_range.end) {
            return None;
        }
        trait_declaration_for_path(graph, trait_path).map(|owner| TraitReferenceTarget { owner })
    })
}

fn trait_impl_use_references_for_source(
    graph: &ModuleGraph,
    source: &crate::SourceRecord,
    target: &TraitReferenceTarget,
) -> Vec<Reference> {
    let mut references = Vec::new();
    let source_id = source.source_id();
    let text = source.text();
    for declaration in graph.declarations() {
        let Some(metadata) = graph.impl_metadata(declaration.id) else {
            continue;
        };
        let ImplMetadataKind::Trait { trait_path } = &metadata.kind else {
            continue;
        };
        if declaration.span.source != source_id
            || trait_declaration_for_path(graph, trait_path) != Some(target.owner)
        {
            continue;
        }
        let Some(span_range) = span_text_range(declaration.span) else {
            continue;
        };
        let Some(name_range) = trait_path_name_range_in_text(text, span_range, trait_path) else {
            continue;
        };
        references.push(Reference {
            document_id: source.document_id().clone(),
            range: diagnostic_range(text, name_range),
            kind: ReferenceKind::Read,
        });
    }
    references
}

fn trait_declaration_for_path(graph: &ModuleGraph, trait_path: &[String]) -> Option<HirDeclId> {
    graph.declarations().find_map(|declaration| {
        (declaration.kind == DeclarationKind::Trait
            && declaration_path_matches(graph, declaration, trait_path))
        .then_some(declaration.id)
    })
}

fn declaration_path_matches(
    graph: &ModuleGraph,
    declaration: &Declaration,
    path: &[String],
) -> bool {
    if path.len() == 1 {
        return path.first().is_some_and(|name| name == &declaration.name);
    }
    qualified_declaration_name(graph, declaration) == path.join("::")
}

fn trait_path_name_range_in_text(
    text: &str,
    range: TextRange,
    trait_path: &[String],
) -> Option<TextRange> {
    let name = trait_path.last()?;
    let full_path = trait_path.join("::");
    if !full_path.is_empty()
        && let Some(full_range) = path_range_in_text(text, range, &full_path)
    {
        return Some(TextRange::new(full_range.end - name.len(), full_range.end));
    }
    name_range_in_text(text, range, name)
}

fn path_range_in_text(text: &str, range: TextRange, path: &str) -> Option<TextRange> {
    let slice = text.get(range.start..range.end)?;
    slice.match_indices(path).find_map(|(offset, matched)| {
        let start = range.start + offset;
        let end = start + matched.len();
        is_identifier_boundary(text, start, end).then(|| TextRange::new(start, end))
    })
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct EnumVariantUseTarget {
    target: EnumVariantReferenceTarget,
    kind: ReferenceKind,
}

fn enum_variant_use_target(
    graph: &ModuleGraph,
    bindings: &BindingMap,
    text: &str,
    token: &ReferenceToken,
) -> Option<EnumVariantUseTarget> {
    let path = path_ending_at(text, token.range)?;
    let variant = path.last()?;
    if let Some(BindingResolution::Declaration(owner)) = bindings.pattern_resolution(&path)
        && enum_variant_exists(graph, *owner, variant)
    {
        return Some(EnumVariantUseTarget {
            target: EnumVariantReferenceTarget {
                owner: *owner,
                variant: variant.clone(),
            },
            kind: ReferenceKind::Pattern,
        });
    }

    match narrowest_resolution_at_token(bindings, token)? {
        BindingResolution::Declaration(owner) if enum_variant_exists(graph, *owner, variant) => {
            Some(EnumVariantUseTarget {
                target: EnumVariantReferenceTarget {
                    owner: *owner,
                    variant: variant.clone(),
                },
                kind: resolved_use_reference_kind(text, token.range),
            })
        }
        BindingResolution::Declaration(_)
        | BindingResolution::Local(_)
        | BindingResolution::Import(_)
        | BindingResolution::QualifiedPath(_) => None,
    }
}

fn enum_variant_use_references_for_source(
    databases: &LanguageServiceDatabases,
    graph: &ModuleGraph,
    source: &crate::SourceRecord,
    target: &EnumVariantReferenceTarget,
) -> Vec<Reference> {
    let mut references = Vec::new();
    let source_id = source.source_id();
    let text = source.text();
    let mut shared_ranges = HashSet::new();
    if let Some(parsed) = databases.parse_db().parsed_source(source.document_id()) {
        for site in path_calls::path_expression_sites(parsed, text) {
            if site
                .path
                .last()
                .is_none_or(|segment| segment != &target.variant)
            {
                continue;
            }
            let range = site.segment_range;
            shared_ranges.insert((range.start, range.end));
            push_enum_variant_use_reference_for_range(
                graph,
                source,
                target,
                range,
                &mut references,
            );
        }
    }
    for range in path_segment_ranges(source_id, text, &target.variant) {
        if shared_ranges.contains(&(range.start, range.end)) {
            continue;
        }
        push_enum_variant_use_reference_for_range(graph, source, target, range, &mut references);
    }
    references
}

fn push_enum_variant_use_reference_for_range(
    graph: &ModuleGraph,
    source: &crate::SourceRecord,
    target: &EnumVariantReferenceTarget,
    range: TextRange,
    references: &mut Vec<Reference>,
) {
    let source_id = source.source_id();
    let text = source.text();
    let Some(start) = u32::try_from(range.start).ok() else {
        return;
    };
    for declaration in graph.declarations() {
        if declaration.span.source != source_id || !declaration.span.contains(start) {
            continue;
        }
        let Some(bindings) = graph.bindings(declaration.id) else {
            continue;
        };
        let Some(found) = enum_variant_use_target(graph, bindings, text, &ReferenceToken { range })
        else {
            continue;
        };
        if found.target == *target {
            references.push(Reference {
                document_id: source.document_id().clone(),
                range: diagnostic_range(text, range),
                kind: found.kind,
            });
            break;
        }
    }
}

fn enum_variant_exists(graph: &ModuleGraph, owner: HirDeclId, variant: &str) -> bool {
    graph
        .enum_shape(owner)
        .is_some_and(|shape| shape.variants.iter().any(|entry| entry.name == variant))
}

fn path_segment_ranges(source_id: SourceId, text: &str, name: &str) -> Vec<TextRange> {
    lex(source_id, text)
        .tokens
        .into_iter()
        .filter_map(|token| match token.kind {
            TokenKind::Ident(token_name) if token_name == name => {
                let range = span_text_range(token.span)?;
                path_ending_at(text, range).map(|_| range)
            }
            TokenKind::Ident(_)
            | TokenKind::Int(_)
            | TokenKind::Float(_)
            | TokenKind::Char(_)
            | TokenKind::String(_)
            | TokenKind::InterpolatedString(_)
            | TokenKind::Bytes(_)
            | TokenKind::Keyword(_)
            | TokenKind::Symbol(_)
            | TokenKind::Eof => None,
        })
        .collect()
}

fn path_ending_at(text: &str, range: TextRange) -> Option<Vec<String>> {
    let mut path = vec![token_text(text, range)?.to_owned()];
    let mut cursor = range.start;
    loop {
        let before_segment = text.get(..cursor)?.trim_end();
        let Some(before_separator) = before_segment.strip_suffix("::").map(str::trim_end) else {
            break;
        };
        let end = before_separator.len();
        let start = before_separator
            .char_indices()
            .rev()
            .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
            .unwrap_or(0);
        if start == end {
            break;
        }
        path.push(text.get(start..end)?.to_owned());
        cursor = start;
    }
    (path.len() > 1).then(|| {
        path.reverse();
        path
    })
}

fn record_owner_names(receiver: &TypeFact) -> Vec<String> {
    let mut owners = Vec::new();
    collect_record_owner_names(receiver, &mut owners);
    owners
}

fn collect_record_owner_names(receiver: &TypeFact, owners: &mut Vec<String>) {
    match receiver {
        TypeFact::Record { name } => {
            push_owner_name(owners, name);
            if let Some(short) = name.rsplit("::").next()
                && short != name
            {
                push_owner_name(owners, short);
            }
        }
        TypeFact::Union(facts) => {
            for fact in facts {
                collect_record_owner_names(fact, owners);
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

fn push_owner_name(owners: &mut Vec<String>, name: &str) {
    if !owners.iter().any(|owner| owner == name) {
        owners.push(name.to_owned());
    }
}

fn declaration_name_matches(graph: &ModuleGraph, declaration: &Declaration, owner: &str) -> bool {
    declaration.name == owner || qualified_declaration_name(graph, declaration) == owner
}

fn qualified_declaration_name(graph: &ModuleGraph, declaration: &Declaration) -> String {
    graph
        .module_path(declaration.module)
        .map(|path| {
            path.segments()
                .iter()
                .chain(std::iter::once(&declaration.name))
                .cloned()
                .collect::<Vec<_>>()
                .join("::")
        })
        .unwrap_or_else(|| declaration.name.clone())
}

fn declaration_reference_target(
    bindings: &BindingMap,
    token: &ReferenceToken,
) -> Option<HirDeclId> {
    let resolution = narrowest_resolution_at_token(bindings, token)?;
    match resolution {
        BindingResolution::Declaration(declaration) => Some(*declaration),
        BindingResolution::Local(_)
        | BindingResolution::Import(_)
        | BindingResolution::QualifiedPath(_) => None,
    }
}

fn local_reference_target(
    text: &str,
    bindings: &BindingMap,
    token: &ReferenceToken,
) -> Option<HirLocalId> {
    if let Some(binding) = local_declaration_at_token(text, bindings, token) {
        return Some(binding.id);
    }

    let resolution = narrowest_resolution_at_token(bindings, token)?;
    match resolution {
        BindingResolution::Local(local) => Some(*local),
        BindingResolution::Declaration(_)
        | BindingResolution::Import(_)
        | BindingResolution::QualifiedPath(_) => None,
    }
}

fn narrowest_resolution_at_token<'a>(
    bindings: &'a BindingMap,
    token: &ReferenceToken,
) -> Option<&'a BindingResolution> {
    bindings
        .resolutions()
        .filter_map(|(expression, resolution)| {
            let expression = bindings.expression(expression)?;
            let start = usize::try_from(expression.span.start).ok()?;
            let end = usize::try_from(expression.span.end).ok()?;
            (start <= token.range.start && token.range.end <= end)
                .then_some((end.saturating_sub(start), resolution))
        })
        .min_by_key(|(len, _)| *len)
        .map(|(_, resolution)| resolution)
}

fn local_declaration_at_token<'a>(
    text: &str,
    bindings: &'a BindingMap,
    token: &ReferenceToken,
) -> Option<&'a LocalBinding> {
    bindings.locals().find(|binding| {
        let Some(range) = span_text_range(binding.span)
            .and_then(|range| name_range_in_text(text, range, &binding.name))
        else {
            return false;
        };
        let start = range.start;
        let end = range.end;
        start <= token.range.start && token.range.end <= end
    })
}

fn diagnostic_range(text: &str, range: TextRange) -> DiagnosticRange {
    let line_index = LineIndex::new(text);
    DiagnosticRange::new(
        line_index.position(range.start),
        line_index.position(range.end),
    )
}

fn span_text_range(span: Span) -> Option<TextRange> {
    let start = usize::try_from(span.start).ok()?;
    let end = usize::try_from(span.end).ok()?;
    Some(TextRange::new(start, end))
}

fn name_range_in_text(text: &str, range: TextRange, name: &str) -> Option<TextRange> {
    let slice = text.get(range.start..range.end)?;
    slice.match_indices(name).find_map(|(offset, matched)| {
        let start = range.start + offset;
        let end = start + matched.len();
        is_identifier_boundary(text, start, end).then(|| TextRange::new(start, end))
    })
}

fn is_identifier_boundary(text: &str, start: usize, end: usize) -> bool {
    let before = text[..start].chars().next_back();
    let after = text[end..].chars().next();
    before.is_none_or(|ch| !is_identifier_continue(ch))
        && after.is_none_or(|ch| !is_identifier_continue(ch))
}

fn token_text(text: &str, range: TextRange) -> Option<&str> {
    text.get(range.start..range.end)
}

const fn document_highlight_kind(kind: ReferenceKind) -> DocumentHighlightKind {
    match kind {
        ReferenceKind::Call => DocumentHighlightKind::Call,
        ReferenceKind::Read => DocumentHighlightKind::Read,
        ReferenceKind::Write => DocumentHighlightKind::Write,
        ReferenceKind::Declaration | ReferenceKind::Import | ReferenceKind::Pattern => {
            DocumentHighlightKind::Text
        }
    }
}

fn resolved_use_reference_kind(text: &str, range: TextRange) -> ReferenceKind {
    if is_call_callee(text, range) {
        ReferenceKind::Call
    } else if is_assignment_target(text, range) {
        ReferenceKind::Write
    } else {
        ReferenceKind::Read
    }
}

fn is_call_callee(text: &str, range: TextRange) -> bool {
    text.get(range.end..)
        .is_some_and(|suffix| suffix.trim_start().starts_with('('))
}

fn is_assignment_target(text: &str, range: TextRange) -> bool {
    text.get(range.end..)
        .map(str::trim_start)
        .is_some_and(|suffix| {
            suffix.starts_with("+=")
                || suffix.starts_with("-=")
                || suffix.starts_with("*=")
                || suffix.starts_with("/=")
                || suffix.starts_with("%=")
                || (suffix.starts_with('=')
                    && !suffix.starts_with("==")
                    && !suffix.starts_with("=>"))
        })
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

#[cfg(test)]
mod field_tests;
#[cfg(test)]
mod highlight_tests;
#[cfg(test)]
mod module_tests;
#[cfg(test)]
mod schema_field_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod variant_field_tests;
