use std::collections::BTreeMap;

use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingMap, BindingResolution, LocalBinding};
use vela_hir::ids::{HirDeclId, HirLocalId};
use vela_hir::module_graph::{Declaration, DeclarationKind, ImportResolution, ModuleGraph};
use vela_syntax::ast::Visibility;
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
    risks: Vec<RenameRisk>,
}

impl WorkspaceEdit {
    #[must_use]
    pub fn new(document_edits: Vec<DocumentTextEdit>) -> Self {
        Self {
            document_edits,
            risks: Vec::new(),
        }
    }

    #[must_use]
    pub fn document_edits(&self) -> &[DocumentTextEdit] {
        &self.document_edits
    }

    #[must_use]
    pub fn risks(&self) -> &[RenameRisk] {
        &self.risks
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RenameRisk {
    kind: RenameRiskKind,
    message: String,
}

impl RenameRisk {
    #[must_use]
    pub const fn kind(&self) -> RenameRiskKind {
        self.kind
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RenameRiskKind {
    HotReloadAbi,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DocumentTextEdit {
    document_id: DocumentId,
    edits: Vec<TextEdit>,
}

impl DocumentTextEdit {
    #[must_use]
    pub fn new(document_id: DocumentId, edits: Vec<TextEdit>) -> Self {
        Self { document_id, edits }
    }

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
    pub fn new(range: DiagnosticRange, new_text: impl Into<String>) -> Self {
        Self {
            range,
            new_text: new_text.into(),
        }
    }

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
        let target = rename_target(
            self.hir_db().graph(),
            source.source_id(),
            source.text(),
            position,
        )?;
        let token_range = diagnostic_range(source.text(), target.token_range());
        Some(PrepareRename {
            document_id: document_id.clone(),
            range: token_range,
            placeholder: target.placeholder().to_owned(),
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
        let target = rename_target(
            self.hir_db().graph(),
            source.source_id(),
            source.text(),
            position,
        )?;
        match target {
            RenameTarget::Local(target) => {
                self.rename_local(document_id, source.text(), target, new_name)
            }
            RenameTarget::Declaration(target) => self.rename_declaration(target, new_name),
        }
    }

    fn rename_local(
        &self,
        document_id: &DocumentId,
        text: &str,
        target: LocalRenameTarget<'_>,
        new_name: &str,
    ) -> Option<WorkspaceEdit> {
        if local_name_conflicts(target.bindings, target.local, new_name) {
            return None;
        }

        let mut edits = Vec::new();
        if let Some(binding) = target.bindings.local(target.local)
            && let Some(range) = local_binding_name_range(text, binding)
        {
            edits.push(TextEdit {
                range: diagnostic_range(text, range),
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
                            range: diagnostic_range(text, span_text_range(expression.span)?),
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
            risks: Vec::new(),
            document_edits: vec![DocumentTextEdit {
                document_id: document_id.clone(),
                edits,
            }],
        })
    }

    fn rename_declaration(
        &self,
        target: DeclarationRenameTarget<'_>,
        new_name: &str,
    ) -> Option<WorkspaceEdit> {
        let graph = self.hir_db().graph();
        if declaration_name_conflicts(graph, target.declaration, new_name) {
            return None;
        }

        let mut edits_by_document = BTreeMap::<DocumentId, Vec<TextEdit>>::new();
        self.push_declaration_edit(target.declaration, new_name, &mut edits_by_document)?;
        self.push_import_edits(target.declaration, new_name, &mut edits_by_document);
        self.push_declaration_use_edits(target.declaration, new_name, &mut edits_by_document);

        let document_edits = edits_by_document
            .into_iter()
            .map(|(document_id, mut edits)| {
                edits.sort_by_key(|edit| {
                    let start = edit.range.start();
                    (start.line, start.character)
                });
                DocumentTextEdit { document_id, edits }
            })
            .collect::<Vec<_>>();

        Some(WorkspaceEdit {
            document_edits,
            risks: rename_risks_for_declaration(target.declaration),
        })
    }

    fn push_declaration_edit(
        &self,
        declaration: &Declaration,
        new_name: &str,
        edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
    ) -> Option<()> {
        let source = self.source_record_for_rename(declaration.span.source)?;
        let span_range = span_text_range(declaration.span)?;
        let range = name_range_in_text(source.text(), span_range, &declaration.name)?;
        edits_by_document
            .entry(source.document_id().clone())
            .or_default()
            .push(TextEdit {
                range: diagnostic_range(source.text(), range),
                new_text: new_name.to_owned(),
            });
        Some(())
    }

    fn push_import_edits(
        &self,
        declaration: &Declaration,
        new_name: &str,
        edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
    ) {
        let graph = self.hir_db().graph();
        for module in graph.module_ids() {
            let Some(imports) = graph.imports(module) else {
                continue;
            };
            for import in imports {
                let Some(ImportResolution::Declaration(resolved)) = import.resolution else {
                    continue;
                };
                if resolved != declaration.id {
                    continue;
                }
                let Some(source) = self.source_record_for_rename(import.span.source) else {
                    continue;
                };
                let Some(span_range) = span_text_range(import.span) else {
                    continue;
                };
                let Some(name) = import.path.last() else {
                    continue;
                };
                let Some(range) = name_range_in_text(source.text(), span_range, name) else {
                    continue;
                };
                edits_by_document
                    .entry(source.document_id().clone())
                    .or_default()
                    .push(TextEdit {
                        range: diagnostic_range(source.text(), range),
                        new_text: new_name.to_owned(),
                    });
            }
        }
    }

    fn push_declaration_use_edits(
        &self,
        declaration: &Declaration,
        new_name: &str,
        edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
    ) {
        let graph = self.hir_db().graph();
        for owner in graph.declarations() {
            let Some(bindings) = graph.bindings(owner.id) else {
                continue;
            };
            for (expression, resolution) in bindings.resolutions() {
                let BindingResolution::Declaration(resolved) = resolution else {
                    continue;
                };
                if *resolved != declaration.id {
                    continue;
                }
                let Some(expression) = bindings.expression(expression) else {
                    continue;
                };
                let Some(source) = self.source_record_for_rename(expression.span.source) else {
                    continue;
                };
                let Some(range) = span_text_range(expression.span) else {
                    continue;
                };
                if token_text(source.text(), range) != Some(declaration.name.as_str()) {
                    continue;
                }
                edits_by_document
                    .entry(source.document_id().clone())
                    .or_default()
                    .push(TextEdit {
                        range: diagnostic_range(source.text(), range),
                        new_text: new_name.to_owned(),
                    });
            }
        }
    }

    fn source_record_for_rename(&self, source_id: SourceId) -> Option<&crate::SourceRecord> {
        self.source_db()
            .records()
            .values()
            .find(|record| record.source_id() == source_id)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum RenameTarget<'a> {
    Local(LocalRenameTarget<'a>),
    Declaration(DeclarationRenameTarget<'a>),
}

impl RenameTarget<'_> {
    const fn token_range(&self) -> TextRange {
        match self {
            Self::Local(target) => target.token.range,
            Self::Declaration(target) => target.token.range,
        }
    }

    fn placeholder(&self) -> &str {
        match self {
            Self::Local(target) => &target.placeholder,
            Self::Declaration(target) => &target.declaration.name,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct LocalRenameTarget<'a> {
    bindings: &'a BindingMap,
    local: HirLocalId,
    token: RenameToken,
    placeholder: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct DeclarationRenameTarget<'a> {
    declaration: &'a Declaration,
    token: RenameToken,
}

fn rename_target<'a>(
    graph: &'a ModuleGraph,
    source_id: SourceId,
    text: &str,
    position: Position,
) -> Option<RenameTarget<'a>> {
    let token = rename_token_at(text, position)?;
    let offset = u32::try_from(token.range.start).ok()?;

    for declaration in graph.declarations() {
        if declaration.span.source != source_id || !declaration.span.contains(offset) {
            continue;
        }
        if token_text(text, token.range) == Some(declaration.name.as_str())
            && declaration.kind == DeclarationKind::Function
        {
            return Some(RenameTarget::Declaration(DeclarationRenameTarget {
                declaration,
                token,
            }));
        }
        let Some(bindings) = graph.bindings(declaration.id) else {
            continue;
        };
        if let Some(binding) = local_declaration_at_token(text, bindings, &token) {
            return Some(RenameTarget::Local(LocalRenameTarget {
                bindings,
                local: binding.id,
                token,
                placeholder: binding.name.clone(),
            }));
        }
        if let Some(local) = local_use_at_token(bindings, &token)
            && let Some(binding) = bindings.local(local)
        {
            return Some(RenameTarget::Local(LocalRenameTarget {
                bindings,
                local,
                token,
                placeholder: binding.name.clone(),
            }));
        }
        if let Some(declaration_id) = declaration_use_at_token(bindings, &token)
            && let Some(target) = graph.declaration(declaration_id)
            && target.kind == DeclarationKind::Function
        {
            return Some(RenameTarget::Declaration(DeclarationRenameTarget {
                declaration: target,
                token,
            }));
        }
    }

    for module in graph.module_ids() {
        let Some(imports) = graph.imports(module) else {
            continue;
        };
        for import in imports {
            if import.span.source != source_id || !import.span.contains(offset) {
                continue;
            }
            let Some(ImportResolution::Declaration(declaration_id)) = import.resolution else {
                continue;
            };
            let Some(name) = import.path.last() else {
                continue;
            };
            if token_text(text, token.range) != Some(name.as_str()) {
                continue;
            }
            let Some(target) = graph.declaration(declaration_id) else {
                continue;
            };
            if target.kind != DeclarationKind::Function {
                continue;
            }
            return Some(RenameTarget::Declaration(DeclarationRenameTarget {
                declaration: target,
                token,
            }));
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

fn declaration_use_at_token(bindings: &BindingMap, token: &RenameToken) -> Option<HirDeclId> {
    let resolution = narrowest_resolution_at_token(bindings, token)?;
    match resolution {
        BindingResolution::Declaration(declaration) => Some(*declaration),
        BindingResolution::Local(_)
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

fn declaration_name_conflicts(
    graph: &ModuleGraph,
    declaration: &Declaration,
    new_name: &str,
) -> bool {
    graph
        .module(declaration.module)
        .and_then(|declarations| declarations.get(new_name))
        .is_some_and(|existing| existing != declaration.id)
}

fn rename_risks_for_declaration(declaration: &Declaration) -> Vec<RenameRisk> {
    if declaration.visibility != Visibility::Public {
        return Vec::new();
    }

    vec![RenameRisk {
        kind: RenameRiskKind::HotReloadAbi,
        message: format!(
            "renaming public function `{}` can break hot-reload ABI compatibility and external callers",
            declaration.name
        ),
    }]
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

    #[test]
    fn private_function_rename_updates_imports() {
        let main = DocumentId::from("/workspace/scripts/game/main.vela");
        let helper = DocumentId::from("/workspace/scripts/game/reward.vela");
        let main_text = "\
use game::reward::grant
pub fn main(amount: i64) -> i64 {
    return grant(amount)
}";
        let helper_text = "pub fn grant(amount: i64) -> i64 { return amount }";
        let databases = databases_for(vec![
            SourceFileSnapshot::new(main.clone(), main_text),
            SourceFileSnapshot::new(helper.clone(), helper_text),
        ]);

        let prepare = databases
            .prepare_rename(
                &main,
                Position::new(2, line(main_text, 2).find("grant").expect("grant call")),
            )
            .expect("script function should be renameable from call site");

        assert_eq!(prepare.placeholder(), "grant");
        assert_eq!(prepare.range().start(), Position::new(2, 11));

        let edit = databases
            .rename(
                &main,
                Position::new(2, line(main_text, 2).find("grant").expect("grant call")),
                "award",
            )
            .expect("script function rename should produce workspace edits");

        let main_edit = document_edit(&edit, &main);
        assert_eq!(main_edit.edits().len(), 2);
        assert_edit_at(main_edit.edits(), 0, 18, "award");
        assert_edit_at(main_edit.edits(), 2, 11, "award");

        let helper_edit = document_edit(&edit, &helper);
        assert_eq!(helper_edit.edits().len(), 1);
        assert_edit_at(helper_edit.edits(), 0, 7, "award");
    }

    #[test]
    fn public_export_rename_reports_hot_reload_risk() {
        let document = DocumentId::from("/workspace/scripts/game/reward.vela");
        let text = "pub fn grant(amount: i64) -> i64 { return amount }";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

        let edit = databases
            .rename(
                &document,
                Position::new(0, line(text, 0).find("grant").expect("grant declaration")),
                "award",
            )
            .expect("public function rename should still produce edits");

        assert_eq!(edit.risks().len(), 1);
        assert_eq!(edit.risks()[0].kind(), RenameRiskKind::HotReloadAbi);
        assert!(
            edit.risks()[0]
                .message()
                .contains("public function `grant`")
        );
        let document_edit = document_edit(&edit, &document);
        assert_eq!(document_edit.edits().len(), 1);
        assert_edit_at(document_edit.edits(), 0, 7, "award");
    }

    #[test]
    fn rename_rejects_module_declaration_collision() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "\
pub fn grant(amount: i64) -> i64 { return amount }
pub fn award(amount: i64) -> i64 { return amount + 1 }";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

        assert_eq!(
            databases.rename(
                &document,
                Position::new(0, line(text, 0).find("grant").expect("grant declaration")),
                "award",
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

    fn document_edit<'a>(
        edit: &'a WorkspaceEdit,
        document_id: &DocumentId,
    ) -> &'a DocumentTextEdit {
        edit.document_edits()
            .iter()
            .find(|document_edit| document_edit.document_id() == document_id)
            .unwrap_or_else(|| panic!("workspace edit should contain {document_id:?}"))
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
