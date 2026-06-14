use vela_common::Span;

use crate::ast::{FloatLiteral, IntegerLiteral};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TokenKind {
    Ident(String),
    Int(IntegerLiteral),
    Float(FloatLiteral),
    Char(char),
    String(String),
    InterpolatedString(Vec<InterpolatedStringTokenPart>),
    Bytes(Vec<u8>),
    Keyword(Keyword),
    Symbol(Symbol),
    Eof,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InterpolatedStringTokenPart {
    Text(String),
    Expr { source: String, span: Span },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Keyword {
    Use,
    Pub,
    Const,
    Global,
    Let,
    Fn,
    Struct,
    Enum,
    Trait,
    Impl,
    For,
    If,
    Else,
    Match,
    Return,
    Break,
    Continue,
    True,
    False,
    Null,
    SelfValue,
    In,
    As,
}

impl Keyword {
    #[must_use]
    pub fn from_text(text: &str) -> Option<Self> {
        match text {
            "use" => Some(Self::Use),
            "pub" => Some(Self::Pub),
            "const" => Some(Self::Const),
            "global" => Some(Self::Global),
            "let" => Some(Self::Let),
            "fn" => Some(Self::Fn),
            "struct" => Some(Self::Struct),
            "enum" => Some(Self::Enum),
            "trait" => Some(Self::Trait),
            "impl" => Some(Self::Impl),
            "for" => Some(Self::For),
            "if" => Some(Self::If),
            "else" => Some(Self::Else),
            "match" => Some(Self::Match),
            "return" => Some(Self::Return),
            "break" => Some(Self::Break),
            "continue" => Some(Self::Continue),
            "true" => Some(Self::True),
            "false" => Some(Self::False),
            "null" => Some(Self::Null),
            "self" => Some(Self::SelfValue),
            "in" => Some(Self::In),
            "as" => Some(Self::As),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Symbol {
    Hash,
    LBracket,
    RBracket,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Dot,
    DotDot,
    DotDotEqual,
    Colon,
    ColonColon,
    Semicolon,
    Arrow,
    FatArrow,
    Equal,
    PlusEqual,
    MinusEqual,
    StarEqual,
    SlashEqual,
    PercentEqual,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Bang,
    BangEqual,
    BangEqualEqual,
    EqualEqual,
    EqualEqualEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    AndAnd,
    OrOr,
    Pipe,
    Question,
}
