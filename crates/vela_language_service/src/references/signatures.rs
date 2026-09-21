use super::{Reference, ReferenceKind, diagnostic_range, resolved_use_reference_kind};
use crate::{
    LanguageServiceDatabases,
    signature_parameters::{self, SiteKind, Target},
};

pub(super) fn references(
    db: &LanguageServiceDatabases,
    target: Target<'_>,
    include_declaration: bool,
) -> Vec<Reference> {
    let Some(symbol) = target.symbol(db) else {
        return Vec::new();
    };
    signature_parameters::sites(db, target, include_declaration)
        .into_iter()
        .filter_map(|site| {
            let source = db.source_db().records().get(&site.document)?;
            let range = signature_parameters::range(site.span);
            Some(Reference {
                document_id: site.document,
                range: diagnostic_range(source.text(), range),
                kind: match site.kind {
                    SiteKind::Declaration => ReferenceKind::Declaration,
                    SiteKind::Label => ReferenceKind::Read,
                    SiteKind::Body => resolved_use_reference_kind(source.text(), range),
                },
                symbol: symbol.clone(),
            })
        })
        .collect()
}
