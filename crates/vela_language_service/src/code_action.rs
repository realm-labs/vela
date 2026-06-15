use crate::{
    DiagnosticRange, DocumentId, DocumentTextEdit, LanguageServiceDatabases, LineIndex, Position,
    ServiceDiagnostic, TextEdit, WorkspaceEdit,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CodeAction {
    title: String,
    kind: CodeActionKind,
    edit: WorkspaceEdit,
}

impl CodeAction {
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub const fn kind(&self) -> CodeActionKind {
        self.kind
    }

    #[must_use]
    pub const fn edit(&self) -> &WorkspaceEdit {
        &self.edit
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CodeActionKind {
    QuickFix,
}

impl CodeActionKind {
    #[must_use]
    pub const fn as_lsp_kind(self) -> &'static str {
        match self {
            Self::QuickFix => "quickfix",
        }
    }
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn code_actions(
        &self,
        document_id: &DocumentId,
        request_range: DiagnosticRange,
    ) -> Vec<CodeAction> {
        let Some(source) = self.source_db().records().get(document_id) else {
            return Vec::new();
        };
        let diagnostics = self.diagnostics_for_document(document_id);
        diagnostics
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic_overlaps_request(diagnostic, request_range))
            .flat_map(|diagnostic| {
                let mut actions = repair_hint_actions(document_id, diagnostic);
                actions.extend(candidate_actions(document_id, source.text(), diagnostic));
                actions
            })
            .collect()
    }
}

fn repair_hint_actions(
    document_id: &DocumentId,
    diagnostic: &ServiceDiagnostic,
) -> Vec<CodeAction> {
    diagnostic
        .repair_hints()
        .iter()
        .filter(|hint| hint.document_id() == document_id)
        .map(|hint| {
            quick_fix(
                hint.title().to_owned(),
                document_id.clone(),
                hint.range(),
                hint.replacement(),
            )
        })
        .collect()
}

fn candidate_actions(
    document_id: &DocumentId,
    text: &str,
    diagnostic: &ServiceDiagnostic,
) -> Vec<CodeAction> {
    let Some(diagnostic_range) = diagnostic.range() else {
        return Vec::new();
    };
    let Some(misspelled) = backticked_token(diagnostic.message()) else {
        return Vec::new();
    };
    let Some(edit_range) = narrowed_token_range(text, diagnostic_range, misspelled) else {
        return Vec::new();
    };

    diagnostic
        .candidates()
        .iter()
        .map(|candidate| {
            quick_fix(
                format!("Replace with `{}`", candidate.replacement()),
                document_id.clone(),
                edit_range,
                candidate.replacement(),
            )
        })
        .collect()
}

fn quick_fix(
    title: String,
    document_id: DocumentId,
    range: DiagnosticRange,
    replacement: &str,
) -> CodeAction {
    CodeAction {
        title,
        kind: CodeActionKind::QuickFix,
        edit: WorkspaceEdit::new(vec![DocumentTextEdit::new(
            document_id,
            vec![TextEdit::new(range, replacement)],
        )]),
    }
}

fn diagnostic_overlaps_request(
    diagnostic: &ServiceDiagnostic,
    request_range: DiagnosticRange,
) -> bool {
    diagnostic
        .range()
        .is_some_and(|range| ranges_overlap(range, request_range))
        || diagnostic
            .repair_hints()
            .iter()
            .any(|hint| ranges_overlap(hint.range(), request_range))
}

fn ranges_overlap(left: DiagnosticRange, right: DiagnosticRange) -> bool {
    position_le(left.start(), right.end()) && position_le(right.start(), left.end())
}

fn position_le(left: Position, right: Position) -> bool {
    left.line < right.line || left.line == right.line && left.character <= right.character
}

fn backticked_token(message: &str) -> Option<&str> {
    let start = message.find('`')? + '`'.len_utf8();
    let end = message[start..].find('`')? + start;
    let token = &message[start..end];
    (!token.is_empty()).then_some(token)
}

fn narrowed_token_range(
    text: &str,
    diagnostic_range: DiagnosticRange,
    token: &str,
) -> Option<DiagnosticRange> {
    let line_index = LineIndex::new(text);
    let start = line_index.offset(diagnostic_range.start());
    let end = line_index.offset(diagnostic_range.end());
    let haystack = text.get(start..end)?;
    let relative = haystack.rfind(token)?;
    let token_start = start + relative;
    let token_end = token_start + token.len();
    Some(DiagnosticRange::new(
        line_index.position(token_start),
        line_index.position(token_end),
    ))
}

#[cfg(test)]
mod tests {
    use vela_analysis::{registry::RegistryFacts, type_fact::TypeFact};

    use super::*;
    use crate::{
        SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
    };

    #[test]
    fn code_action_fixes_unknown_field_typo() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub fn main(player: Player) { return player.levle }";
        let files = vec![SourceFileSnapshot::new(document.clone(), text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        let mut schema = RegistryFacts::default();
        schema.insert_type("Player", TypeFact::host("Player"));
        schema.insert_field("Player", "level", TypeFact::I64);
        databases.set_schema_facts(schema);
        databases.update(&project);

        let actions = databases.code_actions(
            &document,
            DiagnosticRange::new(Position::new(0, 44), Position::new(0, 49)),
        );

        let action = actions
            .iter()
            .find(|action| action.title() == "Replace with `level`")
            .expect("candidate typo quick fix should exist");
        assert_eq!(action.kind(), CodeActionKind::QuickFix);
        let document_edit = &action.edit().document_edits()[0];
        assert_eq!(document_edit.document_id(), &document);
        let edit = &document_edit.edits()[0];
        let typo_start = text.find("levle").expect("field typo");
        assert_eq!(edit.range().start(), Position::new(0, typo_start));
        assert_eq!(
            edit.range().end(),
            Position::new(0, typo_start + "levle".len())
        );
        assert_eq!(edit.new_text(), "level");
    }
}
