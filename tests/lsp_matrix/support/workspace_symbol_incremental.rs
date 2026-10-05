//! Authored finite edits and complete symbol sets; no production provider calls.
use super::{FixtureWorkspace, load, parse_markers};
use serde_json::Value;

pub(crate) fn cases(crlf: bool, shifted: bool) -> Vec<(FixtureWorkspace, Value)> {
    let mut spec = load("workspace-symbol-incremental");
    let transform = |source: &str| {
        let text = if shifted {
            source.replace(
                "[[file:start]]",
                "[[file:start]]// shifted 中😀\n/* extra 😀 */\n",
            )
        } else {
            source.to_owned()
        };
        if crlf {
            text.replace('\n', "\r\n")
        } else {
            text
        }
    };
    for source in spec.files.values_mut() {
        *source = transform(source);
    }
    let mut fixture = FixtureWorkspace::new(&spec).expect("authored source workspace");
    assert_eq!(fixture.disk.len(), 5);
    let mut cases = Vec::new();
    for phase in spec.oracle["phases"]
        .as_array()
        .expect("nine finite phases")
    {
        if let Some(file) = phase["file"].as_str() {
            fixture.disk.insert(
                file.to_owned(),
                parse_markers(&transform(phase["source"].as_str().expect("edit")))
                    .expect("edit markers"),
            );
        }
        let main = &fixture.disk["scripts/main.vela"];
        let name = main.markers["main-name"];
        assert_eq!(
            (name.start.line, name.start.character, name.end.character),
            (1 + usize::from(shifted) * 2, 15, 19)
        );
        assert_eq!(
            name.start.byte - main.text[..name.start.byte].rfind('\n').expect("line") - 1,
            19
        );
        assert_eq!(
            phase["workspace"]["symbols"]
                .as_array()
                .expect("complete rows")
                .len(),
            19
        );
        assert_eq!(
            phase["workspace"]["queries"]
                .as_array()
                .expect("complete queries")
                .len(),
            17
        );
        cases.push((fixture.clone(), phase.clone()));
    }
    assert_eq!(cases.len(), 9);
    cases
}
