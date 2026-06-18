use std::collections::BTreeMap;

use crate::{DiagnosticRange, DocumentId, Position, SourceVersion, SymbolRef};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PrepareRename {
    pub(super) document_id: DocumentId,
    pub(super) range: DiagnosticRange,
    pub(super) placeholder: String,
    pub(super) symbol: SymbolRef,
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

    #[must_use]
    pub const fn symbol(&self) -> &SymbolRef {
        &self.symbol
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WorkspaceEdit {
    edit_plan: EditPlan,
    risks: Vec<RenameRisk>,
    symbol: Option<SymbolRef>,
}

impl WorkspaceEdit {
    #[must_use]
    pub fn new(document_edits: Vec<DocumentTextEdit>) -> Self {
        Self {
            edit_plan: EditPlan::unchecked(document_edits),
            risks: Vec::new(),
            symbol: None,
        }
    }

    #[must_use]
    pub fn try_new(document_edits: Vec<DocumentTextEdit>) -> Option<Self> {
        Self::checked(document_edits, Vec::new())
    }

    pub(super) fn checked(
        document_edits: Vec<DocumentTextEdit>,
        risks: Vec<RenameRisk>,
    ) -> Option<Self> {
        Some(Self {
            edit_plan: EditPlan::new(document_edits)?,
            risks,
            symbol: None,
        })
    }

    #[must_use]
    pub(super) fn with_symbol(mut self, symbol: SymbolRef) -> Self {
        self.symbol = Some(symbol);
        self
    }

    #[must_use]
    pub const fn edit_plan(&self) -> &EditPlan {
        &self.edit_plan
    }

    #[must_use]
    pub fn document_edits(&self) -> &[DocumentTextEdit] {
        self.edit_plan.document_edits()
    }

    #[must_use]
    pub fn risks(&self) -> &[RenameRisk] {
        &self.risks
    }

    #[must_use]
    pub fn symbol(&self) -> Option<&SymbolRef> {
        self.symbol.as_ref()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct EditPlan {
    document_edits: Vec<DocumentTextEdit>,
}

impl EditPlan {
    fn unchecked(document_edits: Vec<DocumentTextEdit>) -> Self {
        Self { document_edits }
    }

    pub(super) fn new(document_edits: Vec<DocumentTextEdit>) -> Option<Self> {
        edits_are_non_overlapping(&document_edits).then_some(Self { document_edits })
    }

    #[must_use]
    pub fn document_edits(&self) -> &[DocumentTextEdit] {
        &self.document_edits
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RenameRisk {
    pub(super) kind: RenameRiskKind,
    pub(super) message: String,
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
    SchemaAbi,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DocumentTextEdit {
    pub(super) document_id: DocumentId,
    document_version: Option<SourceVersion>,
    pub(super) edits: Vec<TextEdit>,
}

impl DocumentTextEdit {
    #[must_use]
    pub fn new(document_id: DocumentId, edits: Vec<TextEdit>) -> Self {
        Self {
            document_id,
            document_version: None,
            edits,
        }
    }

    #[must_use]
    pub fn new_versioned(
        document_id: DocumentId,
        document_version: SourceVersion,
        edits: Vec<TextEdit>,
    ) -> Self {
        Self {
            document_id,
            document_version: Some(document_version),
            edits,
        }
    }

    #[must_use]
    pub fn document_id(&self) -> &DocumentId {
        &self.document_id
    }

    #[must_use]
    pub const fn document_version(&self) -> Option<SourceVersion> {
        self.document_version
    }

    #[must_use]
    pub fn edits(&self) -> &[TextEdit] {
        &self.edits
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TextEdit {
    pub(super) range: DiagnosticRange,
    pub(super) new_text: String,
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

fn edits_are_non_overlapping(document_edits: &[DocumentTextEdit]) -> bool {
    let mut ranges_by_document = BTreeMap::<DocumentId, Vec<DiagnosticRange>>::new();
    for document_edit in document_edits {
        let ranges = ranges_by_document
            .entry(document_edit.document_id.clone())
            .or_default();
        ranges.extend(document_edit.edits.iter().map(TextEdit::range));
    }
    ranges_by_document
        .values_mut()
        .all(|ranges| ranges_are_non_overlapping(ranges))
}

fn ranges_are_non_overlapping(ranges: &mut [DiagnosticRange]) -> bool {
    ranges.sort_by_key(|range| position_key(range.start()));
    ranges
        .windows(2)
        .all(|pair| position_key(pair[0].end()) <= position_key(pair[1].start()))
}

const fn position_key(position: Position) -> (usize, usize) {
    (position.line, position.character)
}
