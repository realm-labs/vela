use serde_json::json;

use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, LineIndex, SchemaArtifact,
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

/// A rejected schema must clear the old owner, not leave borrowed-resource facts
/// available to any of the query surfaces that used to consume them.
pub(crate) fn assert_removed_resource_surface() {
    let document = DocumentId::from("/workspace/scripts/main.vela");
    let text = "fn main(player: Player) { let item = player.item_mut(); return item; }";
    let mut db = LanguageServiceDatabases::new();
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    db.update(&assemble_project_sources(
        &config,
        &[SourceFileSnapshot::new(document.clone(), text)],
        &Workspace::new().snapshot(),
    ));
    let mut schema = json!({
        "formatVersion": 4,
        "facts": {
            "types": [{"name": "Player", "fact": {"kind": "record", "name": "Player"}}],
            "methods": [{"owner": "Player", "name": "item_mut", "fact": {
                "kind": "function", "params": [], "returns": {"kind": "record", "name": "Item"}
            }}]
        }
    });
    db.load_schema_artifact_json("/workspace/schema.json", &schema.to_string());
    let index = LineIndex::new(text);
    let member = index.position(text.find("item_mut").expect("member"));
    assert!(db.hover(&document, member).is_some());
    schema["facts"]["methods"][0]["scoped_resource"] = json!({
        "kind": "mut_view", "parent": {"source": "receiver"}
    });
    db.load_schema_artifact_json("/workspace/schema.json", &schema.to_string());
    assert_eq!(db.schema_db().diagnostics().len(), 1);
    assert!(db.schema_db().facts().types().next().is_none());
    assert!(db.hover(&document, member).is_none());
    assert!(
        db.signature_help(
            &document,
            index.position(text.find("item_mut(").expect("call") + 9)
        )
        .is_none()
    );
    assert!(
        db.completion_items(&document, member)
            .items()
            .iter()
            .all(|item| item.label() != "item_mut")
    );
    assert!(
        db.code_actions(&document, DiagnosticRange::new(member, member))
            .iter()
            .all(|action| !action.title().contains("release"))
    );
}

#[test]
fn schema_four_rejects_removed_shapes_recursively_without_defaulting() {
    for kind in [
        "host",
        "arrayView",
        "arrayMut",
        "mapView",
        "mapMut",
        "setView",
        "setMut",
        "scopedIterator",
    ] {
        let removed = json!({"kind": kind, "name": "Player", "element": {"kind": "any"}, "key": {"kind": "any"}, "value": {"kind": "any"}, "mutation": "fixed"});
        for fact in [
            removed.clone(),
            json!({"kind": "option", "some": removed.clone()}),
            json!({"kind": "function", "params": [removed], "returns": {"kind": "any"}}),
        ] {
            let schema = json!({"formatVersion": 4, "facts": {"types": [{"name": "Removed", "fact": fact}]}});
            assert!(
                SchemaArtifact::from_json(&schema.to_string()).is_err(),
                "{schema}"
            );
        }
    }
    let current = json!({"name": "Value", "id": "0x1", "abiFingerprint": "0x2"});
    for field in [
        "storage",
        "capabilities",
        "collectionView",
        "collection_view",
        "constructorIds",
        "constructor_ids",
        "hostConstructors",
        "host_constructors",
    ] {
        for value in [json!(null), json!([]), json!({}), json!("host")] {
            let mut binding = current.clone();
            binding[field] = value;
            let schema = json!({"formatVersion": 4, "facts": {"typeBindings": [binding]}});
            assert!(
                SchemaArtifact::from_json(&schema.to_string()).is_err(),
                "{schema}"
            );
            assert!(serde_json::from_value::<SchemaArtifact>(schema).is_err());
        }
    }
    for collection in ["fields", "variants", "methods", "traitMethods", "functions"] {
        for field in ["scoped_resource", "scopedResource"] {
            let mut entry = json!({"name": "removed", "fact": {"kind": "any"}});
            if collection != "functions" {
                entry["owner"] = json!("Value");
            }
            entry[field] = json!(null);
            let schema = json!({"formatVersion": 4, "facts": {collection: [entry]}});
            assert!(
                SchemaArtifact::from_json(&schema.to_string()).is_err(),
                "{schema}"
            );
        }
    }
    let schema = json!({"formatVersion": 4, "facts": {"methodAccess": [{"owner":"Value", "name":"removed", "receiver":"shared", "public":true, "reflect_callable":true}]}});
    assert!(SchemaArtifact::from_json(&schema.to_string()).is_err());
}

#[test]
fn removed_host_release_and_extern_bindings_have_no_tooling_surface_but_vm_state_remains() {
    let document = DocumentId::from("/workspace/scripts/main.vela");
    let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
    let mut db = LanguageServiceDatabases::new();
    for text in [
        "fn main() { host::release(1); }",
        "fn main() { host::try_release(1); }",
        "extern state removed: i64; fn main() { return removed; }",
    ] {
        db.update(&assemble_project_sources(
            &config,
            &[SourceFileSnapshot::new(document.clone(), text)],
            &Workspace::new().snapshot(),
        ));
        assert!(
            db.diagnostics_for_document(&document)
                .diagnostics()
                .iter()
                .any(|d| d.severity() == crate::ServiceDiagnosticSeverity::Error)
        );
        let offset = if text.contains("extern state") {
            text.rfind("removed").expect("extern use")
        } else {
            text.find("release").expect("removed call")
        };
        let position = LineIndex::new(text).position(offset);
        assert!(db.hover(&document, position).is_none());
        assert!(db.definition(&document, position).is_none());
        assert!(db.prepare_rename(&document, position).is_none());
        let call_position = LineIndex::new(text).position(text.rfind('(').expect("call") + 1);
        assert!(db.signature_help(&document, call_position).is_none());
        assert!(
            db.code_actions(&document, DiagnosticRange::new(position, position))
                .iter()
                .all(|action| !action.title().contains("host::"))
        );
    }
    let text = "state count: i64 = 1; fn main() { return count; }";
    db.update(&assemble_project_sources(
        &config,
        &[SourceFileSnapshot::new(document.clone(), text)],
        &Workspace::new().snapshot(),
    ));
    assert!(
        db.diagnostics_for_document(&document)
            .diagnostics()
            .is_empty()
    );
    let hover = db
        .hover(
            &document,
            LineIndex::new(text).position(text.rfind("count").expect("state use")),
        )
        .expect("ordinary VM state");
    assert_eq!(hover.kind(), crate::HoverKind::VmState);
    assert!(hover.detail().contains("VM state; owned per Runtime"));
    let items = db.completion_items(&document, crate::Position::new(0, 0));
    assert!(
        items
            .items()
            .iter()
            .all(|item| item.label() != "extern state")
    );
}
