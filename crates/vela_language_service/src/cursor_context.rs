use vela_syntax::ast::{SourceFile, Stmt, StmtKind};

use crate::{LineIndex, Position, TextRange};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CursorContextKind {
    Item,
    Statement,
    Expression,
    Pattern,
    Type,
    UseImport,
    ModulePath,
    MemberAccess,
    RecordExpressionField,
    RecordTypeField,
    CallArgument,
    LambdaParameter,
    MapKey,
    RenameTarget,
    Unknown,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CursorContext {
    kind: CursorContextKind,
    prefix: String,
    replace_range: TextRange,
    module_base: Option<String>,
    member_receiver: Option<TextRange>,
}

impl CursorContext {
    #[must_use]
    pub const fn kind(&self) -> CursorContextKind {
        self.kind
    }

    #[must_use]
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    #[must_use]
    pub const fn replace_range(&self) -> TextRange {
        self.replace_range
    }

    #[must_use]
    pub fn module_base(&self) -> Option<&str> {
        self.module_base.as_deref()
    }

    #[must_use]
    pub const fn member_receiver(&self) -> Option<TextRange> {
        self.member_receiver
    }
}

#[must_use]
pub fn cursor_context_at(
    text: &str,
    position: Position,
    parsed: Option<&SourceFile>,
) -> CursorContext {
    let offset = LineIndex::new(text).offset(position);
    let prefix_start = identifier_prefix_start(text, offset);
    let prefix = text[prefix_start..offset].to_owned();
    let before_prefix = &text[..prefix_start];

    if is_lambda_parameter_context(text, offset) {
        return context(CursorContextKind::LambdaParameter, prefix_start, prefix);
    }

    if is_type_context(text, prefix_start) {
        return context(CursorContextKind::Type, prefix_start, prefix);
    }

    if before_prefix.ends_with('.') {
        let mut cursor = context(CursorContextKind::MemberAccess, prefix_start, prefix);
        cursor.member_receiver = member_receiver_before_dot(before_prefix);
        return cursor;
    }

    if let Some(module_base) = module_base_before_colons(before_prefix) {
        let mut cursor = context(CursorContextKind::ModulePath, prefix_start, prefix);
        cursor.module_base = Some(module_base);
        return cursor;
    }

    if is_use_import_context(text, prefix_start) {
        return context(CursorContextKind::UseImport, prefix_start, prefix);
    }

    if is_item_boundary_context(text, prefix_start, parsed) {
        return context(CursorContextKind::Item, prefix_start, prefix);
    }

    if is_call_argument_context(text, offset) {
        return context(CursorContextKind::CallArgument, prefix_start, prefix);
    }

    if prefix.is_empty() && is_statement_context(parsed, prefix_start) {
        return context(CursorContextKind::Statement, prefix_start, prefix);
    }

    if parsed.is_some_and(|source| offset_is_inside_item(source, prefix_start)) {
        return context(CursorContextKind::Expression, prefix_start, prefix);
    }

    context(CursorContextKind::Unknown, prefix_start, prefix)
}

fn context(kind: CursorContextKind, prefix_start: usize, prefix: String) -> CursorContext {
    CursorContext {
        kind,
        replace_range: TextRange::new(prefix_start, prefix_start + prefix.len()),
        prefix,
        module_base: None,
        member_receiver: None,
    }
}

fn is_type_context(text: &str, prefix_start: usize) -> bool {
    let Some(before_prefix) = text.get(..prefix_start) else {
        return false;
    };
    let trimmed = before_prefix.trim_end();
    if trimmed.ends_with("::") {
        return false;
    }
    trimmed.ends_with("->")
        || (trimmed.ends_with(':')
            && !trimmed.ends_with("::")
            && type_annotation_left_side_is_plausible(trimmed))
        || inside_builtin_type_args(trimmed)
}

fn type_annotation_left_side_is_plausible(trimmed: &str) -> bool {
    let before_colon = trimmed.trim_end_matches(':').trim_end();
    let Some(last) = before_colon.char_indices().rev().find_map(|(index, ch)| {
        (!is_identifier_continue(ch)).then_some(&before_colon[index + 1..])
    }) else {
        return false;
    };
    !matches!(last, "case" | "default")
}

fn inside_builtin_type_args(trimmed: &str) -> bool {
    let Some(open) = trimmed.rfind('<') else {
        return false;
    };
    if trimmed[open + 1..].contains('>') {
        return false;
    }
    let before_open = trimmed[..open].trim_end();
    let start = before_open
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    matches!(
        &before_open[start..],
        "Array" | "Set" | "Map" | "Iterator" | "Option" | "Result"
    )
}

fn is_use_import_context(text: &str, prefix_start: usize) -> bool {
    current_line_before(text, prefix_start)
        .trim_start()
        .starts_with("use ")
}

fn is_item_boundary_context(text: &str, prefix_start: usize, parsed: Option<&SourceFile>) -> bool {
    if parsed.is_some_and(|source| offset_is_inside_item(source, prefix_start)) {
        return false;
    }
    let before_prefix = text[..prefix_start].trim_end();
    before_prefix.is_empty()
        || before_prefix.ends_with('}')
        || before_prefix.ends_with(';')
        || current_line_before(text, prefix_start)
            .trim_start()
            .trim_end()
            == "pub"
}

fn is_call_argument_context(text: &str, offset: usize) -> bool {
    active_call_open(text, offset).is_some()
}

fn is_lambda_parameter_context(text: &str, offset: usize) -> bool {
    let Some(before) = text.get(..offset) else {
        return false;
    };
    let Some(pipe) = before.rfind('|') else {
        return false;
    };
    let params = &before[pipe + 1..];
    is_lambda_parameter_prefix(params) && active_call_open(before, pipe).is_some()
}

fn is_lambda_parameter_prefix(params: &str) -> bool {
    params
        .chars()
        .all(|ch| ch.is_whitespace() || ch == ',' || is_identifier_continue(ch))
}

fn is_statement_context(parsed: Option<&SourceFile>, prefix_start: usize) -> bool {
    let Some(offset) = u32::try_from(prefix_start).ok() else {
        return false;
    };
    parsed.is_some_and(|source| {
        source.items.iter().any(|item| {
            if !item.span.contains(offset) {
                return false;
            }
            match &item.kind {
                vela_syntax::ast::ItemKind::Function(function) => {
                    function.body.statements.iter().any(|statement| {
                        statement.span.contains(offset) && is_statement_start(statement, offset)
                    })
                }
                _ => false,
            }
        })
    })
}

fn is_statement_start(statement: &Stmt, offset: u32) -> bool {
    match &statement.kind {
        StmtKind::Let { .. } => offset <= statement.span.start.saturating_add(4),
        StmtKind::Return(_) | StmtKind::Break | StmtKind::Continue => true,
        StmtKind::Expr(expr) => offset <= expr.span.start.saturating_add(1),
        StmtKind::For { .. } | StmtKind::Block(_) => {
            offset <= statement.span.start.saturating_add(1)
        }
    }
}

fn active_call_open(text: &str, offset: usize) -> Option<usize> {
    let mut stack = Vec::new();
    for (index, ch) in text[..offset].char_indices() {
        match ch {
            '(' => stack.push(index),
            ')' => {
                stack.pop();
            }
            _ => {}
        }
    }
    stack.pop()
}

fn offset_is_inside_item(source: &SourceFile, offset: usize) -> bool {
    let Some(offset) = u32::try_from(offset).ok() else {
        return false;
    };
    source.items.iter().any(|item| item.span.contains(offset))
}

fn identifier_prefix_start(text: &str, offset: usize) -> usize {
    text[..offset]
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0)
}

