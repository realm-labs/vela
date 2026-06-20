use vela_common::SourceId;
use vela_syntax::ast::{AstNode, SyntaxRecordFieldList, SyntaxSourceFile, SyntaxStructFieldList};
use vela_syntax::lexer::lex;
use vela_syntax::token::{Keyword, Symbol, Token, TokenKind};
use vela_syntax::{TextRange as SyntaxTextRange, TextSize};

use super::is_type_context;

pub(super) fn is_record_type_field_context(
    text: &str,
    source: &SyntaxSourceFile,
    offset: usize,
) -> bool {
    let Some(offset_size) = syntax_offset(offset) else {
        return false;
    };
    if is_type_context(text, offset) {
        return false;
    }
    let field_lists = record_field_lists(source);
    if field_lists
        .iter()
        .any(|list| field_list_name_contains(list, offset_size))
    {
        return true;
    }
    if field_lists
        .iter()
        .any(|list| field_list_field_contains(list, offset_size))
    {
        return false;
    }
    if field_lists
        .iter()
        .any(|list| range_contains_offset(list.text_range(), offset_size))
    {
        return true;
    }
    is_struct_item_body_context(text, offset)
}

#[derive(Clone, Debug)]
enum RecordFieldList {
    Struct(SyntaxStructFieldList),
    EnumVariant(SyntaxRecordFieldList),
}

impl RecordFieldList {
    fn text_range(&self) -> SyntaxTextRange {
        match self {
            RecordFieldList::Struct(list) => list.syntax().text_range(),
            RecordFieldList::EnumVariant(list) => list.syntax().text_range(),
        }
    }

    fn field_name_ranges(&self) -> Vec<SyntaxTextRange> {
        match self {
            RecordFieldList::Struct(list) => list
                .fields()
                .filter_map(|field| field.name_token().map(|token| token.text_range()))
                .collect(),
            RecordFieldList::EnumVariant(list) => list
                .fields()
                .filter_map(|field| field.name_token().map(|token| token.text_range()))
                .collect(),
        }
    }

    fn field_ranges(&self) -> Vec<SyntaxTextRange> {
        match self {
            RecordFieldList::Struct(list) => list
                .fields()
                .map(|field| field.syntax().text_range())
                .collect(),
            RecordFieldList::EnumVariant(list) => list
                .fields()
                .map(|field| field.syntax().text_range())
                .collect(),
        }
    }
}

fn record_field_lists(source: &SyntaxSourceFile) -> Vec<RecordFieldList> {
    source
        .structs()
        .filter_map(|item| item.field_list().map(RecordFieldList::Struct))
        .chain(source.enums().flat_map(|item| {
            item.variant_list().into_iter().flat_map(|list| {
                list.variants()
                    .filter_map(|variant| {
                        variant
                            .record_field_list()
                            .map(RecordFieldList::EnumVariant)
                    })
                    .collect::<Vec<_>>()
            })
        }))
        .collect()
}

fn field_list_name_contains(list: &RecordFieldList, offset: TextSize) -> bool {
    list.field_name_ranges()
        .into_iter()
        .any(|range| range_contains_offset(range, offset))
}

fn field_list_field_contains(list: &RecordFieldList, offset: TextSize) -> bool {
    list.field_ranges()
        .into_iter()
        .any(|range| range_contains_offset(range, offset))
}

fn range_contains_offset(range: SyntaxTextRange, offset: TextSize) -> bool {
    range.start() <= offset && offset <= range.end()
}

fn syntax_offset(offset: usize) -> Option<TextSize> {
    let offset = u32::try_from(offset).ok()?;
    Some(TextSize::from(offset))
}

fn is_struct_item_body_context(text: &str, offset: usize) -> bool {
    let Some(offset) = u32::try_from(offset).ok() else {
        return false;
    };
    let lexed = lex(SourceId::new(0), text);
    let mut tokens = Vec::new();
    for token in lexed.tokens {
        if matches!(token.kind, TokenKind::Eof) {
            break;
        }
        if token.span.start > offset {
            break;
        }
        tokens.push(token);
    }
    let Some(open_index) = active_open_brace_before_offset(&tokens, offset) else {
        return false;
    };
    tokens.get(open_index).is_some_and(|token| {
        matches!(token.kind, TokenKind::Symbol(Symbol::LBrace))
            && open_index >= 2
            && matches!(tokens[open_index - 1].kind, TokenKind::Ident(_))
            && matches!(
                tokens[open_index - 2].kind,
                TokenKind::Keyword(Keyword::Struct)
            )
    })
}

fn active_open_brace_before_offset(tokens: &[Token], offset: u32) -> Option<usize> {
    let mut stack = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.span.start > offset {
            break;
        }
        match token.kind {
            TokenKind::Symbol(Symbol::LBrace) if token.span.start < offset => stack.push(index),
            TokenKind::Symbol(Symbol::RBrace) if token.span.start <= offset => {
                stack.pop();
            }
            _ => {}
        }
    }
    stack.pop()
}
