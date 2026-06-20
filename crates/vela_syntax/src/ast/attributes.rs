use super::{AstChildren, AstNode};
use crate::{SyntaxKind, SyntaxNode, SyntaxToken};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxAttribute {
    syntax: SyntaxNode,
}

impl SyntaxAttribute {
    #[must_use]
    pub fn hash_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::Hash)
    }

    #[must_use]
    pub fn l_bracket_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::LBracket)
    }

    #[must_use]
    pub fn r_bracket_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::RBracket)
    }

    #[must_use]
    pub fn path_tokens(&self) -> Vec<SyntaxToken> {
        let mut inside_brackets = false;
        let mut tokens = Vec::new();
        for token in self
            .syntax
            .children_with_tokens()
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
                _ if inside_brackets => tokens.push(token),
                _ => {}
            }
        }
        tokens
    }

    #[must_use]
    pub fn path_text(&self) -> Option<String> {
        let mut path = String::new();
        for token in self.path_tokens() {
            path.push_str(token.text());
        }
        (!path.is_empty()).then_some(path)
    }

    #[must_use]
    pub fn path_segments(&self) -> Vec<String> {
        path_segments_from_tokens(&self.path_tokens())
    }

    #[must_use]
    pub fn path_separator_tokens(&self) -> Vec<SyntaxToken> {
        path_separator_tokens_from_tokens(&self.path_tokens())
    }

    #[must_use]
    pub fn arguments(&self) -> AstChildren<SyntaxAttributeArg> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn l_paren_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::LParen)
    }

    #[must_use]
    pub fn r_paren_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::RParen)
    }

    #[must_use]
    pub fn separator_tokens(&self) -> Vec<SyntaxToken> {
        separator_tokens(&self.syntax, SyntaxKind::Comma)
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxAttributeArg {
    syntax: SyntaxNode,
}

impl SyntaxAttributeArg {
    #[must_use]
    pub fn name_token(&self) -> Option<SyntaxToken> {
        first_significant_token(&self.syntax)
            .filter(|token| token.kind() == SyntaxKind::Ident && self.equal_token().is_some())
    }

    #[must_use]
    pub fn name_text(&self) -> Option<String> {
        self.name_token().map(|token| token.text().to_owned())
    }

    #[must_use]
    pub fn equal_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::Equal)
    }

    #[must_use]
    pub fn value_text(&self) -> Option<String> {
        if self.equal_token().is_some() {
            value_text_after_separator(&self.syntax, SyntaxKind::Equal)
        } else {
            significant_text(&self.syntax)
        }
    }

    #[must_use]
    pub fn value_array(&self) -> Option<SyntaxAttributeArray> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn value_map(&self) -> Option<SyntaxAttributeMap> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxAttributeArg {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::AttributeArg
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxAttributeArray {
    syntax: SyntaxNode,
}

impl SyntaxAttributeArray {
    #[must_use]
    pub fn l_bracket_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::LBracket)
    }

    #[must_use]
    pub fn r_bracket_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::RBracket)
    }

    #[must_use]
    pub fn separator_tokens(&self) -> Vec<SyntaxToken> {
        separator_tokens(&self.syntax, SyntaxKind::Comma)
    }
}

impl AstNode for SyntaxAttributeArray {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::AttributeArray
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxAttributeMap {
    syntax: SyntaxNode,
}

impl SyntaxAttributeMap {
    #[must_use]
    pub fn l_brace_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::LBrace)
    }

    #[must_use]
    pub fn r_brace_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::RBrace)
    }

    #[must_use]
    pub fn entries(&self) -> AstChildren<SyntaxAttributeMapEntry> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn separator_tokens(&self) -> Vec<SyntaxToken> {
        separator_tokens(&self.syntax, SyntaxKind::Comma)
    }
}

impl AstNode for SyntaxAttributeMap {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::AttributeMap
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxAttributeMapEntry {
    syntax: SyntaxNode,
}

impl SyntaxAttributeMapEntry {
    #[must_use]
    pub fn key_token(&self) -> Option<SyntaxToken> {
        first_significant_token(&self.syntax).filter(|token| token.kind() == SyntaxKind::Ident)
    }

    #[must_use]
    pub fn key_text(&self) -> Option<String> {
        self.key_token().map(|token| token.text().to_owned())
    }

    #[must_use]
    pub fn colon_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::Colon)
    }

    #[must_use]
    pub fn value_text(&self) -> Option<String> {
        value_text_after_separator(&self.syntax, SyntaxKind::Colon)
    }

    #[must_use]
    pub fn value_array(&self) -> Option<SyntaxAttributeArray> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn value_map(&self) -> Option<SyntaxAttributeMap> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxAttributeMapEntry {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::AttributeMapEntry
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

fn child<N: AstNode>(parent: &SyntaxNode) -> Option<N> {
    parent.children().find_map(N::cast)
}

fn token(parent: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    parent
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == kind)
}

fn separator_tokens(parent: &SyntaxNode, wanted: SyntaxKind) -> Vec<SyntaxToken> {
    parent
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| token.kind() == wanted)
        .collect()
}

