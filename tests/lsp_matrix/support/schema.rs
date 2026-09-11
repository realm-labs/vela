use serde_json::{Value, json};

use super::FixtureWorkspace;

/// Instantiate fixture metadata; the source IDs come from setup, while byte
/// spans come exclusively from the independent marked source corpus.
pub(crate) fn schema_artifact(
    facts: &Value,
    fixture: &FixtureWorkspace,
    source_id: impl Fn(&str) -> u32,
) -> Value {
    let mut facts = facts.clone();
    for entries in facts
        .as_object_mut()
        .expect("schema fact categories")
        .values_mut()
    {
        for entry in entries.as_array_mut().expect("schema fact entries") {
            let Some(span) = entry.get_mut("sourceSpan") else {
                continue;
            };
            let file = span["file"].as_str().expect("source file");
            let marker = fixture.document(file).expect("source document").markers
                [span["marker"].as_str().expect("source marker")];
            *span =
                json!({"source":source_id(file),"start":marker.start.byte,"end":marker.end.byte});
        }
    }
    json!({"formatVersion":1,"facts":facts})
}
