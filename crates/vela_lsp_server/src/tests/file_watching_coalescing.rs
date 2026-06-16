use crate::{FileEvent, coalesced_watched_file_changes};

#[test]
fn watched_file_batch_coalesces_to_last_event_per_uri() {
    let changes = coalesced_watched_file_changes(vec![
        FileEvent {
            uri: "file:///workspace/scripts/a.vela".to_owned(),
            kind: 1,
        },
        FileEvent {
            uri: "file:///workspace/scripts/b.vela".to_owned(),
            kind: 1,
        },
        FileEvent {
            uri: "file:///workspace/scripts/a.vela".to_owned(),
            kind: 3,
        },
        FileEvent {
            uri: "file:///workspace/vela.toml".to_owned(),
            kind: 2,
        },
        FileEvent {
            uri: "file:///workspace/scripts/b.vela".to_owned(),
            kind: 2,
        },
    ]);

    assert_eq!(changes.len(), 3);
    assert_eq!(changes[0].uri, "file:///workspace/scripts/a.vela");
    assert_eq!(changes[0].kind, 3);
    assert_eq!(changes[1].uri, "file:///workspace/vela.toml");
    assert_eq!(changes[1].kind, 2);
    assert_eq!(changes[2].uri, "file:///workspace/scripts/b.vela");
    assert_eq!(changes[2].kind, 2);
}
