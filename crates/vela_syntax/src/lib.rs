//! Lexer and parser for Vela source files.

pub mod ast;
mod attribute;
pub mod formatting;
pub mod lexer;
pub mod parse;
pub mod parser;
pub mod syntax_kind;
pub mod syntax_node;
pub mod token;

pub use parse::Parse;
pub use rowan::{
    GreenNode, NodeOrToken, SyntaxText, TextRange, TextSize, TokenAtOffset, WalkEvent,
};
pub use syntax_kind::SyntaxKind;
pub use syntax_node::{
    SyntaxElement, SyntaxElementChildren, SyntaxNode, SyntaxNodeChildren, SyntaxToken,
    SyntaxTreeBuilder, VelaLanguage,
};