fn path_segments_from_tokens(tokens: &[SyntaxToken]) -> Vec<String> {
    tokens
        .iter()
        .filter(|token| token.kind() == SyntaxKind::Ident)
        .map(|token| token.text().to_owned())
        .collect()
}

fn path_separator_tokens_from_tokens(tokens: &[SyntaxToken]) -> Vec<SyntaxToken> {
    tokens
        .iter()
        .filter(|token| token.kind() == SyntaxKind::ColonColon)
        .cloned()
        .collect()
}

fn first_significant_token(parent: &SyntaxNode) -> Option<SyntaxToken> {
    parent
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
}

fn value_text_after_separator(parent: &SyntaxNode, separator: SyntaxKind) -> Option<String> {
    let mut seen_separator = false;
    let mut parts = Vec::new();

    for element in parent.children_with_tokens() {
        if !seen_separator {
            if element.kind() == separator {
                seen_separator = true;
            }
            continue;
        }
        if parts.is_empty() && element.kind().is_trivia() {
            continue;
        }
        parts.push((element.kind(), element_text(&element)));
    }

    joined_parts_without_trailing_trivia(parts)
}

fn significant_text(parent: &SyntaxNode) -> Option<String> {
    let mut parts = Vec::new();
    for element in parent.children_with_tokens() {
        if parts.is_empty() && element.kind().is_trivia() {
            continue;
        }
        parts.push((element.kind(), element_text(&element)));
    }
    joined_parts_without_trailing_trivia(parts)
}

fn joined_parts_without_trailing_trivia(mut parts: Vec<(SyntaxKind, String)>) -> Option<String> {
    while parts.last().is_some_and(|(kind, _)| kind.is_trivia()) {
        parts.pop();
    }
    let text = parts.into_iter().map(|(_, text)| text).collect::<String>();
    (!text.is_empty()).then_some(text)
}

fn element_text(element: &crate::SyntaxElement) -> String {
    match element {
        rowan::NodeOrToken::Node(node) => node.text().to_string(),
        rowan::NodeOrToken::Token(token) => token.text().to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::{AstNode, SyntaxAttribute, SyntaxSourceFile};
    use crate::parse::parse_source;
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
        assert_eq!(attribute.path_segments(), vec!["derive", "PartialEq"]);
        assert_eq!(
            attribute
                .path_separator_tokens()
                .iter()
                .map(|token| token.kind())
                .collect::<Vec<_>>(),
            vec![SyntaxKind::ColonColon]
        );
        assert_eq!(
            SyntaxAttribute::cast(attribute.syntax().clone()).expect("attribute node"),
            attribute
        );
    }

    #[test]
    fn ast_attribute_exposes_argument_children() {
        let source = r#"
#[rule(kind = game::reward::Rule, tags = ["daily", "quest"], config = { enabled: true, limit: 10 })]
fn main() {}
"#;
        let parse = parse_source(source);
        let tree = parse.tree();
        let function = tree.functions().next().expect("function item");
        let attribute = function.attributes().next().expect("attribute");
        let arguments = attribute.arguments().collect::<Vec<_>>();

        assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
        assert_eq!(attribute.hash_token().expect("hash").text(), "#");
        assert_eq!(
            attribute.l_bracket_token().expect("open bracket").text(),
            "["
        );
        assert_eq!(
            attribute.r_bracket_token().expect("close bracket").text(),
            "]"
        );
        assert_eq!(attribute.path_text().as_deref(), Some("rule"));
        assert_eq!(attribute.path_segments(), vec!["rule"]);
        assert!(attribute.path_separator_tokens().is_empty());
        assert_eq!(attribute.l_paren_token().expect("open paren").text(), "(");
        assert_eq!(attribute.r_paren_token().expect("close paren").text(), ")");
        assert_eq!(
            attribute
                .separator_tokens()
                .iter()
                .map(|token| token.text().to_owned())
                .collect::<Vec<_>>(),
            vec![",", ","]
        );

        assert_eq!(
            arguments
                .iter()
                .map(|argument| argument.name_text().expect("argument name"))
                .collect::<Vec<_>>(),
            vec!["kind", "tags", "config"]
        );
        assert_eq!(
            arguments[0].value_text().as_deref(),
            Some("game::reward::Rule")
        );
        assert_eq!(
            arguments[1].value_text().as_deref(),
            Some("[\"daily\", \"quest\"]")
        );
        assert_eq!(
            arguments[1]
                .value_array()
                .expect("attribute array")
                .separator_tokens()
                .len(),
            1
        );

        let config = arguments[2].value_map().expect("attribute map");
        assert_eq!(config.l_brace_token().expect("map open").text(), "{");
        assert_eq!(config.r_brace_token().expect("map close").text(), "}");
        assert_eq!(
            config
                .separator_tokens()
                .iter()
                .map(|token| token.text().to_owned())
                .collect::<Vec<_>>(),
            vec![","]
        );
        let entries = config.entries().collect::<Vec<_>>();
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.key_text().expect("map key"))
                .collect::<Vec<_>>(),
            vec!["enabled", "limit"]
        );
        assert_eq!(entries[0].colon_token().expect("colon").text(), ":");
        assert_eq!(entries[0].value_text().as_deref(), Some("true"));
        assert_eq!(entries[1].value_text().as_deref(), Some("10"));
    }
}
