use std::collections::{BTreeMap, BTreeSet};

use lsp_types::{FileChangeType, FileEvent};
use vela_language_service::DocumentId;

use crate::{CONFIG_FILE, SOURCE_EXTENSION, document_uri_path, normalized_path};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReloadOperation {
    Upsert,
    Remove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReloadTarget {
    Config,
    Schema,
    Source,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReloadWork {
    WatchedFile {
        generation: u64,
        uri: String,
        operation: ReloadOperation,
        target: ReloadTarget,
        open_file_priority: bool,
    },
    WorkspaceRoots {
        generation: u64,
        roots: BTreeSet<String>,
    },
}

#[derive(Debug, Default)]
pub(crate) struct ReloadScheduler {
    generation: u64,
    pending: Vec<ReloadWork>,
}

impl ReloadScheduler {
    pub(crate) fn schedule_watched_files(
        &mut self,
        changes: Vec<FileEvent>,
        schema_path: Option<&str>,
        open_documents: &BTreeSet<DocumentId>,
    ) {
        let changes = coalesced_watched_file_changes(changes);
        if changes.is_empty() {
            return;
        }
        let generation = self.bump_generation();
        self.pending.extend(
            changes
                .into_iter()
                .map(|change| watched_file_work(generation, change, schema_path, open_documents)),
        );
    }

    pub(crate) fn schedule_workspace_roots(&mut self, roots: BTreeSet<String>) {
        let generation = self.bump_generation();
        self.pending
            .push(ReloadWork::WorkspaceRoots { generation, roots });
    }

    pub(crate) fn drain(&mut self) -> Vec<ReloadWork> {
        let mut pending = std::mem::take(&mut self.pending);
        pending.sort_by_key(|work| !work.open_file_priority());
        pending
    }

    fn bump_generation(&mut self) -> u64 {
        self.generation = self.generation.saturating_add(1);
        self.generation
    }
}

impl ReloadWork {
    const fn open_file_priority(&self) -> bool {
        match self {
            Self::WatchedFile {
                open_file_priority, ..
            } => *open_file_priority,
            Self::WorkspaceRoots { .. } => false,
        }
    }
}

fn watched_file_work(
    generation: u64,
    change: FileEvent,
    schema_path: Option<&str>,
    open_documents: &BTreeSet<DocumentId>,
) -> ReloadWork {
    let uri = change.uri.to_string();
    ReloadWork::WatchedFile {
        generation,
        target: reload_target(&uri, schema_path),
        operation: if change.typ == FileChangeType::DELETED {
            ReloadOperation::Remove
        } else {
            ReloadOperation::Upsert
        },
        open_file_priority: open_documents.contains(&DocumentId::from(uri.clone())),
        uri,
    }
}

fn coalesced_watched_file_changes(changes: Vec<FileEvent>) -> Vec<FileEvent> {
    let mut latest_by_uri = BTreeMap::<String, (usize, FileChangeType)>::new();
    for (index, change) in changes.into_iter().enumerate() {
        latest_by_uri.insert(change.uri.to_string(), (index, change.typ));
    }

    let mut events = latest_by_uri
        .into_iter()
        .map(|(uri, (index, typ))| {
            (
                index,
                FileEvent {
                    uri: uri.parse().expect("coalesced URI should remain valid"),
                    typ,
                },
            )
        })
        .collect::<Vec<_>>();
    events.sort_by_key(|(index, _)| *index);
    events.into_iter().map(|(_, event)| event).collect()
}

fn reload_target(uri: &str, schema_path: Option<&str>) -> ReloadTarget {
    if uri.trim_end_matches('/').ends_with(CONFIG_FILE) {
        ReloadTarget::Config
    } else if is_schema_uri(uri, schema_path) {
        ReloadTarget::Schema
    } else if uri.ends_with(SOURCE_EXTENSION) {
        ReloadTarget::Source
    } else {
        ReloadTarget::Other
    }
}

fn is_schema_uri(uri: &str, schema_path: Option<&str>) -> bool {
    schema_path.is_some_and(|schema_path| {
        normalized_path(document_uri_path(uri)) == normalized_path(schema_path)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watched_file_scheduler_coalesces_classifies_and_bumps_generation() {
        let mut scheduler = ReloadScheduler::default();
        let schema_path = "/workspace/target/vela/schema.json";
        let open_documents = BTreeSet::from([DocumentId::from(
            "file:///workspace/scripts/open.vela".to_owned(),
        )]);

        scheduler.schedule_watched_files(
            vec![
                file_event(
                    "file:///workspace/scripts/open.vela",
                    FileChangeType::CREATED,
                ),
                file_event("file:///workspace/vela.toml", FileChangeType::CHANGED),
                file_event(
                    "file:///workspace/target/vela/schema.json",
                    FileChangeType::CHANGED,
                ),
                file_event(
                    "file:///workspace/scripts/open.vela",
                    FileChangeType::DELETED,
                ),
            ],
            Some(schema_path),
            &open_documents,
        );

        assert_eq!(
            scheduler.drain(),
            vec![
                ReloadWork::WatchedFile {
                    generation: 1,
                    uri: "file:///workspace/scripts/open.vela".to_owned(),
                    operation: ReloadOperation::Remove,
                    target: ReloadTarget::Source,
                    open_file_priority: true,
                },
                ReloadWork::WatchedFile {
                    generation: 1,
                    uri: "file:///workspace/vela.toml".to_owned(),
                    operation: ReloadOperation::Upsert,
                    target: ReloadTarget::Config,
                    open_file_priority: false,
                },
                ReloadWork::WatchedFile {
                    generation: 1,
                    uri: "file:///workspace/target/vela/schema.json".to_owned(),
                    operation: ReloadOperation::Upsert,
                    target: ReloadTarget::Schema,
                    open_file_priority: false,
                },
            ]
        );
    }

    #[test]
    fn reload_drain_keeps_stable_order_within_priority_groups() {
        let mut scheduler = ReloadScheduler::default();
        let open_documents = BTreeSet::from([
            DocumentId::from("file:///workspace/scripts/a.vela".to_owned()),
            DocumentId::from("file:///workspace/scripts/b.vela".to_owned()),
        ]);

        scheduler.schedule_watched_files(
            vec![
                file_event("file:///workspace/scripts/c.vela", FileChangeType::CHANGED),
                file_event("file:///workspace/scripts/a.vela", FileChangeType::CHANGED),
                file_event("file:///workspace/scripts/b.vela", FileChangeType::CHANGED),
                file_event("file:///workspace/scripts/d.vela", FileChangeType::CHANGED),
            ],
            None,
            &open_documents,
        );

        let uris = scheduler
            .drain()
            .into_iter()
            .map(|work| match work {
                ReloadWork::WatchedFile { uri, .. } => uri,
                ReloadWork::WorkspaceRoots { .. } => unreachable!(),
            })
            .collect::<Vec<_>>();

        assert_eq!(
            uris,
            vec![
                "file:///workspace/scripts/a.vela",
                "file:///workspace/scripts/b.vela",
                "file:///workspace/scripts/c.vela",
                "file:///workspace/scripts/d.vela",
            ]
        );
    }

    #[test]
    fn workspace_root_scheduler_records_generation() {
        let mut scheduler = ReloadScheduler::default();
        scheduler.schedule_workspace_roots(BTreeSet::from(["/workspace/scripts".to_owned()]));

        assert_eq!(
            scheduler.drain(),
            vec![ReloadWork::WorkspaceRoots {
                generation: 1,
                roots: BTreeSet::from(["/workspace/scripts".to_owned()]),
            }]
        );
    }

    fn file_event(uri: &str, typ: FileChangeType) -> FileEvent {
        FileEvent {
            uri: uri.parse().expect("test URI should parse"),
            typ,
        }
    }
}
