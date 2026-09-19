use vela_syntax::ast::{AstNode, SyntaxRecordPattern};

use super::model::RecordConstructor;
use crate::QueryContext;

pub(super) fn record_pattern_at(query: &QueryContext<'_>) -> Option<RecordConstructor> {
    let offset = query.cursor().replace_range().end;
    let parse = query.syntax_parse()?;
    let record = parse
        .tree()
        .syntax()
        .descendants()
        .filter_map(SyntaxRecordPattern::cast)
        .filter(|record| {
            let start = record
                .l_brace_token()
                .map(|token| usize::from(token.text_range().end()));
            let end = record
                .r_brace_token()
                .map_or(usize::from(record.syntax().text_range().end()), |token| {
                    usize::from(token.text_range().start())
                });
            start.is_some_and(|start| start <= offset && offset <= end)
        })
        .min_by_key(|record| record.syntax().text_range().len())?;
    let mut field_names = Vec::new();
    for field in record.fields() {
        let range = field.syntax().text_range();
        if usize::from(range.start()) <= offset && offset <= usize::from(range.end()) {
            let label = field.label_token()?.text_range();
            // A value pattern after ':' must not receive field-name candidates.
            if !(usize::from(label.start()) <= offset && offset <= usize::from(label.end())) {
                return None;
            }
        } else if let Some(name) = field.label_text() {
            field_names.push(name);
        }
    }
    Some(RecordConstructor {
        path: query
            .expand_import_path(&record.path_segments())
            .unwrap_or_default(),
        field_names,
        current_module: query.module_key().cloned(),
    })
}
