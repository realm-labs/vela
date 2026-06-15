use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingMap, BindingResolution, LocalBinding};
use vela_hir::ids::HirLocalId;
use vela_hir::module_graph::ModuleGraph;
use vela_syntax::token::Keyword;

use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, LineIndex, Position, TextRange,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PrepareRename {
    document_id: DocumentId,
    range: DiagnosticRange,
    placeholder: String,
}

impl PrepareRename {
    #[must_use]
    pub fn document_id(&self) -> &DocumentId {
        &self.document_id
    }

    #[must_use]
    pub const fn range(&self) -> DiagnosticRange {
        self.range
    }

    #[must_use]
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WorkspaceEdit {
    document_edits: Vec<DocumentTextEdit>,
}

impl WorkspaceEdit {
    #[must_use]
    pub fn document_edits(&self) -> &[DocumentTextEdit] {
        &self.document_edits
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DocumentTextEdit {
    document_id: DocumentId,
    edits: Vec<TextEdit>,
}

impl DocumentTextEdit {
    #[must_use]
    pub fn document_id(&self) -> &DocumentId {
        &self.document_id
    }

    #[must_use]
    pub fn edits(&self) -> &[TextEdit] {
        &self.edits
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TextEdit {
    range: DiagnosticRange,
    new_text: String,
}

impl TextEdit {
    #[must_use]
    pub const fn range(&self) -> DiagnosticRange {
        self.range
    }

    #[must_use]
    pub fn new_text(&self) -> &str {
        &self.new_text
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct RenameToken {
    range: TextRange,
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn prepare_rename(
        &self,
        document_id: &DocumentId,
        position: Position,
    ) -> Option<PrepareRename> {
        let source = self.source_db().records().get(document_id)?;
        let target = local_rename_target(
            self.hir_db().graph(),
            source.source_id(),
            source.text(),
            position,
        )?;
        let token_range = diagnostic_range(source.text(), target.token.range);
        Some(PrepareRename {
            document_id: document_id.clone(),
            range: token_range,
            placeholder: target.placeholder,
        })
    }

    #[must_use]
    pub fn rename(
        &self,
        document_id: &DocumentId,
        position: Position,
        new_name: &str,
    ) -> Option<WorkspaceEdit> {
        if !is_valid_rename_identifier(new_name) {
            return None;
        }
        let source = self.source_db().records().get(document_id)?;
        let target = local_rename_target(
            self.hir_db().graph(),
            source.source_id(),
            source.text(),
            position,
        )?;
        if local_name_conflicts(target.bindings, target.local, new_name) {
            return None;
        }

        let mut edits = Vec::new();
        if let Some(binding) = target.bindings.local(target.local)
            && let Some(range) = local_binding_name_range(source.text(), binding)
        {
            edits.push(TextEdit {
                range: diagnostic_range(source.text(), range),
                new_text: new_name.to_owned(),
            });
        }
        edits.extend(
            target
                .bindings
                .resolutions()
                .filter_map(|(expression, resolution)| match resolution {
                    BindingResolution::Local(local) if *local == target.local => {
                        let expression = target.bindings.expression(expression)?;
                        Some(TextEdit {
                            range: diagnostic_range(
                                source.text(),
                                span_text_range(expression.span)?,
                            ),
                            new_text: new_name.to_owned(),
                        })
                    }
                    BindingResolution::Local(_)
                    | BindingResolution::Declaration(_)
                    | BindingResolution::Import(_)
                    | BindingResolution::QualifiedPath(_) => None,
                }),
        );

        edits.sort_by_key(|edit| {
            let start = edit.range.start();
            (start.line, start.character)
        });

        Some(WorkspaceEdit {
            document_edits: vec![DocumentTextEdit {
                document_id: document_id.clone(),
                edits,
            }],
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct LocalRenameTarget<'a> {
    bindings: &'a BindingMap,
    local: HirLocalId,
    token: RenameToken,
    placeholder: String,
}

fn local_rename_target<'a>(
    graph: &'a ModuleGraph,
    source_id: SourceId,
    text: &str,
    position: Position,
) -> Option<LocalRenameTarget<'a>> {
    let token = rename_token_at(text, position)?;
    let offset = u32::try_from(token.range.start).ok()?;

    for declaration in graph.declarations() {
        if declaration.span.source != source_id || !declaration.span.contains(offset) {
            continue;
        }
        let Some(bindings) = graph.bindings(declaration.id) else {
            continue;
        };
        if let Some(binding) = local_declaration_at_token(text, bindings, &token) {
            return Some(LocalRenameTarget {
                bindings,
                local: binding.id,
                token,
                placeholder: binding.name.clone(),
            });
        }
        if let Some(local) = local_use_at_token(bindings, &token)
            && let Some(binding) = bindings.local(local)
        {
            return Some(LocalRenameTarget {
                bindings,
                local,
                token,
                placeholder: binding.name.clone(),
            });
        }
    }

    None
}

fn local_use_at_token(bindings: &BindingMap, token: &RenameToken) -> Option<HirLocalId> {
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
    token: &RenameToken,
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
    token: &RenameToken,
) -> Option<&'a LocalBinding> {
    bindings.locals().find(|binding| {
        let Some(range) = local_binding_name_range(text, binding) else {
            return false;
        };
        range.start <= token.range.start && token.range.end <= range.end
    })
}

fn local_binding_name_range(text: &str, binding: &LocalBinding) -> Option<TextRange> {
    span_text_range(binding.span).and_then(|range| name_range_in_text(text, range, &binding.name))
}

fn local_name_conflicts(bindings: &BindingMap, local: HirLocalId, new_name: &str) -> bool {
    bindings
        .locals()
        .any(|binding| binding.id != local && binding.name == new_name)
}

fn is_valid_rename_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(is_identifier_continue)
        && Keyword::from_text(name).is_none()
}

fn rename_token_at(text: &str, position: Position) -> Option<RenameToken> {
    let offset = LineIndex::new(text).offset(position);
    let range = identifier_range_at(text, offset)?;
    Some(RenameToken { range })
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
    fn prepare_rename_rejects_keywords_and_literals() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "\
pub fn main(amount: i64) -> i64 {
    return amount + 1
}";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

        assert_eq!(
            databases.prepare_rename(
                &document,
                Position::new(1, line(text, 1).find("return").expect("return keyword"))
            ),
            None
        );
        assert_eq!(
            databases.prepare_rename(
                &document,
                Position::new(1, line(text, 1).find('1').expect("literal"))
            ),
            None
        );
    }

    #[test]
    fn local_rename_updates_all_function_uses() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "\
pub fn main(amount: i64) -> i64 {
    let next = amount + 1
    next += amount
    return next
}";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

        let prepare = databases
            .prepare_rename(
                &document,
                Position::new(2, line(text, 2).find("next").expect("next write")),
            )
            .expect("local binding should be renameable");

        assert_eq!(prepare.document_id(), &document);
        assert_eq!(prepare.placeholder(), "next");
        assert_eq!(prepare.range().start(), Position::new(2, 4));

        let edit = databases
            .rename(
                &document,
                Position::new(2, line(text, 2).find("next").expect("next write")),
                "score",
            )
            .expect("local rename should produce edits");

        let document_edit = edit
            .document_edits()
            .first()
            .expect("rename should edit one document");
        assert_eq!(document_edit.document_id(), &document);
        assert_eq!(document_edit.edits().len(), 3);
        assert_edit_at(document_edit.edits(), 1, 8, "score");
        assert_edit_at(document_edit.edits(), 2, 4, "score");
        assert_edit_at(document_edit.edits(), 3, 11, "score");
    }

    #[test]
    fn rename_rejects_scope_collision() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "\
pub fn main(amount: i64) -> i64 {
    let next = amount + 1
    return next
}";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

        assert_eq!(
            databases.rename(
                &document,
                Position::new(1, line(text, 1).find("next").expect("next local")),
                "amount",
            ),
            None
        );
    }

    fn assert_edit_at(edits: &[TextEdit], line: usize, character: usize, new_text: &str) {
        assert!(
            edits.iter().any(|edit| {
                edit.range().start() == Position::new(line, character)
                    && edit.new_text() == new_text
            }),
            "{edits:?}"
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
