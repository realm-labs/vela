use std::marker::PhantomData;

use vela_common::Diagnostic;

use crate::SyntaxNode;

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
