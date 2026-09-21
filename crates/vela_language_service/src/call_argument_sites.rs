use crate::{
    DocumentId, LanguageServiceDatabases, LineIndex, TextRange, definition::SourceParameters,
};
use vela_syntax::{
    SyntaxKind,
    ast::{AstNode, SyntaxCallExpr},
};

pub(crate) struct ArgumentLabel {
    pub(crate) name: String,
    pub(crate) range: TextRange,
}

pub(crate) struct ResolvedCall<'a> {
    pub(crate) parameters: SourceParameters<'a>,
    pub(crate) labels: Vec<ArgumentLabel>,
}

pub(crate) fn in_document<'a>(
    databases: &'a LanguageServiceDatabases,
    document: &DocumentId,
    at: Option<TextRange>,
    names: Option<&[&str]>,
) -> Vec<ResolvedCall<'a>> {
    let Some(parsed) = databases.parse_db().syntax_parse(document) else {
        return Vec::new();
    };
    let Some(source) = databases.source_db().records().get(document) else {
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
            .filter_map(|label| {
                let range = TextRange::new(
                    usize::from(label.text_range().start()),
                    usize::from(label.text_range().end()),
                );
                (at.is_none_or(|at| at == range)
                    && names.is_none_or(|names| names.contains(&label.text())))
                .then(|| ArgumentLabel {
                    name: label.text().to_owned(),
                    range,
                })
            })
            .collect();
        if labels.is_empty() {
            continue;
        }
        let Some(expression) = call.callee() else {
            continue;
        };
        let Some(name) = expression
            .as_path()
            .and_then(|path| path.path_tokens().last().cloned())
            .filter(|token| token.kind() == SyntaxKind::Ident)
            .or_else(|| expression.as_field().and_then(|field| field.name_token()))
        else {
            continue;
        };
        let Some(callee) = databases.definition(
            document,
            lines.position(usize::from(name.text_range().start())),
        ) else {
            continue;
        };
        let Some(parameters) = databases.source_parameters_for_navigation(&callee) else {
            continue;
        };
        result.push(ResolvedCall { parameters, labels });
    }
    result
}
