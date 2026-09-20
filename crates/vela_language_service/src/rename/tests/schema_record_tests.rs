use super::*;
use crate::matrix_fixture::parse_markers;

#[test]
fn source_backed_schema_record_labels_produce_complete_owned_edits() {
    for crlf in [false, true] {
        let main = DocumentId::from("/workspace/scripts/main.vela");
        let schema = DocumentId::from("/workspace/scripts/schema_defs.vela");
        let marked = "use host::Record as Alias\n/* 中😀 */ fn main(value: i64, item: host::Record) {\n let a = host::Record { [[explicit:start]]value[[explicit:end]]: value };\n let b = Alias { [[short:start]]value[[short:end]] };\n match a { host::Record { [[pattern:start]]value[[pattern:end]]: bound } => bound };\n item.[[dot:start]]value[[dot:end]];\n}\n";
        let marked = if crlf {
            marked.replace('\n', "\r\n")
        } else {
            marked.to_owned()
        };
        let document = parse_markers(&marked).expect("markers");
        assert!(
            vela_syntax::parse::parse_source(&document.text)
                .diagnostics()
                .is_empty()
        );
        let schema_text = "pub fn value() {}";
        let mut db = databases_for(vec![
            SourceFileSnapshot::new(main.clone(), document.text.as_str()),
            SourceFileSnapshot::new(schema.clone(), schema_text),
        ]);
        let source = db.source_db().records()[&schema].source_id().get();
        let artifact = serde_json::json!({"formatVersion":1,"facts":{
            "types":[{"name":"host::Record","fact":{"kind":"host","name":"host::Record"}}],
            "fields":[{"owner":"host::Record","name":"value","fact":{"kind":"primitive","name":"i64"},"sourceSpan":{"source":source,"start":7,"end":12}},
                {"owner":"host::Record","name":"spare","fact":{"kind":"primitive","name":"i64"}}]
        }});
        db.load_schema_artifact_json("/workspace/target/schema.json", &artifact.to_string());
        let index = LineIndex::new(&document.text);
        let mut expected = ["explicit", "short", "pattern", "dot"]
            .into_iter()
            .map(|marker| {
                let range = document.markers[marker];
                (
                    DiagnosticRange::new(
                        index.position(range.start.byte),
                        index.position(range.end.byte),
                    ),
                    if marker == "short" {
                        "rank: value"
                    } else {
                        "rank"
                    }
                    .to_owned(),
                )
            })
            .collect::<Vec<_>>();
        expected.sort_by_key(|(range, _)| (range.start().line, range.start().character));
        for (file, point) in std::iter::once((&schema, Position::new(0, 8))).chain(
            ["explicit", "pattern", "dot"].into_iter().map(|marker| {
                (
                    &main,
                    index.position(document.markers[marker].start.byte + 1),
                )
            }),
        ) {
            let prepared = db.prepare_rename(file, point).expect("schema field target");
            assert_eq!(
                prepared.symbol(),
                &SymbolRef::Schema("host::Record.value".into())
            );
            assert!(
                db.rename(file, point, "spare").is_none(),
                "schema collision"
            );
            let plan = db.rename(file, point, "rank").expect("schema rename");
            assert_eq!(plan.document_edits().len(), 2);
            let edits = document_edit(&plan, &main);
            let mut actual = edits
                .edits()
                .iter()
                .map(|edit| (edit.range(), edit.new_text().to_owned()))
                .collect::<Vec<_>>();
            actual.sort_by_key(|(range, _)| (range.start().line, range.start().character));
            assert_eq!(actual, expected, "complete constructor/pattern/dot edits");
            assert_edit_at(document_edit(&plan, &schema).edits(), 0, 7, "rank");
            assert_schema_risk(&plan, "host::Record.value");
            let mut applied = document.text.clone();
            for (range, replacement) in actual.iter().rev() {
                applied.replace_range(
                    index.offset(range.start())..index.offset(range.end()),
                    replacement,
                );
            }
            let expected_text = document
                .text
                .replace("Record { value: value", "Record { rank: value")
                .replace("Alias { value }", "Alias { rank: value }")
                .replace("Record { value: bound", "Record { rank: bound")
                .replace("item.value", "item.rank");
            assert_eq!(applied, expected_text);
            assert!(
                vela_syntax::parse::parse_source(&applied)
                    .diagnostics()
                    .is_empty()
            );
            let mut rebuilt = databases_for(vec![
                SourceFileSnapshot::new(main.clone(), applied.as_str()),
                SourceFileSnapshot::new(schema.clone(), "pub fn rank() {}"),
            ]);
            let mut regenerated = artifact.clone();
            regenerated["facts"]["fields"][0]["name"] = "rank".into();
            regenerated["facts"]["fields"][0]["sourceSpan"]["source"] =
                rebuilt.source_db().records()[&schema]
                    .source_id()
                    .get()
                    .into();
            regenerated["facts"]["fields"][0]["sourceSpan"]["end"] = 11.into();
            rebuilt.load_schema_artifact_json(
                "/workspace/target/schema.json",
                &regenerated.to_string(),
            );
            let lines = LineIndex::new(&applied);
            for (offset, _) in applied.match_indices("rank") {
                assert_eq!(
                    rebuilt
                        .prepare_rename(&main, lines.position(offset + 1))
                        .expect("renamed field")
                        .symbol(),
                    &SymbolRef::Schema("host::Record.rank".into())
                );
            }
            let local = applied.find("rank: value }").expect("expanded local") + "rank: ".len();
            let definition = rebuilt
                .definition(&main, lines.position(local + 1))
                .expect("local binding");
            assert_eq!(definition.document_id(), &main);
            assert_eq!(
                definition.range().start(),
                lines.position(applied.find("value: i64").expect("parameter"))
            );
        }
    }
}
