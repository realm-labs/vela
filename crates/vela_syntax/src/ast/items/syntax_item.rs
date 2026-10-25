use super::{AstChildren, SyntaxAttribute, SyntaxItem, SyntaxToken, TextRange, pub_token};

impl SyntaxItem {
    #[must_use]
    pub fn text_range(&self) -> TextRange {
        self.syntax.text_range()
    }

    #[must_use]
    pub fn attributes(&self) -> AstChildren<SyntaxAttribute> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn pub_token(&self) -> Option<SyntaxToken> {
        pub_token(&self.syntax)
    }

    #[must_use]
    pub fn is_public(&self) -> bool {
        self.pub_token().is_some()
    }
}
