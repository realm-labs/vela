use crate::token::{Keyword, Symbol, TokenKind};

pub(crate) fn normalize_attribute_value(tokens: &[TokenKind]) -> String {
    match tokens {
        [TokenKind::String(value)] | [TokenKind::Ident(value)] => return value.clone(),
        [TokenKind::Int(value)] | [TokenKind::Float(value)] => return value.clone(),
        [TokenKind::Keyword(keyword)] => return keyword_text(*keyword).to_owned(),
        _ => {}
    }

    let mut normalized = String::new();
    for token in tokens {
        normalized.push_str(&attribute_token_text(token));
    }
    normalized
}

fn attribute_token_text(token: &TokenKind) -> String {
    match token {
        TokenKind::Ident(value) | TokenKind::Int(value) | TokenKind::Float(value) => value.clone(),
        TokenKind::String(value) => quoted_attribute_string(value),
        TokenKind::Keyword(keyword) => keyword_text(*keyword).to_owned(),
        TokenKind::Symbol(symbol) => symbol_text(*symbol).to_owned(),
        TokenKind::Eof => String::new(),
    }
}

fn quoted_attribute_string(value: &str) -> String {
    let mut quoted = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            '\0' => quoted.push_str("\\0"),
            _ => quoted.push(ch),
        }
    }
    quoted.push('"');
    quoted
}

fn keyword_text(keyword: Keyword) -> &'static str {
    match keyword {
        Keyword::Use => "use",
        Keyword::Pub => "pub",
        Keyword::Const => "const",
        Keyword::Let => "let",
        Keyword::Fn => "fn",
        Keyword::Struct => "struct",
        Keyword::Enum => "enum",
        Keyword::Trait => "trait",
        Keyword::Impl => "impl",
        Keyword::For => "for",
        Keyword::If => "if",
        Keyword::Else => "else",
        Keyword::Match => "match",
        Keyword::Return => "return",
        Keyword::Break => "break",
        Keyword::Continue => "continue",
        Keyword::True => "true",
        Keyword::False => "false",
        Keyword::Null => "null",
        Keyword::SelfValue => "self",
        Keyword::In => "in",
        Keyword::As => "as",
    }
}

fn symbol_text(symbol: Symbol) -> &'static str {
    match symbol {
        Symbol::Hash => "#",
        Symbol::LBracket => "[",
        Symbol::RBracket => "]",
        Symbol::LParen => "(",
        Symbol::RParen => ")",
        Symbol::LBrace => "{",
        Symbol::RBrace => "}",
        Symbol::Comma => ",",
        Symbol::Dot => ".",
        Symbol::DotDot => "..",
        Symbol::DotDotEqual => "..=",
        Symbol::Colon => ":",
        Symbol::Semicolon => ";",
        Symbol::Arrow => "->",
        Symbol::FatArrow => "=>",
        Symbol::Equal => "=",
        Symbol::PlusEqual => "+=",
        Symbol::MinusEqual => "-=",
        Symbol::StarEqual => "*=",
        Symbol::SlashEqual => "/=",
        Symbol::PercentEqual => "%=",
        Symbol::Plus => "+",
        Symbol::Minus => "-",
        Symbol::Star => "*",
        Symbol::Slash => "/",
        Symbol::Percent => "%",
        Symbol::Bang => "!",
        Symbol::BangEqual => "!=",
        Symbol::EqualEqual => "==",
        Symbol::Less => "<",
        Symbol::LessEqual => "<=",
        Symbol::Greater => ">",
        Symbol::GreaterEqual => ">=",
        Symbol::AndAnd => "&&",
        Symbol::OrOr => "||",
        Symbol::Pipe => "|",
        Symbol::Question => "?",
    }
}
