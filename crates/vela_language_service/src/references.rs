use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingMap, BindingResolution, LocalBinding};
use vela_hir::ids::{HirDeclId, HirLocalId};
use vela_hir::module_graph::{Declaration, ImportResolution};

use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, LineIndex, Position, TextRange,
};

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReferenceKind {
    Declaration,
    Import,
    Read,
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
struct ReferenceToken {
    range: TextRange,
}

impl LanguageServiceDatabases {
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
                        self.reference_for_span(expression.span, ReferenceKind::Read)
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
                            self.reference_for_span(expression.span, ReferenceKind::Read)
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

    fn reference_for_span(&self, span: Span, kind: ReferenceKind) -> Option<Reference> {
        let source = self.source_record_for_reference(span.source)?;
        let range = diagnostic_range(source.text(), span_text_range(span)?);
        Some(Reference {
            document_id: source.document_id().clone(),
            range,
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
            ReferenceKind::Read,
        );
        assert_reference_in_document(
            &references,
            &main,
            3,
            line(main_text, 3).find("grant").expect("second call"),
            ReferenceKind::Read,
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
