use std::collections::BTreeMap;

use vela_hir::module_graph::DeclarationKind;
use vela_syntax::ast::{AstNode, SyntaxRecordExpr, SyntaxRecordPattern};

use super::{
    RenameToken, TextEdit, diagnostic_range,
    schema::{SchemaMemberRenameKind, SchemaMemberRenameTarget},
};
use crate::{
    DocumentId, LanguageServiceDatabases, LineIndex, QueryContext, SourceRecord, TextRange,
};

struct Site {
    owner: String,
    name: String,
    range: TextRange,
    shorthand: bool,
}

fn sites(db: &LanguageServiceDatabases, source: &SourceRecord) -> Vec<Site> {
    let Some(parsed) = db.parse_db().syntax_parse(source.document_id()) else {
        return Vec::new();
    };
    let lines = LineIndex::new(source.text());
    let graph = db.hir_db().graph();
    let mut result = Vec::new();
    for node in parsed.tree().syntax().descendants() {
        let (path, fields) = if let Some(record) = SyntaxRecordExpr::cast(node.clone()) {
            (
                record.path_segments(),
                record
                    .fields()
                    .into_iter()
                    .filter_map(|field| Some((field.label_token()?, field.is_shorthand())))
                    .collect::<Vec<_>>(),
            )
        } else if let Some(pattern) = SyntaxRecordPattern::cast(node) {
            (
                pattern.path_segments(),
                pattern
                    .fields()
                    .filter_map(|field| Some((field.label_token()?, field.is_shorthand())))
                    .collect(),
            )
        } else {
            continue;
        };
        let Some((first, _)) = fields.first() else {
            continue;
        };
        let Some(query) = QueryContext::from_databases(
            db,
            source.document_id(),
            lines.position(usize::from(first.text_range().start())),
        ) else {
            continue;
        };
        let Some(module) = query.module_key().and_then(|key| graph.module_id(key)) else {
            continue;
        };
        let Some(path) = query.expand_import_path(&path) else {
            continue;
        };
        let Some((_, parent)) = path.split_last() else {
            continue;
        };
        if [
            DeclarationKind::Struct,
            DeclarationKind::Enum,
            DeclarationKind::Trait,
            DeclarationKind::Function,
            DeclarationKind::Const,
            DeclarationKind::State,
        ]
        .into_iter()
        .any(|kind| {
            graph
                .resolve_visible_declaration_path(module, &path, kind)
                .is_some()
                || graph
                    .resolve_visible_declaration_path(module, parent, kind)
                    .is_some()
        }) {
            continue;
        }
        let owner = path.join("::");
        if !db
            .schema_db()
            .facts()
            .fields()
            .any(|candidate| candidate.owner == owner)
        {
            continue;
        }
        result.extend(fields.into_iter().map(|(label, shorthand)| Site {
            owner: owner.clone(),
            name: label.text().to_owned(),
            range: TextRange::new(
                usize::from(label.text_range().start()),
                usize::from(label.text_range().end()),
            ),
            shorthand,
        }));
    }
    result
}

pub(super) fn target(
    db: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    token: &RenameToken,
) -> Option<Option<SchemaMemberRenameTarget>> {
    let site = sites(db, query.source_record()?)
        .into_iter()
        .find(|site| !site.shorthand && site.range == token.range)?;
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
