use super::*;
use crate::matrix_fixture::parse_markers;

#[test]
fn source_backed_schema_record_labels_produce_complete_owned_edits() {
    for crlf in [false, true] {
        let main = DocumentId::from("/workspace/scripts/main.vela");
        let schema = DocumentId::from("/workspace/scripts/schema_defs.vela");
        let marked = "use host::Record as Alias\n/* 中😀 */ fn main(value: i64, item: host::Record) {\n let a = host::Record { [[explicit:start]]value[[explicit:end]]: value };\n let b = Alias { [[short:start]]value[[short:end]] };\n match a { host::Record { [[pattern:start]]value[[pattern:end]]: bound } => bound };\n item.[[dot:start]]value[[dot:end]];\n}\n";
        let marked = marked.to_owned()
            + "pub struct Record { value: i64 }\nfn unrelated() { Record { value: 1 }; other::Record { [[metadata:start]]value[[metadata:end]]: 2 }; host::Record { [[unknown:start]]bogus[[unknown:end]]: 3 }; }\n";
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
            "types":[{"name":"host::Record","fact":{"kind":"host","name":"host::Record"}}, {"name":"other::Record","fact":{"kind":"host","name":"other::Record"}}],
            "fields":[{"owner":"host::Record","name":"value","fact":{"kind":"primitive","name":"i64"},"sourceSpan":{"source":source,"start":7,"end":12}},
                {"owner":"host::Record","name":"spare","fact":{"kind":"primitive","name":"i64"}}, {"owner":"other::Record","name":"value","fact":{"kind":"primitive","name":"i64"}}]
        }});
        db.load_schema_artifact_json("/workspace/target/schema.json", &artifact.to_string());
        let index = LineIndex::new(&document.text);
        let unknown = index.position(document.markers["unknown"].start.byte + 1);
        assert!(db.references(&main, unknown, true).is_empty());
        assert!(db.prepare_rename(&main, unknown).is_none());
        let metadata = index.position(document.markers["metadata"].start.byte + 1);
        assert!(db.rename(&main, metadata, "rank").is_none());
        let metadata_refs = db.references(&main, metadata, true);
        assert_eq!(metadata_refs.len(), 1);
        assert_eq!(
            metadata_refs[0].symbol(),
            &SymbolRef::Schema("other::Record.value".into())
        );
        assert_eq!(
            metadata_refs[0].range(),
            DiagnosticRange::new(
                index.position(document.markers["metadata"].start.byte),
                index.position(document.markers["metadata"].end.byte)
            )
        );
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
            for include_declaration in [false, true] {
                let references = db.references(file, point, include_declaration);
                let mut expected_refs = expected
                    .iter()
                    .map(|(range, _)| (main.clone(), *range))
                    .collect::<Vec<_>>();
                if include_declaration {
                    expected_refs.push((
                        schema.clone(),
                        DiagnosticRange::new(Position::new(0, 7), Position::new(0, 12)),
                    ));
                }
                let sort = |items: &mut Vec<(DocumentId, DiagnosticRange)>| {
                    items.sort_by_key(|(doc, range)| {
                        (
                            doc.as_str().to_owned(),
                            range.start().line,
                            range.start().character,
                        )
                    })
                };
                let mut actual_refs = references
                    .iter()
                    .map(|reference| {
                        assert_eq!(
                            reference.symbol(),
                            &SymbolRef::Schema("host::Record.value".into())
                        );
                        let kind = if reference.document_id() == &schema {
                            crate::ReferenceKind::Declaration
                        } else if reference.range().start()
                            == index.position(document.markers["pattern"].start.byte)
                        {
                            crate::ReferenceKind::Pattern
                        } else {
                            crate::ReferenceKind::Read
                        };
                        assert_eq!(reference.kind(), kind);
                        (reference.document_id().clone(), reference.range())
                    })
                    .collect::<Vec<_>>();
                sort(&mut actual_refs);
                sort(&mut expected_refs);
                assert_eq!(actual_refs, expected_refs, "complete schema reference set");
                if include_declaration {
                    let mut actual_highlights = db
                        .document_highlights(file, point)
                        .into_iter()
                        .map(|highlight| (highlight.range(), format!("{:?}", highlight.kind())))
                        .collect::<Vec<_>>();
                    let mut expected_highlights = expected_refs
                        .iter()
                        .filter(|(doc, _)| doc == file)
                        .map(|(doc, range)| {
                            (
                                *range,
                                if doc == &schema
                                    || range.start()
                                        == index.position(document.markers["pattern"].start.byte)
                                {
                                    "Text"
                                } else {
                                    "Read"
                                }
                                .to_owned(),
                            )
                        })
                        .collect::<Vec<_>>();
                    actual_highlights
                        .sort_by_key(|(range, _)| (range.start().line, range.start().character));
                    expected_highlights
                        .sort_by_key(|(range, _)| (range.start().line, range.start().character));
                    assert_eq!(
                        actual_highlights, expected_highlights,
                        "complete schema highlights"
                    );
                }
            }
            let prepared = db.prepare_rename(file, point).expect("schema field target");
            assert_eq!(
                prepared.symbol(),
                &SymbolRef::Schema("host::Record.value".into())
            );
            assert!(
                db.rename(file, point, "spare").is_none(),
                "schema collision"
            );
            assert!(
                db.rename(file, point, "bogus").is_none(),
                "unknown label capture"
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
                for include_declaration in [false, true] {
                    let references =
                        rebuilt.references(&main, lines.position(offset + 1), include_declaration);
                    let mut actual_refs = references
                        .iter()
                        .map(|reference| {
                            assert_eq!(
                                reference.symbol(),
                                &SymbolRef::Schema("host::Record.rank".into())
                            );
                            (
                                reference.document_id().as_str().to_owned(),
                                reference.range().start().line,
                                reference.range().start().character,
                                reference.range().end().character,
                            )
                        })
                        .collect::<Vec<_>>();
                    let mut expected_refs = applied
                        .match_indices("rank")
                        .map(|(start, word)| {
                            let point = lines.position(start);
                            (
                                main.as_str().to_owned(),
                                point.line,
                                point.character,
                                point.character + word.len(),
                            )
                        })
                        .collect::<Vec<_>>();
                    if include_declaration {
                        expected_refs.push((schema.as_str().to_owned(), 0, 7, 11));
                    }
                    actual_refs.sort();
                    expected_refs.sort();
                    assert_eq!(actual_refs, expected_refs, "rebuilt schema reference sets");
                }
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
