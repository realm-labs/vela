//! Lexer and parser for Vela source files.

pub mod ast;
#[cfg(feature = "legacy-body-parser")]
mod attribute;
mod cst_parser;
pub mod formatting;
pub mod lexer;
pub mod parse;
mod parser;
pub mod syntax_kind;
pub mod syntax_node;
mod syntax_validation;
pub mod token;

#[cfg(feature = "legacy-body-parser")]
pub mod body_parser_support {
    pub use crate::parser::parse_body_blocks_at_spans;
}

pub use parse::Parse;
pub use rowan::{
    GreenNode, NodeOrToken, SyntaxText, TextRange, TextSize, TokenAtOffset, WalkEvent,
};
pub use syntax_kind::SyntaxKind;
pub use syntax_node::{
    SyntaxElement, SyntaxElementChildren, SyntaxNode, SyntaxNodeChildren, SyntaxToken,
    SyntaxTreeBuilder, VelaLanguage,
};
