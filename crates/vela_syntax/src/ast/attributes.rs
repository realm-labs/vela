use super::AstNode;
use crate::{SyntaxKind, SyntaxNode};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxAttribute {
    syntax: SyntaxNode,
}

impl SyntaxAttribute {
    #[must_use]
    pub fn path_text(&self) -> Option<String> {
        let mut path = String::new();
        let mut inside_brackets = false;
        for token in self
            .syntax
            .descendants_with_tokens()
            .filter_map(|element| element.into_token())
            .filter(|token| !token.kind().is_trivia())
        {
            match token.kind() {
                SyntaxKind::LBracket => {
                    inside_brackets = true;
                }
                SyntaxKind::LParen | SyntaxKind::RBracket if inside_brackets => {
                    break;
                }
                _ if inside_brackets => path.push_str(token.text()),
                _ => {}
            }
        }
        (!path.is_empty()).then_some(path)
    }
}

impl AstNode for SyntaxAttribute {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Attribute
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::{AstNode, SyntaxAttribute, SyntaxSourceFile};
    use crate::{SyntaxKind, SyntaxTreeBuilder};

    #[test]
    fn ast_attribute_exposes_path_text() {
        let mut builder = SyntaxTreeBuilder::default();
        builder.start_node(SyntaxKind::SourceFile);
        builder.start_node(SyntaxKind::FunctionItem);
        builder.start_node(SyntaxKind::Attribute);
        builder.token(SyntaxKind::Hash, "#");
        builder.token(SyntaxKind::LBracket, "[");
        builder.token(SyntaxKind::Ident, "derive");
        builder.token(SyntaxKind::ColonColon, "::");
        builder.token(SyntaxKind::Ident, "PartialEq");
        builder.token(SyntaxKind::LParen, "(");
        builder.token(SyntaxKind::RParen, ")");
        builder.token(SyntaxKind::RBracket, "]");
        builder.finish_node();
        builder.finish_node();
        builder.finish_node();

        let parse: crate::Parse<SyntaxSourceFile> = builder.finish();
        let source = SyntaxSourceFile::cast(parse.syntax_node()).expect("source file root");
        let attribute = source
            .functions()
            .next()
            .expect("function item")
            .attributes()
            .next()
            .expect("attribute");

        assert_eq!(attribute.path_text().as_deref(), Some("derive::PartialEq"));
        assert_eq!(
            SyntaxAttribute::cast(attribute.syntax().clone()).expect("attribute node"),
            attribute
        );
    }
}
