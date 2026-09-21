use vela_syntax::{
    SyntaxKind,
    ast::{AstNode, SyntaxCallExpr},
};

use super::{FieldSite, RecordOwner};
use crate::{LanguageServiceDatabases, LineIndex, SourceRecord, TextRange};

pub(super) fn sites(
    databases: &LanguageServiceDatabases,
    source: &SourceRecord,
    at: Option<TextRange>,
) -> Vec<FieldSite> {
    let Some(parsed) = databases.parse_db().syntax_parse(source.document_id()) else {
        return Vec::new();
    };
    let lines = LineIndex::new(source.text());
    let mut result = Vec::new();
    for call in parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(SyntaxCallExpr::cast)
    {
        let labels: Vec<_> = call
            .arguments()
            .iter()
            .filter_map(|argument| argument.name_token())
            .filter(|label| {
                at.is_none_or(|range| {
                    usize::from(label.text_range().start()) == range.start
                        && usize::from(label.text_range().end()) == range.end
                })
            })
            .collect();
        if labels.is_empty() {
            continue;
        }
        let Some(name) = call
            .callee()
            .and_then(|callee| callee.as_path())
            .and_then(|path| path.path_tokens().last().cloned())
            .filter(|token| token.kind() == SyntaxKind::Ident)
        else {
            continue;
        };
        let Some(callee) = databases.definition(
            source.document_id(),
            lines.position(usize::from(name.text_range().start())),
        ) else {
            continue;
        };
        let Some(parameters) = databases.source_parameters_for_navigation(&callee) else {
            continue;
        };
        let Some((declaration, variant)) = parameters.variant else {
            continue;
        };
        let owner = RecordOwner {
            declaration,
            variant: Some(variant.to_owned()),
            tuple: true,
        };
        result.extend(labels.into_iter().map(|label| FieldSite {
            owner: owner.clone(),
            name: label.text().to_owned(),
            range: TextRange::new(
                usize::from(label.text_range().start()),
                usize::from(label.text_range().end()),
            ),
            shorthand: false,
            pattern: false,
        }));
    }
    result
}
