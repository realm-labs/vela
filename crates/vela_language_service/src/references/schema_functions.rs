use super::{Reference, ReferenceKind, diagnostic_range, span_text_range};
use crate::{LanguageServiceDatabases, schema_function_sites, symbol_ref::schema_symbol};

pub(super) fn references(
    db: &LanguageServiceDatabases,
    name: &str,
    include: bool,
) -> Vec<Reference> {
    let mut result = Vec::new();
    for source in db.source_db().records().values() {
        if include
            && let Some(span) = db.schema_db().source_locations().function_span(name)
            && span.source == source.source_id()
            && let Some(range) = span_text_range(span)
        {
            result.push(Reference {
                document_id: source.document_id().clone(),
                range: diagnostic_range(source.text(), range),
                kind: ReferenceKind::Declaration,
                symbol: schema_symbol(name),
            });
        }
        result.extend(
            schema_function_sites::sites(db, source)
                .into_iter()
                .filter(|site| site.name == name)
                .map(|site| Reference {
                    document_id: source.document_id().clone(),
                    range: diagnostic_range(source.text(), site.range),
                    kind: if site.call {
                        ReferenceKind::Call
                    } else {
                        ReferenceKind::Read
                    },
                    symbol: schema_symbol(name),
                }),
        );
    }
    result.sort_by(|a, b| {
        a.document_id.cmp(&b.document_id).then_with(|| {
            (a.range.start().line, a.range.start().character)
                .cmp(&(b.range.start().line, b.range.start().character))
        })
    });
    result
}
