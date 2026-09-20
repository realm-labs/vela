use crate::{LanguageServiceDatabases, LineIndex, QueryContext, SourceRecord, TextRange};
use vela_hir::module_graph::DeclarationKind;
use vela_syntax::ast::{AstNode, SyntaxRecordExpr, SyntaxRecordPattern};
pub(crate) struct Site {
    pub(crate) owner: String,
    pub(crate) name: String,
    pub(crate) range: TextRange,
    pub(crate) shorthand: bool,
    pub(crate) pattern: bool,
}

pub(crate) fn sites(db: &LanguageServiceDatabases, source: &SourceRecord) -> Vec<Site> {
    collect(db, source, None)
}

pub(crate) fn explicit_target(
    db: &LanguageServiceDatabases,
    source: &SourceRecord,
    range: TextRange,
) -> Option<Site> {
    collect(db, source, Some(range)).into_iter().next()
}

fn collect(
    db: &LanguageServiceDatabases,
    source: &SourceRecord,
    at: Option<TextRange>,
) -> Vec<Site> {
    let Some(parsed) = db.parse_db().syntax_parse(source.document_id()) else {
        return Vec::new();
    };
    let lines = LineIndex::new(source.text());
    let graph = db.hir_db().graph();
    let mut result = Vec::new();
    for node in parsed.tree().syntax().descendants() {
        let (path, mut fields, pattern) = if let Some(record) = SyntaxRecordExpr::cast(node.clone())
        {
            (
                record.path_segments(),
                record
                    .fields()
                    .into_iter()
                    .filter_map(|field| Some((field.label_token()?, field.is_shorthand())))
                    .collect::<Vec<_>>(),
                false,
            )
        } else if let Some(pattern) = SyntaxRecordPattern::cast(node) {
            (
                pattern.path_segments(),
                pattern
                    .fields()
                    .filter_map(|field| Some((field.label_token()?, field.is_shorthand())))
                    .collect(),
                true,
            )
        } else {
            continue;
        };
        if let Some(range) = at {
            fields.retain(|(label, shorthand)| {
                !shorthand
                    && usize::from(label.text_range().start()) == range.start
                    && usize::from(label.text_range().end()) == range.end
            });
        }
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
            pattern,
        }));
    }
    result
}
