use super::{AstChildren, AstNode};
use crate::{SyntaxKind, SyntaxNode, SyntaxToken};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxPattern {
    syntax: SyntaxNode,
}

impl AstNode for SyntaxPattern {
    fn can_cast(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            SyntaxKind::Pattern | SyntaxKind::TuplePattern | SyntaxKind::RecordPattern
        )
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

impl SyntaxPattern {
    #[must_use]
    pub fn is_wildcard(&self) -> bool {
        significant_tokens(&self.syntax)
            .map(|token| token.text().to_owned())
            .eq(["_"])
    }

    #[must_use]
    pub fn binding_name(&self) -> Option<String> {
        let mut tokens = significant_tokens(&self.syntax);
        let token = tokens.next()?;
        (tokens.next().is_none() && token.kind() == SyntaxKind::Ident && token.text() != "_")
            .then(|| token.text().to_owned())
    }

    #[must_use]
    pub fn literal_text(&self) -> Option<String> {
        let token = first_significant_token(&self.syntax)?;
        literal_token(token.kind()).then(|| self.syntax.text().to_string())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxTuplePattern {
    syntax: SyntaxNode,
}

impl SyntaxTuplePattern {
    #[must_use]
    pub fn path_text(&self) -> Option<String> {
        path_text_before(&self.syntax, SyntaxKind::LParen)
    }

    #[must_use]
    pub fn patterns(&self) -> AstChildren<SyntaxPattern> {
        AstChildren::new(&self.syntax)
    }
}

impl AstNode for SyntaxTuplePattern {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TuplePattern
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxRecordPattern {
    syntax: SyntaxNode,
}

impl SyntaxRecordPattern {
    #[must_use]
    pub fn path_text(&self) -> Option<String> {
        path_text_before(&self.syntax, SyntaxKind::LBrace)
    }

    #[must_use]
    pub fn fields(&self) -> AstChildren<SyntaxRecordPatternField> {
        AstChildren::new(&self.syntax)
    }
}

impl AstNode for SyntaxRecordPattern {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::RecordPattern
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxRecordPatternField {
    syntax: SyntaxNode,
}

impl SyntaxRecordPatternField {
    #[must_use]
    pub fn label_text(&self) -> Option<String> {
        first_significant_token(&self.syntax)
            .filter(|token| token.kind() == SyntaxKind::Ident)
            .map(|token| token.text().to_owned())
    }

    #[must_use]
    pub fn pattern(&self) -> Option<SyntaxPattern> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxRecordPatternField {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::RecordPatternField
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

fn first_significant_token(parent: &SyntaxNode) -> Option<SyntaxToken> {
    significant_tokens(parent).next()
}

fn significant_tokens(parent: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> + '_ {
    parent
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
}

fn path_text_before(parent: &SyntaxNode, delimiter: SyntaxKind) -> Option<String> {
    let mut path = String::new();
    for token in significant_tokens(parent) {
        if token.kind() == delimiter {
            break;
        }
        path.push_str(token.text());
    }
    (!path.is_empty()).then_some(path)
}

const fn literal_token(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::TrueKw
            | SyntaxKind::FalseKw
            | SyntaxKind::NullKw
            | SyntaxKind::Int
            | SyntaxKind::Float
            | SyntaxKind::Char
            | SyntaxKind::String
            | SyntaxKind::Bytes
    )
}
