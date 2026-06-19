use rowan::{GreenNodeBuilder, Language};

use crate::SyntaxKind;
use crate::parse::Parse;
pub use crate::syntax_kind::VelaLanguage;

pub type SyntaxNode = rowan::SyntaxNode<VelaLanguage>;
pub type SyntaxToken = rowan::SyntaxToken<VelaLanguage>;
pub type SyntaxElement = rowan::SyntaxElement<VelaLanguage>;
pub type SyntaxNodeChildren = rowan::SyntaxNodeChildren<VelaLanguage>;
pub type SyntaxElementChildren = rowan::SyntaxElementChildren<VelaLanguage>;

#[derive(Default)]
pub struct SyntaxTreeBuilder {
    inner: GreenNodeBuilder<'static>,
}

impl SyntaxTreeBuilder {
    pub fn start_node(&mut self, kind: SyntaxKind) {
        self.inner.start_node(VelaLanguage::kind_to_raw(kind));
    }

    pub fn finish_node(&mut self) {
        self.inner.finish_node();
    }

    pub fn token(&mut self, kind: SyntaxKind, text: &str) {
        self.inner.token(VelaLanguage::kind_to_raw(kind), text);
    }

    #[must_use]
    pub fn finish<T>(self) -> Parse<T> {
        Parse::new(self.inner.finish(), Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use crate::{SyntaxKind, SyntaxTreeBuilder};

    #[test]
    fn syntax_tree_builder_creates_rowan_root() {
        let mut builder = SyntaxTreeBuilder::default();
        builder.start_node(SyntaxKind::SourceFile);
        builder.token(SyntaxKind::Whitespace, "\n");
        builder.finish_node();

        let parse: crate::Parse<crate::ast::SourceFile> = builder.finish();
        let root = parse.syntax_node();

        assert_eq!(root.kind(), SyntaxKind::SourceFile);
        assert_eq!(root.text().to_string(), "\n");
        assert!(parse.diagnostics().is_empty());
    }
}
