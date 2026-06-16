use vela_analysis::{facts::AnalysisFacts, type_fact::TypeFact};
use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingMap, BindingResolution, LocalBinding};
use vela_hir::ids::{HirDeclId, HirLocalId};
use vela_hir::module_graph::{Declaration, DeclarationKind, ImportResolution, ModuleGraph};
use vela_syntax::lexer::lex;
use vela_syntax::token::TokenKind;

use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, LineIndex, Position, TextRange,
};

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReferenceKind {
    Declaration,
    Import,
    Call,
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
struct FieldReferenceTarget {
    owner: HirDeclId,
    field: String,
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
        let Some(source) = self.source_db().records().get(document_id) else {
            return Vec::new();
        };
        let Some(token) = reference_token_at(source.text(), position) else {
            return Vec::new();
        };
        let source_id = source.source_id();
        let Ok(offset) = u32::try_from(token.range.start) else {
            return Vec::new();
        };
        let graph = self.hir_db().graph();
        let facts = AnalysisFacts::from_module_graph(graph);

        if let Some(target) =
            script_field_declaration_target(graph, source_id, source.text(), &token)
        {
            return self.script_field_references(&target, include_declaration);
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
            if let Some(declaration) = declaration_reference_target(bindings, &token) {
                return self.declaration_references(declaration, include_declaration);
            }
            if let Some(target) =
                script_field_use_target(graph, &facts, source.text(), source_id, bindings, &token)
            {
                return self.script_field_references(&target, include_declaration);
            }
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

    fn script_field_references(
        &self,
        target: &FieldReferenceTarget,
        include_declaration: bool,
    ) -> Vec<Reference> {
        let graph = self.hir_db().graph();
        let facts = AnalysisFacts::from_module_graph(graph);
        let mut references = Vec::new();

        if include_declaration
            && let Some(reference) = self.reference_for_script_field_declaration(target)
        {
            references.push(reference);
        }

        for source in self.source_db().records().values() {
            references.extend(script_field_use_references_for_source(
                graph, &facts, source, target,
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

    fn reference_for_script_field_declaration(
        &self,
        target: &FieldReferenceTarget,
    ) -> Option<Reference> {
        let graph = self.hir_db().graph();
        let field = graph
            .struct_shape(target.owner)?
            .fields
            .iter()
            .find(|field| field.name == target.field)?;
        let source = self.source_record_for_reference(field.span.source)?;
        let span_range = span_text_range(field.span)?;
        let name_range =
            name_range_in_text(source.text(), span_range, &field.name).unwrap_or(span_range);
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

fn script_field_declaration_target(
    graph: &ModuleGraph,
    source_id: SourceId,
    text: &str,
    token: &ReferenceToken,
) -> Option<FieldReferenceTarget> {
    let start = u32::try_from(token.range.start).ok()?;
    for declaration in graph.declarations() {
        if declaration.kind != DeclarationKind::Struct
            || declaration.span.source != source_id
            || !declaration.span.contains(start)
        {
            continue;
        }
        let shape = graph.struct_shape(declaration.id)?;
        for field in &shape.fields {
            let span_range = span_text_range(field.span)?;
            let name_range = name_range_in_text(text, span_range, &field.name)?;
            if name_range.start <= token.range.start && token.range.end <= name_range.end {
                return Some(FieldReferenceTarget {
                    owner: declaration.id,
                    field: field.name.clone(),
                });
            }
        }
    }
    None
}

fn script_field_use_target(
    graph: &ModuleGraph,
    facts: &AnalysisFacts,
    text: &str,
    source_id: SourceId,
    bindings: &BindingMap,
    token: &ReferenceToken,
) -> Option<FieldReferenceTarget> {
    let field = token_text(text, token.range)?;
    script_field_target_for_member(graph, facts, text, source_id, bindings, field, token.range)
}

fn script_field_use_references_for_source(
    graph: &ModuleGraph,
    facts: &AnalysisFacts,
    source: &crate::SourceRecord,
    target: &FieldReferenceTarget,
) -> Vec<Reference> {
    let mut references = Vec::new();
    let source_id = source.source_id();
    let text = source.text();
    for range in member_field_ranges(source_id, text, &target.field) {
        let Some(start) = u32::try_from(range.start).ok() else {
            continue;
        };
        for declaration in graph.declarations() {
            if declaration.span.source != source_id || !declaration.span.contains(start) {
                continue;
            }
            let Some(bindings) = graph.bindings(declaration.id) else {
                continue;
            };
            if script_field_target_for_member(
                graph,
                facts,
                text,
                source_id,
                bindings,
                &target.field,
                range,
            )
            .as_ref()
                == Some(target)
            {
                references.push(Reference {
                    document_id: source.document_id().clone(),
                    range: diagnostic_range(text, range),
                    kind: resolved_use_reference_kind(text, range),
                });
                break;
            }
        }
    }
    references
}

fn member_field_ranges(source_id: SourceId, text: &str, field: &str) -> Vec<TextRange> {
    lex(source_id, text)
        .tokens
        .into_iter()
        .filter_map(|token| match token.kind {
            TokenKind::Ident(name) if name == field => {
                let range = span_text_range(token.span)?;
                member_receiver_range(text, range.start).map(|_| range)
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

fn script_field_target_for_member(
    graph: &ModuleGraph,
    facts: &AnalysisFacts,
    text: &str,
    source_id: SourceId,
    bindings: &BindingMap,
    field: &str,
    member_range: TextRange,
) -> Option<FieldReferenceTarget> {
    let receiver = member_receiver_range(text, member_range.start)?;
    let start = u32::try_from(receiver.start).ok()?;
    let end = u32::try_from(receiver.end).ok()?;
    let span = Span::new(source_id, start, end);
    let resolution = bindings.resolution_at_span(span)?;
    let receiver = type_fact_for_resolution(resolution, facts)?;
    let owner = script_field_owner(graph, &receiver, field)?;
    Some(FieldReferenceTarget {
        owner,
        field: field.to_owned(),
    })
}

fn type_fact_for_resolution(
    resolution: &BindingResolution,
    facts: &AnalysisFacts,
) -> Option<TypeFact> {
    match resolution {
        BindingResolution::Local(local) => facts
            .local(*local)
            .cloned()
            .filter(|fact| !matches!(fact, TypeFact::Unknown)),
        BindingResolution::Declaration(declaration) => facts.declaration(*declaration).cloned(),
        BindingResolution::Import(_) | BindingResolution::QualifiedPath(_) => None,
    }
}

fn script_field_owner(graph: &ModuleGraph, receiver: &TypeFact, field: &str) -> Option<HirDeclId> {
    let owner_names = record_owner_names(receiver);
    graph.declarations().find_map(|declaration| {
        if declaration.kind != DeclarationKind::Struct {
            return None;
        }
        let matches_owner = owner_names
            .iter()
            .any(|owner| declaration_name_matches(graph, declaration, owner));
        let has_field = graph
            .struct_shape(declaration.id)
            .is_some_and(|shape| shape.fields.iter().any(|entry| entry.name == field));
        (matches_owner && has_field).then_some(declaration.id)
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

fn reference_token_at(text: &str, position: Position) -> Option<ReferenceToken> {
    let offset = LineIndex::new(text).offset(position);
    let range = identifier_range_at(text, offset)?;
    Some(ReferenceToken { range })
}

fn identifier_range_at(text: &str, offset: usize) -> Option<TextRange> {
    let offset = offset.min(text.len());
    let start = text[..offset]
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    let end = text[offset..]
        .char_indices()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(offset + index))
        .unwrap_or(text.len());
    (start < end).then(|| TextRange::new(start, end))
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
        ReferenceKind::Declaration | ReferenceKind::Import => DocumentHighlightKind::Text,
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

fn member_receiver_range(text: &str, member_start: usize) -> Option<TextRange> {
    let before_member = text.get(..member_start)?.trim_end();
    let before_dot = before_member.strip_suffix('.')?.trim_end();
    let end = before_dot.len();
    let start = before_dot
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    (start < end).then(|| TextRange::new(start, end))
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
    };

    #[test]
    fn references_find_local_binding_uses() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "\
pub fn main(amount: i64) -> i64 {
    let next = amount + 1
    return next + amount
}";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

        let references = databases.references(
            &document,
            Position::new(2, line(text, 2).find("amount").expect("amount use")),
            true,
        );

        assert_eq!(references.len(), 3);
        assert_reference(
            &references,
            0,
            line(text, 0).find("amount").expect("parameter declaration"),
            ReferenceKind::Declaration,
        );
        assert_reference(
            &references,
            1,
            line(text, 1).find("amount").expect("first read"),
            ReferenceKind::Read,
        );
        assert_reference(
            &references,
            2,
            line(text, 2).find("amount").expect("second read"),
            ReferenceKind::Read,
        );
    }

    #[test]
    fn references_can_exclude_local_declaration() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub fn main(amount: i64) -> i64 { return amount }";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

        let references = databases.references(
            &document,
            Position::new(0, text.find("amount").expect("parameter declaration")),
            false,
        );

        assert_eq!(references.len(), 1);
        assert_reference(
            &references,
            0,
            text.rfind("amount").expect("parameter read"),
            ReferenceKind::Read,
        );
    }

    #[test]
    fn references_find_imported_function_uses() {
        let main = DocumentId::from("/workspace/scripts/game/main.vela");
        let helper = DocumentId::from("/workspace/scripts/game/reward.vela");
        let main_text = "\
use game::reward::grant
pub fn main(amount: i64) -> i64 {
    let first = grant(amount)
    return grant(first)
}";
        let helper_text = "pub fn grant(amount: i64) -> i64 { return amount }";
        let databases = databases_for(vec![
            SourceFileSnapshot::new(main.clone(), main_text),
            SourceFileSnapshot::new(helper.clone(), helper_text),
        ]);

        let references = databases.references(
            &main,
            Position::new(2, line(main_text, 2).find("grant").expect("grant call")),
            true,
        );

        assert_eq!(references.len(), 4);
        assert_reference_in_document(
            &references,
            &helper,
            0,
            helper_text.find("grant").expect("function declaration"),
            ReferenceKind::Declaration,
        );
        assert_reference_in_document(
            &references,
            &main,
            0,
            line(main_text, 0).find("grant").expect("import"),
            ReferenceKind::Import,
        );
        assert_reference_in_document(
            &references,
            &main,
            2,
            line(main_text, 2).find("grant").expect("first call"),
            ReferenceKind::Call,
        );
        assert_reference_in_document(
            &references,
            &main,
            3,
            line(main_text, 3).find("grant").expect("second call"),
            ReferenceKind::Call,
        );
    }

    #[test]
    fn references_find_field_reads_and_writes() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "\
pub struct Reward {
    amount: i64
}

pub fn main(reward: Reward) -> i64 {
    let first = reward.amount
    reward.amount += 1
    return reward.amount + first
}";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

        let references = databases.references(
            &document,
            Position::new(5, line(text, 5).find("amount").expect("first field read")),
            true,
        );

        assert_eq!(references.len(), 4);
        assert_reference(
            &references,
            1,
            line(text, 1).find("amount").expect("field declaration"),
            ReferenceKind::Declaration,
        );
        assert_reference(
            &references,
            5,
            line(text, 5).find("amount").expect("first field read"),
            ReferenceKind::Read,
        );
        assert_reference(
            &references,
            6,
            line(text, 6).find("amount").expect("field write"),
            ReferenceKind::Write,
        );
        assert_reference(
            &references,
            7,
            line(text, 7).find("amount").expect("second field read"),
            ReferenceKind::Read,
        );
    }

    #[test]
    fn document_highlight_marks_local_declaration_and_reads() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "\
pub fn main(amount: i64) -> i64 {
    let next = amount + 1
    return next + amount
}";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

        let highlights = databases.document_highlights(
            &document,
            Position::new(2, line(text, 2).find("amount").expect("amount use")),
        );

        assert_eq!(highlights.len(), 3);
        assert_highlight(
            &highlights,
            0,
            line(text, 0).find("amount").expect("parameter declaration"),
            DocumentHighlightKind::Text,
        );
        assert_highlight(
            &highlights,
            1,
            line(text, 1).find("amount").expect("first read"),
            DocumentHighlightKind::Read,
        );
        assert_highlight(
            &highlights,
            2,
            line(text, 2).find("amount").expect("second read"),
            DocumentHighlightKind::Read,
        );
    }

    #[test]
    fn document_highlight_marks_import_and_calls_in_active_document() {
        let main = DocumentId::from("/workspace/scripts/game/main.vela");
        let helper = DocumentId::from("/workspace/scripts/game/reward.vela");
        let main_text = "\
use game::reward::grant
pub fn main(amount: i64) -> i64 {
    let first = grant(amount)
    return grant(first)
}";
        let helper_text = "pub fn grant(amount: i64) -> i64 { return amount }";
        let databases = databases_for(vec![
            SourceFileSnapshot::new(main.clone(), main_text),
            SourceFileSnapshot::new(helper, helper_text),
        ]);

        let highlights = databases.document_highlights(
            &main,
            Position::new(2, line(main_text, 2).find("grant").expect("grant call")),
        );

        assert_eq!(highlights.len(), 3);
        assert_highlight(
            &highlights,
            0,
            line(main_text, 0).find("grant").expect("import"),
            DocumentHighlightKind::Text,
        );
        assert_highlight(
            &highlights,
            2,
            line(main_text, 2).find("grant").expect("first call"),
            DocumentHighlightKind::Call,
        );
        assert_highlight(
            &highlights,
            3,
            line(main_text, 3).find("grant").expect("second call"),
            DocumentHighlightKind::Call,
        );
    }

    #[test]
    fn document_highlight_marks_read_write_call() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "\
pub fn grant(amount: i64) -> i64 { return amount }
pub fn main(amount: i64) -> i64 {
    let score = amount
    score += grant(amount)
    return score + grant(score)
}";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

        let score_highlights = databases.document_highlights(
            &document,
            Position::new(3, line(text, 3).find("score").expect("score write")),
        );

        assert_eq!(score_highlights.len(), 4);
        assert_highlight(
            &score_highlights,
            2,
            line(text, 2).find("score").expect("score declaration"),
            DocumentHighlightKind::Text,
        );
        assert_highlight(
            &score_highlights,
            3,
            line(text, 3).find("score").expect("score write"),
            DocumentHighlightKind::Write,
        );
        assert_highlight(
            &score_highlights,
            4,
            line(text, 4).find("score").expect("score read"),
            DocumentHighlightKind::Read,
        );
        assert_highlight(
            &score_highlights,
            4,
            line(text, 4).rfind("score").expect("score argument read"),
            DocumentHighlightKind::Read,
        );

        let grant_highlights = databases.document_highlights(
            &document,
            Position::new(3, line(text, 3).find("grant").expect("grant call")),
        );

        assert_eq!(grant_highlights.len(), 3);
        assert_highlight(
            &grant_highlights,
            0,
            line(text, 0).find("grant").expect("grant declaration"),
            DocumentHighlightKind::Text,
        );
        assert_highlight(
            &grant_highlights,
            3,
            line(text, 3).find("grant").expect("first grant call"),
            DocumentHighlightKind::Call,
        );
        assert_highlight(
            &grant_highlights,
            4,
            line(text, 4).find("grant").expect("second grant call"),
            DocumentHighlightKind::Call,
        );
    }

    fn assert_reference(
        references: &[Reference],
        line: usize,
        character: usize,
        kind: ReferenceKind,
    ) {
        assert!(
            references.iter().any(|reference| {
                reference.range().start().line == line
                    && reference.range().start().character == character
                    && reference.kind() == kind
            }),
            "{references:?}"
        );
    }

    fn assert_reference_in_document(
        references: &[Reference],
        document_id: &DocumentId,
        line: usize,
        character: usize,
        kind: ReferenceKind,
    ) {
        assert!(
            references.iter().any(|reference| {
                reference.document_id() == document_id
                    && reference.range().start().line == line
                    && reference.range().start().character == character
                    && reference.kind() == kind
            }),
            "{references:?}"
        );
    }

    fn assert_highlight(
        highlights: &[DocumentHighlight],
        line: usize,
        character: usize,
        kind: DocumentHighlightKind,
    ) {
        assert!(
            highlights.iter().any(|highlight| {
                highlight.range().start().line == line
                    && highlight.range().start().character == character
                    && highlight.kind() == kind
            }),
            "{highlights:?}"
        );
    }

    fn line(text: &str, line: usize) -> &str {
        text.lines().nth(line).expect("line should exist")
    }

    fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        databases
    }
}
