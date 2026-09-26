//! Explicit record labels own their spans; shorthand keeps its local binding.
use std::collections::BTreeMap;

use vela_common::SourceId;
use vela_syntax::ast::{AstNode, SyntaxRecordExpr};

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
    if let Some(parsed) = db.parse_db().syntax_parse(source.document_id()) {
        for record in parsed
            .tree()
            .syntax()
            .descendants()
            .filter_map(SyntaxRecordExpr::cast)
        {
            for label in record
                .fields()
                .into_iter()
                .filter(|field| !field.is_shorthand())
                .filter_map(|field| field.label_token())
            {
                result.insert(
                    (
                        usize::from(label.text_range().start()),
                        usize::from(label.text_range().end()),
                    ),
                    C::new(T::Variable, M::NONE),
                );
            }
        }
    }
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
        // Builtin enum contracts have unit/tuple payloads; schema collisions
        // cannot manufacture record fields on those exact owners.
        if site.owner.rsplit_once("::").is_some_and(|(owner, _)| {
            vela_analysis::stdlib::stdlib_enum_variants(owner)
                .next()
                .is_some()
        }) {
            continue;
        }
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
