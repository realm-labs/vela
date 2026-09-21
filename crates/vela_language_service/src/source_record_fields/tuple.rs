use super::{FieldSite, RecordOwner};
use crate::{LanguageServiceDatabases, SourceRecord, TextRange};

pub(super) fn sites(
    databases: &LanguageServiceDatabases,
    source: &SourceRecord,
    at: Option<TextRange>,
) -> Vec<FieldSite> {
    let mut result = Vec::new();
    for call in crate::call_argument_sites::in_document(databases, source.document_id(), at, None) {
        let parameters = call.parameters;
        let Some((declaration, variant)) = parameters.variant else {
            continue;
        };
        let owner = RecordOwner {
            declaration,
            variant: Some(variant.to_owned()),
            tuple: true,
        };
        result.extend(call.labels.into_iter().map(|label| FieldSite {
            owner: owner.clone(),
            name: label.name,
            range: label.range,
            shorthand: false,
            pattern: false,
        }));
    }
    result
}
