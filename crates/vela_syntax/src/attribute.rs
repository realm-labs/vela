use crate::token::{InterpolatedStringTokenPart, Keyword, Symbol, TokenKind};

pub(crate) fn normalize_attribute_value(tokens: &[TokenKind]) -> String {
    match tokens {
        [TokenKind::String(value)] | [TokenKind::Ident(value)] => return value.clone(),
        [TokenKind::Bytes(value)] => return quoted_attribute_bytes(value),
        [TokenKind::Int(value)] => return value.source_text_with_suffix(),
        [TokenKind::Float(value)] => return value.source_text_with_suffix(),
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
        TokenKind::Ident(value) => value.clone(),
        TokenKind::Int(value) => value.source_text_with_suffix(),
        TokenKind::Float(value) => value.source_text_with_suffix(),
        TokenKind::String(value) => quoted_attribute_string(value),
        TokenKind::InterpolatedString(parts) => quoted_interpolated_attribute_string(parts),
        TokenKind::Bytes(value) => quoted_attribute_bytes(value),
        TokenKind::Keyword(keyword) => keyword_text(*keyword).to_owned(),
        TokenKind::Symbol(symbol) => symbol_text(*symbol).to_owned(),
        TokenKind::Eof => String::new(),
    }
}

fn quoted_interpolated_attribute_string(parts: &[InterpolatedStringTokenPart]) -> String {
    let mut quoted = String::from("f\"");
    for part in parts {
        match part {
            InterpolatedStringTokenPart::Text(value) => {
                append_quoted_attribute_string_body(&mut quoted, value);
            }
            InterpolatedStringTokenPart::Expr { source, .. } => {
                quoted.push('{');
                quoted.push_str(source);
                quoted.push('}');
            }
        }
    }
    quoted.push('"');
    quoted
}

fn quoted_attribute_string(value: &str) -> String {
    let mut quoted = String::from("\"");
    append_quoted_attribute_string_body(&mut quoted, value);
    quoted.push('"');
    quoted
}

fn append_quoted_attribute_string_body(quoted: &mut String, value: &str) {
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
}

fn quoted_attribute_bytes(value: &[u8]) -> String {
    let mut quoted = String::from("b\"");
    for byte in value {
        quoted.push_str(&format!("\\x{byte:02x}"));
    }
    quoted.push('"');
    quoted
}

fn keyword_text(keyword: Keyword) -> &'static str {
    match keyword {
        Keyword::Use => "use",
        Keyword::Pub => "pub",
        Keyword::Const => "const",
        Keyword::Global => "global",
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
        Symbol::ColonColon => "::",
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
