use vela_hir::module_graph::{Declaration, DeclarationKind};
use vela_syntax::ast::Visibility;

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
                actions.extend(self.import_actions(document_id, source.text(), diagnostic));
                actions.extend(fill_match_arm_actions(
                    document_id,
                    source.text(),
                    diagnostic,
                ));
                actions
            })
            .collect()
    }

    fn import_actions(
        &self,
        document_id: &DocumentId,
        text: &str,
        diagnostic: &ServiceDiagnostic,
    ) -> Vec<CodeAction> {
        if diagnostic.code() != Some("hir::unresolved_name") {
            return Vec::new();
        }
        let Some(name) = backticked_token(diagnostic.message()) else {
            return Vec::new();
        };
        let Some(current_module_path) = self.project_db().module_by_document().get(document_id)
        else {
            return Vec::new();
        };
        let Some(current_module) = self.hir_db().graph().module_id(current_module_path) else {
            return Vec::new();
        };
        let matches = self
            .hir_db()
            .graph()
            .declarations()
            .filter(|declaration| importable_declaration(declaration, current_module, name))
            .collect::<Vec<_>>();
        let [declaration] = matches.as_slice() else {
            return Vec::new();
        };
        let Some(module_path) = self.hir_db().graph().module_path(declaration.module) else {
            return Vec::new();
        };
        let import_path = format!("{}::{}", module_path.join(), declaration.name);
        let range = import_insertion_range(text);
        vec![CodeAction {
            title: format!("Import `{import_path}`"),
            kind: CodeActionKind::QuickFix,
            edit: WorkspaceEdit::new(vec![DocumentTextEdit::new(
                document_id.clone(),
                vec![TextEdit::new(range, format!("use {import_path}\n"))],
            )]),
        }]
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

fn importable_declaration(
    declaration: &Declaration,
    current_module: vela_hir::ids::ModuleId,
    name: &str,
) -> bool {
    declaration.module != current_module
        && declaration.name == name
        && declaration.visibility == Visibility::Public
        && matches!(
            declaration.kind,
            DeclarationKind::Const
                | DeclarationKind::Global
                | DeclarationKind::Function
                | DeclarationKind::Struct
                | DeclarationKind::Enum
                | DeclarationKind::Trait
        )
}

fn import_insertion_range(text: &str) -> DiagnosticRange {
    let mut offset = 0usize;
    let mut insertion_offset = 0usize;
    let mut saw_import = false;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("use ") {
            saw_import = true;
            offset += line.len();
            insertion_offset = offset;
            continue;
        }
        if !saw_import && trimmed.trim().is_empty() {
            offset += line.len();
            insertion_offset = offset;
            continue;
        }
        break;
    }

    let line_index = LineIndex::new(text);
    let position = line_index.position(insertion_offset);
    DiagnosticRange::new(position, position)
}

fn fill_match_arm_actions(
    document_id: &DocumentId,
    text: &str,
    diagnostic: &ServiceDiagnostic,
) -> Vec<CodeAction> {
    if diagnostic.code() != Some("analysis::non_exhaustive_match") {
        return Vec::new();
    }
    let Some(enum_name) = backticked_token(diagnostic.message()) else {
        return Vec::new();
    };
    let missing = missing_variants(diagnostic);
    if missing.is_empty() {
        return Vec::new();
    }
    let Some(range) = diagnostic.range() else {
        return Vec::new();
    };
    let Some((insert_range, closing_indent)) = match_arm_insertion(text, range) else {
        return Vec::new();
    };

    let mut edit_text = String::new();
    for variant in &missing {
        edit_text.push_str("    ");
        edit_text.push_str(enum_name);
        edit_text.push_str("::");
        edit_text.push_str(variant);
        edit_text.push_str(" => null,\n");
    }
    edit_text.push_str(&closing_indent);

    vec![CodeAction {
        title: format!("Add missing match arms for `{enum_name}`"),
        kind: CodeActionKind::QuickFix,
        edit: WorkspaceEdit::new(vec![DocumentTextEdit::new(
            document_id.clone(),
            vec![TextEdit::new(insert_range, edit_text)],
        )]),
    }]
}

