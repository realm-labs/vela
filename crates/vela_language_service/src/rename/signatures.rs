use super::{
    PrepareRename, TextEdit, WorkspaceEdit, diagnostic_range, local_collisions, shorthand,
    workspace_edit_for_rename,
};
use crate::{
    DocumentId, LanguageServiceDatabases, QueryContext,
    signature_parameters::{self, Target},
};
use std::collections::BTreeMap;

pub(super) fn prepare(
    db: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
) -> Option<PrepareRename> {
    let target = signature_parameters::target(db, query)?;
    Some(PrepareRename {
        document_id: query.document_id().clone(),
        range: diagnostic_range(query.text(), query.identifier_range()?),
        placeholder: target.parameter.name.clone(),
        symbol: target.symbol(db)?,
    })
}

pub(super) fn rename(
    db: &LanguageServiceDatabases,
    target: Target<'_>,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    if target
        .signature
        .params
        .iter()
        .any(|param| param.span != target.parameter.span && param.name == new_name)
        || signature_parameters::captures_label(db, target, new_name)
        || target.bindings(db).iter().any(|(bindings, local)| {
            local_collisions::conflicts(db.hir_db().graph(), bindings, *local, new_name)
        })
    {
        return None;
    }
    let mut edits = BTreeMap::<DocumentId, Vec<TextEdit>>::new();
    for site in signature_parameters::sites(db, target, true) {
        let source = db.source_db().records().get(&site.document)?;
        let labels = shorthand::local_labels(db.hir_db().graph(), source.source_id());
        edits.entry(site.document).or_default().push(TextEdit {
            range: diagnostic_range(source.text(), signature_parameters::range(site.span)),
            new_text: shorthand::local_replacement(&labels, site.span, new_name),
        });
    }
    workspace_edit_for_rename(db, edits, Vec::new())
        .map(|edit| edit.with_symbol(target.symbol(db).expect("source parameter")))
}
