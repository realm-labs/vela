//! Lexer and parser for Vela source files.

pub mod ast;
mod lexer;
mod parser;
mod token;

pub use ast::*;
pub use lexer::{Lexed, lex};
pub use parser::parse_source;
pub use token::{Keyword, Symbol, Token, TokenKind};
