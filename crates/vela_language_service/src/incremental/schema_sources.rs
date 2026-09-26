//! Schema SourceIds belong to the source table at load time. Bind their document
//! owners once, then project spans into each current table without borrowing a
//! newly assigned numeric ID from another document.
use std::collections::BTreeMap;

use vela_common::SourceId;

use super::SourceRecord;
use crate::{DocumentId, SchemaSourceLocations};

#[derive(Debug, Clone, Default)]
pub(super) struct BoundSchemaLocations {
    artifact: SchemaSourceLocations,
    current: SchemaSourceLocations,
    owners: Option<BTreeMap<SourceId, DocumentId>>,
}

impl BoundSchemaLocations {
    pub const fn current(&self) -> &SchemaSourceLocations {
        &self.current
    }

    pub fn replace(&mut self, locations: SchemaSourceLocations) {
        // A project/config refresh can reread the same artifact after numeric
        // source IDs have changed. Keep its originally bound document owners.
        if self.artifact != locations {
            self.current = locations.clone();
            self.artifact = locations;
            self.owners = None;
        }
    }

    pub fn refresh(&mut self, sources: &BTreeMap<DocumentId, SourceRecord>) {
        if self.owners.is_none() && sources.is_empty() {
            // Initialization loads metadata before the first source snapshot.
            return;
        }
        let owners = self.owners.get_or_insert_with(|| {
            let ids = self.artifact.source_ids();
            sources
                .values()
                .filter(|source| ids.contains(&source.source_id()))
                .map(|source| (source.source_id(), source.document_id().clone()))
                .collect()
        });
        self.current = self.artifact.map_spans(|mut span| {
            let source = sources.get(owners.get(&span.source)?)?;
            span.source = source.source_id();
            Some(span)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SchemaArtifact;
    use serde_json::json;
    use vela_common::Span;
    use vela_package::{ModuleKey, ModulePath, PackageId};

    fn records(id: u32, document: &str) -> BTreeMap<DocumentId, SourceRecord> {
        let document_id = DocumentId::from(document);
        BTreeMap::from([(
            document_id.clone(),
            SourceRecord {
                document_id,
                source_id: SourceId::new(id),
                module_key: ModuleKey::new(
                    PackageId::anonymous(),
                    ModulePath::from_qualified("anchor"),
                ),
                text: "pub fn anchor() {}".into(),
                version: crate::SourceVersion::new(1),
                content_hash: 1,
            },
        )])
    }

    fn locations(id: u32) -> SchemaSourceLocations {
        let fact = json!({"kind":"primitive","name":"i64"});
        let span = json!({"source":id,"start":7,"end":13});
        let named = json!([{"name":"owner","fact":fact,"sourceSpan":span}]);
        let member = json!([{"owner":"owner","name":"member","fact":fact,"sourceSpan":span}]);
        SchemaArtifact::from_json(
            &json!({"formatVersion":1,"facts":{
                "types":named,"traits":named,"modules":named,"functions":named,
                "fields":member,"variants":member,"methods":member,"traitMethods":member
            }})
            .to_string(),
        )
        .expect("authored span categories")
        .source_locations()
    }

    fn assert_all(locations: &SchemaSourceLocations, id: u32) {
        let expected = Some(Span::new(SourceId::new(id), 7, 13));
        for span in [
            locations.type_span("owner"),
            locations.trait_span("owner"),
            locations.module_span("owner"),
            locations.function_span("owner"),
            locations.field_span("owner", "member"),
            locations.variant_span("owner", "member"),
            locations.method_span("owner", "member"),
            locations.trait_method_span("owner", "member"),
        ] {
            assert_eq!(
                span, expected,
                "preserve every schema span category and byte bounds"
            );
        }
    }

    #[test]
    fn schema_spans_keep_document_owners_across_source_table_replacement() {
        let original = locations(7);
        let mut bound = BoundSchemaLocations::default();
        bound.replace(original.clone());
        bound.refresh(&BTreeMap::new()); // artifact loaded before initial sources
        bound.refresh(&records(7, "/workspace/anchor.vela"));
        assert_all(bound.current(), 7);

        let mut moved = records(2, "/workspace/anchor.vela");
        moved.extend(records(7, "/workspace/decoy.vela"));
        bound.refresh(&moved);
        assert_all(bound.current(), 2); // never borrow decoy SourceId 7
        bound.replace(original); // unchanged artifact reread after config refresh
        bound.refresh(&moved);
        assert_all(bound.current(), 2);

        bound.refresh(&records(7, "/workspace/decoy.vela"));
        assert_eq!(
            bound.current(),
            &SchemaSourceLocations::default(),
            "deleted anchor has no spans"
        );
        bound.refresh(&BTreeMap::new());
        assert_eq!(bound.current(), &SchemaSourceLocations::default());
        bound.refresh(&records(9, "/workspace/anchor.vela"));
        assert_all(bound.current(), 9); // recreated owner receives a fresh ID

        bound.replace(locations(3)); // explicit replacement binds its new table
        bound.refresh(&records(3, "/workspace/new_anchor.vela"));
        bound.refresh(&records(5, "/workspace/new_anchor.vela"));
        assert_all(bound.current(), 5);
    }
}
