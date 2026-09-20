use super::{
    RenameToken, TextEdit, diagnostic_range,
    schema::{SchemaMemberRenameKind, SchemaMemberRenameTarget},
};
use crate::{DocumentId, LanguageServiceDatabases, QueryContext, schema_record_fields::sites};
use std::collections::BTreeMap;
pub(super) fn target(
    db: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    token: &RenameToken,
) -> Option<Option<SchemaMemberRenameTarget>> {
    let site =
        crate::schema_record_fields::explicit_target(db, query.source_record()?, token.range)?;
    let writable = db
        .schema_db()
        .source_locations()
        .field_span(&site.owner, &site.name)
        .is_some();
    Some(writable.then_some(SchemaMemberRenameTarget {
        owner: site.owner,
        member: site.name,
        kind: SchemaMemberRenameKind::Field,
        token: token.clone(),
    }))
}

pub(super) fn append_edits(
    db: &LanguageServiceDatabases,
    target: &SchemaMemberRenameTarget,
    name: &str,
    edits: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) -> Option<()> {
    if target.kind != SchemaMemberRenameKind::Field {
        return Some(());
    }
    for source in db.source_db().records().values() {
        for site in sites(db, source)
            .into_iter()
            .filter(|site| site.owner == target.owner)
        {
            if name != target.member && site.name == name {
                return None;
            }
            if site.name != target.member {
                continue;
            }
            let new_text = if site.shorthand && name != site.name {
                format!("{name}: {}", site.name)
            } else {
                name.to_owned()
            };
            edits
                .entry(source.document_id().clone())
                .or_default()
                .push(TextEdit {
                    range: diagnostic_range(source.text(), site.range),
                    new_text,
                });
        }
    }
    Some(())
}
