use super::{
    AstChildren, SyntaxAttribute, SyntaxItem, SyntaxKind, SyntaxToken, TextRange, pub_token, token,
};

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

    #[must_use]
    pub fn async_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::AsyncKw)
    }

    #[must_use]
    pub fn is_async(&self) -> bool {
        self.async_token().is_some()
    }
}
