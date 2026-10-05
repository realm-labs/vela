//! Finite independently authored body/import changes; never query a provider.
use super::{FixtureWorkspace, load};
use serde_json::{Value, json};

pub(crate) fn cases(
    group: &str,
    crlf: bool,
    shifted: bool,
) -> Vec<(String, FixtureWorkspace, Value)> {
    let changes: Value =
        serde_json::from_str(include_str!("../fixtures/workspace-symbol-states.json"))
            .expect("authored changes");
    let base = load("workspace-symbol-ownership");
    assert_eq!(
        base.oracle["symbols"].as_array().expect("whole rows").len(),
        60
    );
    assert_eq!(
        base.oracle["queries"]
            .as_array()
            .expect("whole queries")
            .len(),
        49
    );
    changes["sequences"][group]
        .as_array()
        .expect("finite sequence")
        .iter()
        .map(|step| {
            let mut spec = base.clone();
            let file = changes["file"].as_str().expect("main file");
            let source = spec.files.get_mut(file).expect("main source");
            let import_anchor = "/* 中😀 */ [[local-range:start]]";
            let body_anchor = "\n}[[uses-range:end]]";
            assert_eq!(source.matches(import_anchor).count(), 1);
            assert_eq!(source.matches(body_anchor).count(), 1);
            *source = source
                .replacen(
                    import_anchor,
                    &format!(
                        "{}{import_anchor}",
                        step["imports"].as_str().expect("imports")
                    ),
                    1,
                )
                .replacen(
                    body_anchor,
                    &format!(
                        "\n{}}}[[uses-range:end]]",
                        step["body"].as_str().expect("body")
                    ),
                    1,
                );
            for (file, text) in &mut spec.files {
                if file.ends_with(".vela") {
                    if shifted {
                        *text = text.replace(
                            "[[file:start]]",
                            "[[file:start]]// shifted 中😀\n/* extra 😀 */\n",
                        );
                    }
                    if crlf {
                        *text = text.replace('\n', "\r\n");
                    }
                }
            }
            for query in changes["emptyQueries"].as_array().expect("empty probes") {
                spec.oracle["queries"]
                    .as_array_mut()
                    .expect("query list")
                    .push(json!({
                        "id":query,"query":query,"symbols":[]
                    }));
            }
            let fixture = FixtureWorkspace::new(&spec).expect("owned markers");
            (
                step["id"].as_str().expect("step ID").to_owned(),
                fixture,
                spec.oracle,
            )
        })
        .collect()
}
