use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingMap, BindingResolution, LocalBinding};
use vela_hir::ids::HirLocalId;

use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, LineIndex, Position, TextRange,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ReferenceKind {
    Declaration,
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
            if let Some(local) = local_reference_target(bindings, &token) {
                return self.local_references(bindings, local, include_declaration);
            }
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
            && let Some(reference) =
                self.reference_for_span(binding.span, ReferenceKind::Declaration)
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

    fn reference_for_span(&self, span: Span, kind: ReferenceKind) -> Option<Reference> {
        let source = self.source_record_for_reference(span.source)?;
        let start = usize::try_from(span.start).ok()?;
        let end = usize::try_from(span.end).ok()?;
        let range = diagnostic_range(source.text(), TextRange::new(start, end));
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

fn local_reference_target(bindings: &BindingMap, token: &ReferenceToken) -> Option<HirLocalId> {
    if let Some(binding) = local_declaration_at_token(bindings, token) {
        return Some(binding.id);
    }

    let resolution = bindings
        .resolutions()
        .filter_map(|(expression, resolution)| {
            let expression = bindings.expression(expression)?;
            let start = usize::try_from(expression.span.start).ok()?;
            let end = usize::try_from(expression.span.end).ok()?;
            (start <= token.range.start && token.range.end <= end)
                .then_some((end.saturating_sub(start), resolution))
        })
        .min_by_key(|(len, _)| *len)?
        .1;

    match resolution {
        BindingResolution::Local(local) => Some(*local),
        BindingResolution::Declaration(_)
        | BindingResolution::Import(_)
        | BindingResolution::QualifiedPath(_) => None,
    }
}

fn local_declaration_at_token<'a>(
    bindings: &'a BindingMap,
    token: &ReferenceToken,
) -> Option<&'a LocalBinding> {
    bindings.locals().find(|binding| {
        let Ok(start) = usize::try_from(binding.span.start) else {
            return false;
        };
        let Ok(end) = usize::try_from(binding.span.end) else {
            return false;
        };
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
