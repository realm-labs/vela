use vela_common::SourceId;
use vela_syntax::ast::{EnumVariantFields, ItemKind, SourceFile, StructField};
use vela_syntax::lexer::lex;
use vela_syntax::token::{Keyword, Symbol, Token, TokenKind};

use crate::TextRange;

use super::{is_type_context, span_contains_usize};

pub(super) fn is_record_type_field_context(text: &str, source: &SourceFile, offset: usize) -> bool {
    let Some(offset_u32) = u32::try_from(offset).ok() else {
        return false;
    };
    let offset = usize::try_from(offset_u32).unwrap_or_default();
    if is_type_context(text, offset) {
        return false;
    }
    if source.items.iter().any(|item| match &item.kind {
        ItemKind::Struct(item) => item
            .fields
            .iter()
            .any(|field| field_name_contains(text, field, offset_u32)),
        ItemKind::Enum(item) => item.variants.iter().any(|variant| match &variant.fields {
            EnumVariantFields::Record(fields) => fields
                .iter()
                .any(|field| field_name_contains(text, field, offset_u32)),
            EnumVariantFields::Unit | EnumVariantFields::Tuple(_) => false,
        }),
        _ => false,
    }) {
        return true;
    }
    if source.items.iter().any(|item| match &item.kind {
        ItemKind::Struct(item) => item
            .fields
            .iter()
            .any(|field| span_contains_usize(field.span, offset)),
        ItemKind::Enum(item) => item.variants.iter().any(|variant| match &variant.fields {
            EnumVariantFields::Record(fields) => fields
                .iter()
                .any(|field| span_contains_usize(field.span, offset)),
            EnumVariantFields::Unit | EnumVariantFields::Tuple(_) => false,
        }),
        _ => false,
    }) {
        return false;
    }
    is_struct_item_body_context(text, offset)
}

fn field_name_contains(text: &str, field: &StructField, offset: u32) -> bool {
    let Some(range) = field_name_range(text, field) else {
        return false;
    };
    let Some(offset) = usize::try_from(offset).ok() else {
        return false;
    };
    range.start <= offset && offset <= range.end
}

fn field_name_range(text: &str, field: &StructField) -> Option<TextRange> {
    let start = usize::try_from(field.span.start).ok()?;
    let end = usize::try_from(field.span.end).ok()?;
    let field_text = text.get(start..end)?;
    let name_start = field_text.find(&field.name)?;
    let start = start + name_start;
    Some(TextRange::new(start, start + field.name.len()))
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
