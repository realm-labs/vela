#![cfg_attr(not(test), deny(clippy::wildcard_imports))]

//! Lexer and parser for Vela source files.

pub mod ast;
mod cst_parser;
pub mod formatting;
pub mod lexer;
pub mod parse;
pub mod syntax_kind;
pub mod syntax_node;
mod syntax_validation;
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
