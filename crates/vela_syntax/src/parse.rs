use std::marker::PhantomData;

use vela_common::{Diagnostic, SourceId};

use crate::ast::{AstNode, SyntaxSourceFile};
use crate::lexer::lex;
use crate::parser::cst;
use crate::syntax_validation::validate_source;
use crate::{SyntaxNode, SyntaxTreeBuilder};

#[must_use]
pub fn parse_source(text: &str) -> Parse<SyntaxSourceFile> {
    parse_source_with_id(SourceId::new(0), text)
}

#[must_use]
pub fn parse_source_with_id(source: SourceId, text: &str) -> Parse<SyntaxSourceFile> {
    let lexed = lex(source, text);
    let mut builder = SyntaxTreeBuilder::default();
    let mut diagnostics = lexed.diagnostics.clone();
    diagnostics.extend(cst::build_source_tree(&lexed, &mut builder));

    let parse = builder.finish_with_diagnostics(diagnostics);
    let mut diagnostics = parse.diagnostics().to_vec();
    diagnostics.extend(validate_source(source, &parse.tree()));
    Parse::new(parse.green().clone(), diagnostics)
}

#[derive(Debug, Eq, PartialEq)]
pub struct Parse<T> {
    green: rowan::GreenNode,
    diagnostics: Vec<Diagnostic>,
    _ty: PhantomData<fn() -> T>,
}

impl<T> Clone for Parse<T> {
    fn clone(&self) -> Self {
        Self {
            green: self.green.clone(),
            diagnostics: self.diagnostics.clone(),
            _ty: PhantomData,
        }
    }
}

impl<T> Parse<T> {
    #[must_use]
    pub fn new(green: rowan::GreenNode, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            green,
            diagnostics,
            _ty: PhantomData,
        }
    }

    #[must_use]
    pub fn syntax_node(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    #[must_use]
    pub fn green(&self) -> &rowan::GreenNode {
        &self.green
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    #[must_use]
    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Parse<SyntaxSourceFile> {
    #[must_use]
    pub fn tree(&self) -> SyntaxSourceFile {
        SyntaxSourceFile::cast(self.syntax_node())
            .expect("parse_source must produce a source-file root")
    }
}

#[cfg(test)]
mod tests;