fn missing_variants(diagnostic: &ServiceDiagnostic) -> Vec<String> {
    diagnostic
        .labels()
        .iter()
        .find_map(|label| label.message().strip_prefix("missing variants: "))
        .map(|variants| {
            variants
                .split(',')
                .map(str::trim)
                .filter(|variant| !variant.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn match_arm_insertion(
    text: &str,
    diagnostic_range: DiagnosticRange,
) -> Option<(DiagnosticRange, String)> {
    let line_index = LineIndex::new(text);
    let end = line_index.offset(diagnostic_range.end());
    let brace_offset = text.get(..end)?.rfind('}')?;
    let line_start = text
        .get(..brace_offset)?
        .rfind('\n')
        .map_or(0, |offset| offset + 1);
    let closing_indent = text.get(line_start..brace_offset)?.to_owned();
    if !closing_indent.chars().all(char::is_whitespace) {
        return None;
    }
    let position = line_index.position(brace_offset);
    Some((DiagnosticRange::new(position, position), closing_indent))
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
        SourceFileSnapshot, SourceVersion, Workspace, WorkspaceConfig, WorkspaceRoot,
        assemble_project_sources,
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

    #[test]
    fn code_action_inserts_missing_import() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub fn main() { return grant }";
        let files = vec![
            SourceFileSnapshot::new(document.clone(), text),
            SourceFileSnapshot::new(
                "/workspace/scripts/game/reward.vela",
                "pub fn grant() { return 1 }",
            ),
        ];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        let grant_start = text.find("grant").expect("unresolved call");
        let actions = databases.code_actions(
            &document,
            DiagnosticRange::new(
                Position::new(0, grant_start),
                Position::new(0, grant_start + "grant".len()),
            ),
        );

        let action = actions
            .iter()
            .find(|action| action.title() == "Import `game::reward::grant`")
            .expect("missing import quick fix should exist");
        assert_eq!(action.kind(), CodeActionKind::QuickFix);
        let document_edit = &action.edit().document_edits()[0];
        assert_eq!(document_edit.document_id(), &document);
        let edit = &document_edit.edits()[0];
        assert_eq!(edit.range().start(), Position::new(0, 0));
        assert_eq!(edit.range().end(), Position::new(0, 0));
        assert_eq!(edit.new_text(), "use game::reward::grant\n");
    }

    #[test]
    fn code_action_fills_enum_match_arms() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "\
pub fn main(maybe_name: Option<String>) {
    match maybe_name {
        Option::Some(name) => name,
    }
}";
        let files = vec![SourceFileSnapshot::new(document.clone(), text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        let actions = databases.code_actions(
            &document,
            DiagnosticRange::new(Position::new(1, 4), Position::new(3, 5)),
        );

        let action = actions
            .iter()
            .find(|action| action.title() == "Add missing match arms for `Option`")
            .expect("missing match arm quick fix should exist");
        let edit = &action.edit().document_edits()[0].edits()[0];
        assert_eq!(edit.range().start(), Position::new(3, 4));
        assert_eq!(edit.range().end(), Position::new(3, 4));
        assert_eq!(edit.new_text(), "    Option::None => null,\n    ");
    }

    #[test]
    fn code_action_rejects_ambiguous_dynamic_fix() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub fn main(player) { return player.levle + grant }";
        let files = vec![
            SourceFileSnapshot::new(document.clone(), text),
            SourceFileSnapshot::new(
                "/workspace/scripts/game/reward.vela",
                "pub fn grant() { return 1 }",
            ),
            SourceFileSnapshot::new(
                "/workspace/scripts/game/bonus.vela",
                "pub fn grant() { return 2 }",
            ),
        ];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        let dynamic_member_start = text.find("levle").expect("dynamic member typo");
        let dynamic_actions = databases.code_actions(
            &document,
            DiagnosticRange::new(
                Position::new(0, dynamic_member_start),
                Position::new(0, dynamic_member_start + "levle".len()),
            ),
        );
        assert!(
            dynamic_actions.is_empty(),
            "dynamic receiver typos must not invent type facts: {dynamic_actions:?}"
        );

        let grant_start = text.find("grant").expect("ambiguous missing import");
        let import_actions = databases.code_actions(
            &document,
            DiagnosticRange::new(
                Position::new(0, grant_start),
                Position::new(0, grant_start + "grant".len()),
            ),
        );
        assert!(
            import_actions.is_empty(),
            "ambiguous imports must not choose an arbitrary module: {import_actions:?}"
        );
    }

    #[test]
    fn code_action_ranges_follow_open_overlay_text() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let disk_text = "pub fn main(player: Player) { return player.level }";
        let overlay_text = "\npub fn main(player: Player) {
    return player.levle
}";
        let files = vec![SourceFileSnapshot::new(document.clone(), disk_text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let mut workspace = Workspace::new();
        workspace.open_document(
            document.clone(),
            overlay_text,
            SourceVersion::new(SourceVersion::INITIAL.get() + 1),
        );
        let project = assemble_project_sources(&config, &files, &workspace.snapshot());
        let mut databases = LanguageServiceDatabases::new();
        let mut schema = RegistryFacts::default();
        schema.insert_type("Player", TypeFact::host("Player"));
        schema.insert_field("Player", "level", TypeFact::I64);
        databases.set_schema_facts(schema);
        databases.update(&project);

        let index = LineIndex::new(overlay_text);
        let typo_start = overlay_text.find("levle").expect("overlay typo");
        let typo_end = typo_start + "levle".len();
        let actions = databases.code_actions(
            &document,
            DiagnosticRange::new(index.position(typo_start), index.position(typo_end)),
        );

        let action = actions
            .iter()
            .find(|action| action.title() == "Replace with `level`")
            .expect("overlay-backed typo quick fix should exist");
        let edit = &action.edit().document_edits()[0].edits()[0];
        assert_eq!(edit.range().start(), index.position(typo_start));
        assert_eq!(edit.range().end(), index.position(typo_end));
        assert_eq!(edit.new_text(), "level");
    }
}
