use std::collections::BTreeMap;
use std::sync::Arc;

use crate::LineIndex;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocumentId(Arc<str>);

impl DocumentId {
    #[must_use]
    pub fn new(id: impl Into<Arc<str>>) -> Self {
        Self(id.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for DocumentId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for DocumentId {
    fn from(value: String) -> Self {
        Self::new(Arc::<str>::from(value))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceVersion(u64);

impl SourceVersion {
    pub const INITIAL: Self = Self(1);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkspaceGeneration(u64);

impl WorkspaceGeneration {
    pub const INITIAL: Self = Self(0);

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    fn advance(&mut self) {
        self.0 = self.0.saturating_add(1);
    }
}

#[derive(Debug, Clone)]
struct DocumentEntry {
    text: Arc<str>,
    version: SourceVersion,
    line_index: Arc<LineIndex>,
}

impl DocumentEntry {
    fn new(text: impl Into<Arc<str>>, version: SourceVersion) -> Self {
        let text = text.into();
        let line_index = Arc::new(LineIndex::new(&text));
        Self {
            text,
            version,
            line_index,
        }
    }

    fn snapshot(&self) -> DocumentSnapshot {
        DocumentSnapshot {
            text: Arc::clone(&self.text),
            version: self.version,
            line_index: Arc::clone(&self.line_index),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DocumentSnapshot {
    text: Arc<str>,
    version: SourceVersion,
    line_index: Arc<LineIndex>,
}

impl DocumentSnapshot {
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub const fn version(&self) -> SourceVersion {
        self.version
    }

    #[must_use]
    pub fn line_index(&self) -> &LineIndex {
        &self.line_index
    }
}

#[derive(Debug, Default)]
pub struct Workspace {
    open_documents: BTreeMap<DocumentId, DocumentEntry>,
    disk_snapshots: BTreeMap<DocumentId, DocumentEntry>,
    generation: WorkspaceGeneration,
}

impl Workspace {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn generation(&self) -> WorkspaceGeneration {
        self.generation
    }

    pub fn set_disk_snapshot(
        &mut self,
        document_id: impl Into<DocumentId>,
        text: impl Into<Arc<str>>,
        version: SourceVersion,
    ) {
        self.disk_snapshots
            .insert(document_id.into(), DocumentEntry::new(text, version));
        self.generation.advance();
    }

    pub fn open_document(
        &mut self,
        document_id: impl Into<DocumentId>,
        text: impl Into<Arc<str>>,
        version: SourceVersion,
    ) {
        self.open_documents
            .insert(document_id.into(), DocumentEntry::new(text, version));
        self.generation.advance();
    }

    pub fn change_document(
        &mut self,
        document_id: impl Into<DocumentId>,
        text: impl Into<Arc<str>>,
        version: SourceVersion,
    ) {
        self.open_document(document_id, text, version);
    }

    pub fn close_document(&mut self, document_id: &DocumentId) {
        if self.open_documents.remove(document_id).is_some() {
            self.generation.advance();
        }
    }

    #[must_use]
    pub fn document_text(&self, document_id: &DocumentId) -> Option<&str> {
        self.document_entry(document_id)
            .map(|entry| entry.text.as_ref())
    }

    #[must_use]
    pub fn document(&self, document_id: &DocumentId) -> Option<DocumentSnapshot> {
        self.document_entry(document_id)
            .map(DocumentEntry::snapshot)
    }

    #[must_use]
    pub fn snapshot(&self) -> WorkspaceSnapshot {
        WorkspaceSnapshot {
            open_documents: self.open_documents.clone(),
            disk_snapshots: self.disk_snapshots.clone(),
            generation: self.generation,
        }
    }

    fn document_entry(&self, document_id: &DocumentId) -> Option<&DocumentEntry> {
        self.open_documents
            .get(document_id)
            .or_else(|| self.disk_snapshots.get(document_id))
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceSnapshot {
    open_documents: BTreeMap<DocumentId, DocumentEntry>,
    disk_snapshots: BTreeMap<DocumentId, DocumentEntry>,
    generation: WorkspaceGeneration,
}

impl WorkspaceSnapshot {
    #[must_use]
    pub const fn generation(&self) -> WorkspaceGeneration {
        self.generation
    }

    #[must_use]
    pub fn document_text(&self, document_id: &DocumentId) -> Option<&str> {
        self.document_entry(document_id)
            .map(|entry| entry.text.as_ref())
    }

    #[must_use]
    pub fn document(&self, document_id: &DocumentId) -> Option<DocumentSnapshot> {
        self.document_entry(document_id)
            .map(DocumentEntry::snapshot)
    }

    fn document_entry(&self, document_id: &DocumentId) -> Option<&DocumentEntry> {
        self.open_documents
            .get(document_id)
            .or_else(|| self.disk_snapshots.get(document_id))
    }
}

impl Default for WorkspaceGeneration {
    fn default() -> Self {
        Self::INITIAL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> DocumentId {
        DocumentId::from("file:///workspace/main.vela")
    }

    #[test]
    fn open_document_creates_overlay() {
        let mut workspace = Workspace::new();
        let document_id = doc();

        workspace.set_disk_snapshot(document_id.clone(), "let value = 1;", SourceVersion::new(1));
        workspace.open_document(document_id.clone(), "let value = 2;", SourceVersion::new(2));

        let snapshot = workspace.document(&document_id).expect("document exists");
        assert_eq!(snapshot.text(), "let value = 2;");
        assert_eq!(snapshot.version(), SourceVersion::new(2));
        assert_eq!(snapshot.line_index().position(4).line, 0);
    }

    #[test]
    fn change_document_updates_version_and_generation() {
        let mut workspace = Workspace::new();
        let document_id = doc();

        workspace.open_document(document_id.clone(), "let value = 1;", SourceVersion::new(1));
        let before = workspace.generation();
        workspace.change_document(document_id.clone(), "let value = 2;", SourceVersion::new(2));

        assert!(workspace.generation() > before);
        let snapshot = workspace.document(&document_id).expect("document exists");
        assert_eq!(snapshot.text(), "let value = 2;");
        assert_eq!(snapshot.version(), SourceVersion::new(2));
    }

    #[test]
    fn close_document_preserves_disk_snapshot() {
        let mut workspace = Workspace::new();
        let document_id = doc();

        workspace.set_disk_snapshot(
            document_id.clone(),
            "let disk = true;",
            SourceVersion::new(1),
        );
        workspace.open_document(
            document_id.clone(),
            "let overlay = true;",
            SourceVersion::new(2),
        );
        workspace.close_document(&document_id);

        let snapshot = workspace
            .document(&document_id)
            .expect("disk snapshot remains");
        assert_eq!(snapshot.text(), "let disk = true;");
        assert_eq!(snapshot.version(), SourceVersion::new(1));
    }

    #[test]
    fn snapshot_reads_are_generation_stable() {
        let mut workspace = Workspace::new();
        let document_id = doc();

        workspace.open_document(document_id.clone(), "let value = 1;", SourceVersion::new(1));
        let snapshot = workspace.snapshot();
        let snapshot_generation = snapshot.generation();

        workspace.change_document(document_id.clone(), "let value = 2;", SourceVersion::new(2));

        assert_eq!(snapshot.generation(), snapshot_generation);
        assert_eq!(snapshot.document_text(&document_id), Some("let value = 1;"));
        assert_eq!(
            workspace.document_text(&document_id),
            Some("let value = 2;")
        );
    }
}
