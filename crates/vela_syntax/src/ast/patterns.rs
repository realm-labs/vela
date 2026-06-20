use super::{AstChildren, AstNode};
use crate::{SyntaxKind, SyntaxNode};

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxTuplePattern {
    syntax: SyntaxNode,
}

impl SyntaxTuplePattern {
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
