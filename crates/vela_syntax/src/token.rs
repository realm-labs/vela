use vela_common::Span;

use crate::SyntaxKind;
use crate::ast::{FloatLiteral, IntegerLiteral};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LosslessToken {
    pub kind: SyntaxKind,
    pub span: Span,
    pub text: String,
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

impl TokenKind {
    #[must_use]
    pub fn syntax_kind(&self) -> SyntaxKind {
        match self {
            Self::Ident(_) => SyntaxKind::Ident,
            Self::Int(_) => SyntaxKind::Int,
            Self::Float(_) => SyntaxKind::Float,
            Self::Char(_) => SyntaxKind::Char,
            Self::String(_) => SyntaxKind::String,
            Self::InterpolatedString(_) => SyntaxKind::InterpolatedString,
            Self::Bytes(_) => SyntaxKind::Bytes,
            Self::Keyword(keyword) => keyword.syntax_kind(),
            Self::Symbol(symbol) => symbol.syntax_kind(),
            Self::Eof => SyntaxKind::Eof,
        }
    }
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

    #[must_use]
    pub const fn syntax_kind(self) -> SyntaxKind {
        match self {
            Self::Use => SyntaxKind::UseKw,
            Self::Pub => SyntaxKind::PubKw,
            Self::Const => SyntaxKind::ConstKw,
            Self::Global => SyntaxKind::GlobalKw,
            Self::Let => SyntaxKind::LetKw,
            Self::Fn => SyntaxKind::FnKw,
            Self::Struct => SyntaxKind::StructKw,
            Self::Enum => SyntaxKind::EnumKw,
            Self::Trait => SyntaxKind::TraitKw,
            Self::Impl => SyntaxKind::ImplKw,
            Self::For => SyntaxKind::ForKw,
            Self::If => SyntaxKind::IfKw,
            Self::Else => SyntaxKind::ElseKw,
            Self::Match => SyntaxKind::MatchKw,
            Self::Return => SyntaxKind::ReturnKw,
            Self::Break => SyntaxKind::BreakKw,
            Self::Continue => SyntaxKind::ContinueKw,
            Self::True => SyntaxKind::TrueKw,
            Self::False => SyntaxKind::FalseKw,
            Self::Null => SyntaxKind::NullKw,
            Self::SelfValue => SyntaxKind::SelfKw,
            Self::In => SyntaxKind::InKw,
            Self::As => SyntaxKind::AsKw,
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

impl Symbol {
    #[must_use]
    pub const fn syntax_kind(self) -> SyntaxKind {
        match self {
            Self::Hash => SyntaxKind::Hash,
            Self::LBracket => SyntaxKind::LBracket,
            Self::RBracket => SyntaxKind::RBracket,
            Self::LParen => SyntaxKind::LParen,
            Self::RParen => SyntaxKind::RParen,
            Self::LBrace => SyntaxKind::LBrace,
            Self::RBrace => SyntaxKind::RBrace,
            Self::Comma => SyntaxKind::Comma,
            Self::Dot => SyntaxKind::Dot,
            Self::DotDot => SyntaxKind::DotDot,
            Self::DotDotEqual => SyntaxKind::DotDotEqual,
            Self::Colon => SyntaxKind::Colon,
            Self::ColonColon => SyntaxKind::ColonColon,
            Self::Semicolon => SyntaxKind::Semicolon,
            Self::Arrow => SyntaxKind::Arrow,
            Self::FatArrow => SyntaxKind::FatArrow,
            Self::Equal => SyntaxKind::Equal,
            Self::PlusEqual => SyntaxKind::PlusEqual,
            Self::MinusEqual => SyntaxKind::MinusEqual,
            Self::StarEqual => SyntaxKind::StarEqual,
            Self::SlashEqual => SyntaxKind::SlashEqual,
            Self::PercentEqual => SyntaxKind::PercentEqual,
            Self::Plus => SyntaxKind::Plus,
            Self::Minus => SyntaxKind::Minus,
            Self::Star => SyntaxKind::Star,
            Self::Slash => SyntaxKind::Slash,
            Self::Percent => SyntaxKind::Percent,
            Self::Bang => SyntaxKind::Bang,
            Self::BangEqual => SyntaxKind::BangEqual,
            Self::BangEqualEqual => SyntaxKind::BangEqualEqual,
            Self::EqualEqual => SyntaxKind::EqualEqual,
            Self::EqualEqualEqual => SyntaxKind::EqualEqualEqual,
            Self::Less => SyntaxKind::Less,
            Self::LessEqual => SyntaxKind::LessEqual,
            Self::Greater => SyntaxKind::Greater,
            Self::GreaterEqual => SyntaxKind::GreaterEqual,
            Self::AndAnd => SyntaxKind::AndAnd,
            Self::OrOr => SyntaxKind::OrOr,
            Self::Pipe => SyntaxKind::Pipe,
            Self::Question => SyntaxKind::Question,
        }
    }
}
