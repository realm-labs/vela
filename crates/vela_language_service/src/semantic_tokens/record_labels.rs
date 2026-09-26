//! Explicit record labels own their spans; shorthand keeps its local binding.
use std::collections::BTreeMap;

use vela_common::SourceId;

use crate::{LanguageServiceDatabases, schema_record_fields, source_record_fields};

use super::{
    SemanticTokenClassification as C, SemanticTokenModifiers as M, SemanticTokenType as T,
};

pub(super) fn collect(
    db: &LanguageServiceDatabases,
    source: SourceId,
) -> BTreeMap<(usize, usize), C> {
    let mut result = BTreeMap::new();
    let Some(source) = db
        .source_db()
        .records()
        .values()
        .find(|record| record.source_id() == source)
    else {
        return result;
    };
    for site in source_record_fields::record_sites(db, source)
        .into_iter()
        .filter(|site| !site.shorthand)
    {
        let known = site
            .owner
            .fields(db.hir_db().graph())
            .is_some_and(|fields| fields.iter().any(|field| field.name == site.name));
        result.insert(
            (site.range.start, site.range.end),
            C::new(
                if known { T::Property } else { T::Variable },
                if known { M::SOURCE } else { M::NONE },
            ),
        );
    }
    for site in schema_record_fields::sites(db, source)
        .into_iter()
        .filter(|site| !site.shorthand)
    {
        let known = db
            .schema_db()
            .facts()
            .field_fact(&site.owner, &site.name)
            .is_some();
        result.insert(
            (site.range.start, site.range.end),
            C::new(
                if known { T::Property } else { T::Variable },
                if known {
                    M::HOST.union(M::SCHEMA)
                } else {
                    M::NONE
                },
            ),
        );
    }
    result
}
