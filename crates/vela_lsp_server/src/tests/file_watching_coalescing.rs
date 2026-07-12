use std::collections::BTreeSet;

use lsp_types::{FileChangeType, FileEvent, Url};

use crate::reload::{ReloadOperation, ReloadScheduler, ReloadWork};

#[test]
fn watched_file_batch_coalesces_to_last_event_per_uri() {
    let mut scheduler = ReloadScheduler::default();
    scheduler.schedule_watched_files(
        vec![
            event("file:///workspace/scripts/a.vela", FileChangeType::CREATED),
            event("file:///workspace/scripts/b.vela", FileChangeType::CREATED),
            event("file:///workspace/scripts/a.vela", FileChangeType::DELETED),
            event("file:///workspace/vela.toml", FileChangeType::CHANGED),
            event("file:///workspace/scripts/b.vela", FileChangeType::CHANGED),
        ],
        None,
        &BTreeSet::new(),
    );
    let changes = scheduler.drain();

    assert_eq!(changes.len(), 3);
    assert_watched_file(
        &changes[0],
        "file:///workspace/scripts/a.vela",
        ReloadOperation::Remove,
    );
    assert_watched_file(
        &changes[1],
        "file:///workspace/vela.toml",
        ReloadOperation::Upsert,
    );
    assert_watched_file(
        &changes[2],
        "file:///workspace/scripts/b.vela",
        ReloadOperation::Upsert,
    );
}

fn event(uri: &str, typ: FileChangeType) -> FileEvent {
    FileEvent {
        uri: Url::parse(uri).expect("test URI should parse"),
        typ,
    }
}

fn assert_watched_file(work: &ReloadWork, expected_uri: &str, expected: ReloadOperation) {
    let ReloadWork::WatchedFile { uri, operation, .. } = work else {
        panic!("expected watched-file work: {work:?}");
    };
    assert_eq!(uri, expected_uri);
    assert_eq!(*operation, expected);
}
