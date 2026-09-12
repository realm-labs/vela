use vela_syntax::ast::{AstNode, SyntaxCallExpr};

use crate::QueryContext;

use super::{CompletionInsertFormat, CompletionItem};

pub(super) fn argument_call(query: &QueryContext<'_>) -> Option<SyntaxCallExpr> {
    let offset = query.cursor().replace_range().end;
    query
        .syntax_parse()?
        .tree()
        .syntax()
        .descendants()
        .filter_map(SyntaxCallExpr::cast)
        .filter(|call| {
            call.l_paren_token()
                .is_some_and(|open| usize::from(open.text_range().end()) <= offset)
                && offset <= usize::from(call.syntax().text_range().end())
        })
        .min_by_key(|call| call.syntax().text_range().len())
}

pub(super) fn set_path_insertion(
    item: &mut CompletionItem,
    query: &QueryContext<'_>,
    path: String,
) {
    item.insert_text = Some(path.clone());
    item.insert_format = CompletionInsertFormat::PlainText;
    if let Some(edit) = &mut item.metadata.text_edit {
        edit.new_text = path;
        if let Some(range) = query.cursor().identifier_range() {
            edit.range = range;
            item.metadata.edit_range = Some(range);
        }
    }
}