fn module_base_before_colons(before_prefix: &str) -> Option<String> {
    let before_colons = before_prefix.strip_suffix("::")?;
    let start = before_colons
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_module_path_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    let module_base = before_colons[start..].trim_matches(':');
    (!module_base.is_empty()).then(|| module_base.to_owned())
}

fn member_receiver_before_dot(before_prefix: &str) -> Option<TextRange> {
    let before_dot = before_prefix.strip_suffix('.')?;
    let end = before_dot.len();
    let start = before_dot
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    (start < end).then(|| TextRange::new(start, end))
}

fn current_line_before(text: &str, offset: usize) -> &str {
    text[..offset].rsplit('\n').next().unwrap_or_default()
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn is_module_path_continue(ch: char) -> bool {
    is_identifier_continue(ch) || ch == ':'
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_common::SourceId;
    use vela_syntax::parser::parse_source;

    fn classify(text: &str, needle: &str) -> CursorContext {
        let offset = text.find(needle).expect("needle should exist") + needle.len();
        let parsed = parse_source(SourceId::new(1), text);
        cursor_context_at(text, LineIndex::new(text).position(offset), Some(&parsed))
    }

    fn classify_offset(text: &str, offset: usize) -> CursorContext {
        let parsed = parse_source(SourceId::new(1), text);
        cursor_context_at(text, LineIndex::new(text).position(offset), Some(&parsed))
    }

    #[test]
    fn cursor_context_classifies_item_boundary_keywords() {
        let cursor = classify("f", "f");

        assert_eq!(cursor.kind(), CursorContextKind::Item);
        assert_eq!(cursor.prefix(), "f");
    }

    #[test]
    fn cursor_context_classifies_type_hints() {
        let cursor = classify("pub fn main(player: Pl) { return 1 }", "Pl");

        assert_eq!(cursor.kind(), CursorContextKind::Type);
    }

    #[test]
    fn cursor_context_classifies_member_access() {
        let cursor = classify("pub fn main(player) { player.le }", "le");

        assert_eq!(cursor.kind(), CursorContextKind::MemberAccess);
        assert_eq!(cursor.member_receiver(), Some(TextRange::new(22, 28)));
    }

    #[test]
    fn cursor_context_classifies_module_path() {
        let cursor = classify("use game::r", "r");

        assert_eq!(cursor.kind(), CursorContextKind::ModulePath);
        assert_eq!(cursor.module_base(), Some("game"));
    }

    #[test]
    fn cursor_context_classifies_call_arguments() {
        let cursor = classify("pub fn main() { grant(am) }", "am");

        assert_eq!(cursor.kind(), CursorContextKind::CallArgument);
    }

    #[test]
    fn cursor_context_classifies_statement_boundary() {
        let text = "pub fn main() { return 1 }";
        let cursor = classify_offset(text, text.find("return").expect("return should exist"));

        assert_eq!(cursor.kind(), CursorContextKind::Statement);
        assert_eq!(cursor.prefix(), "");
    }

    #[test]
    fn cursor_context_classifies_expression_position() {
        let cursor = classify("pub fn main() { Pla }", "Pla");

        assert_eq!(cursor.kind(), CursorContextKind::Expression);
    }
}
