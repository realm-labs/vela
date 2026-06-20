use super::{AstChildren, AstNode, SyntaxBlock, SyntaxParamList, SyntaxPattern};
use crate::{SyntaxKind, SyntaxNode};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxExpression {
    syntax: SyntaxNode,
}

impl AstNode for SyntaxExpression {
    fn can_cast(kind: SyntaxKind) -> bool {
        expression_kind(kind)
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxLiteral {
    syntax: SyntaxNode,
}

impl AstNode for SyntaxLiteral {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Literal
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxPathExpr {
    syntax: SyntaxNode,
}

impl AstNode for SyntaxPathExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PathExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxAssignExpr {
    syntax: SyntaxNode,
}

impl SyntaxAssignExpr {
    #[must_use]
    pub fn expressions(&self) -> AstChildren<SyntaxExpression> {
        AstChildren::new(&self.syntax)
    }
}

impl AstNode for SyntaxAssignExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::AssignExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxBinaryExpr {
    syntax: SyntaxNode,
}

impl SyntaxBinaryExpr {
    #[must_use]
    pub fn expressions(&self) -> AstChildren<SyntaxExpression> {
        AstChildren::new(&self.syntax)
    }
}

impl AstNode for SyntaxBinaryExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::BinaryExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxUnaryExpr {
    syntax: SyntaxNode,
}

impl SyntaxUnaryExpr {
    #[must_use]
    pub fn expression(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxUnaryExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::UnaryExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxFieldExpr {
    syntax: SyntaxNode,
}

impl SyntaxFieldExpr {
    #[must_use]
    pub fn receiver(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxFieldExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::FieldExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxCallExpr {
    syntax: SyntaxNode,
}

impl SyntaxCallExpr {
    #[must_use]
    pub fn callee(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn arg_list(&self) -> Option<SyntaxArgList> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxCallExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CallExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxIndexExpr {
    syntax: SyntaxNode,
}

impl SyntaxIndexExpr {
    #[must_use]
    pub fn expressions(&self) -> AstChildren<SyntaxExpression> {
        AstChildren::new(&self.syntax)
    }
}

impl AstNode for SyntaxIndexExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::IndexExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxTryExpr {
    syntax: SyntaxNode,
}

impl SyntaxTryExpr {
    #[must_use]
    pub fn expression(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxTryExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TryExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxArgList {
    syntax: SyntaxNode,
}

impl SyntaxArgList {
    #[must_use]
    pub fn arguments(&self) -> AstChildren<SyntaxArgument> {
        AstChildren::new(&self.syntax)
    }
}

impl AstNode for SyntaxArgList {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ArgList
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxArgument {
    syntax: SyntaxNode,
}

impl SyntaxArgument {
    #[must_use]
    pub fn expression(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxArgument {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Argument
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxArrayExpr {
    syntax: SyntaxNode,
}

impl SyntaxArrayExpr {
    #[must_use]
    pub fn expressions(&self) -> AstChildren<SyntaxExpression> {
        AstChildren::new(&self.syntax)
    }
}

impl AstNode for SyntaxArrayExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ArrayExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxMapExpr {
    syntax: SyntaxNode,
}

impl SyntaxMapExpr {
    #[must_use]
    pub fn entries(&self) -> AstChildren<SyntaxMapEntry> {
        AstChildren::new(&self.syntax)
    }
}

impl AstNode for SyntaxMapExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MapExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxMapEntry {
    syntax: SyntaxNode,
}

impl SyntaxMapEntry {
    #[must_use]
    pub fn expressions(&self) -> AstChildren<SyntaxExpression> {
        AstChildren::new(&self.syntax)
    }
}

impl AstNode for SyntaxMapEntry {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MapEntry
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxRecordExpr {
    syntax: SyntaxNode,
}

impl SyntaxRecordExpr {
    #[must_use]
    pub fn path(&self) -> Option<SyntaxPathExpr> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn field_list(&self) -> Option<SyntaxRecordExprFieldList> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxRecordExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::RecordExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxRecordExprFieldList {
    syntax: SyntaxNode,
}

impl SyntaxRecordExprFieldList {
    #[must_use]
    pub fn fields(&self) -> AstChildren<SyntaxRecordExprField> {
        AstChildren::new(&self.syntax)
    }
}

impl AstNode for SyntaxRecordExprFieldList {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::RecordExprFieldList
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxRecordExprField {
    syntax: SyntaxNode,
}

impl SyntaxRecordExprField {
    #[must_use]
    pub fn expression(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxRecordExprField {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::RecordExprField
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxLambdaExpr {
    syntax: SyntaxNode,
}

impl SyntaxLambdaExpr {
    #[must_use]
    pub fn param_list(&self) -> Option<SyntaxParamList> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn body_expression(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn body_block(&self) -> Option<SyntaxBlock> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxLambdaExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::LambdaExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxMatchExpr {
    syntax: SyntaxNode,
}

impl SyntaxMatchExpr {
    #[must_use]
    pub fn scrutinee(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn arm_list(&self) -> Option<SyntaxMatchArmList> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxMatchExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MatchExpr
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxMatchArmList {
    syntax: SyntaxNode,
}

impl SyntaxMatchArmList {
    #[must_use]
    pub fn arms(&self) -> AstChildren<SyntaxMatchArm> {
        AstChildren::new(&self.syntax)
    }
}

impl AstNode for SyntaxMatchArmList {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MatchArmList
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxMatchArm {
    syntax: SyntaxNode,
}

impl SyntaxMatchArm {
    #[must_use]
    pub fn pattern(&self) -> Option<SyntaxPattern> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn guard(&self) -> Option<SyntaxExpression> {
        self.has_guard()
            .then(|| self.expressions().next())
            .flatten()
    }

    #[must_use]
    pub fn expressions(&self) -> AstChildren<SyntaxExpression> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn body_expression(&self) -> Option<SyntaxExpression> {
        if self.body_block().is_some() {
            return None;
        }
        let mut expressions = self.expressions();
        if self.has_guard() {
            expressions.next();
        }
        expressions.next()
    }

    #[must_use]
    pub fn body_block(&self) -> Option<SyntaxBlock> {
        child(&self.syntax)
    }

    fn has_guard(&self) -> bool {
        self.syntax
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .take_while(|token| token.kind() != SyntaxKind::FatArrow)
            .any(|token| token.kind() == SyntaxKind::IfKw)
    }
}

impl AstNode for SyntaxMatchArm {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MatchArm
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

fn expression_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Literal
            | SyntaxKind::PathExpr
            | SyntaxKind::UnaryExpr
            | SyntaxKind::BinaryExpr
            | SyntaxKind::AssignExpr
            | SyntaxKind::FieldExpr
            | SyntaxKind::CallExpr
            | SyntaxKind::IndexExpr
            | SyntaxKind::TryExpr
            | SyntaxKind::ArrayExpr
            | SyntaxKind::MapExpr
            | SyntaxKind::RecordExpr
            | SyntaxKind::LambdaExpr
            | SyntaxKind::Block
            | SyntaxKind::IfExpr
            | SyntaxKind::MatchExpr
    )
}

fn child<N: AstNode>(parent: &SyntaxNode) -> Option<N> {
    parent.children().find_map(N::cast)
}

#[cfg(test)]
mod tests {
    use crate::SyntaxKind;
    use crate::ast::{AstNode, SyntaxBlock, SyntaxMapExpr};
    use crate::parse::parse_source;

    #[test]
    fn ast_block_expression_exposes_statement_children() {
        let source = r#"fn update(score) {
    let value = {
        return score;
    };
    let table = { score: score };
}
"#;
        let parse = parse_source(source);
        let body = parse
            .tree()
            .functions()
            .next()
            .expect("function item")
            .body()
            .expect("function body");
        let lets = body.let_statements().collect::<Vec<_>>();

        assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
        assert_eq!(lets.len(), 2);

        let block_initializer = lets[0].initializer().expect("block initializer");
        assert_eq!(block_initializer.syntax().kind(), SyntaxKind::Block);
        let block =
            SyntaxBlock::cast(block_initializer.syntax().clone()).expect("typed block expression");
        assert_eq!(
            block
                .statements()
                .map(|statement| statement.syntax().kind())
                .collect::<Vec<_>>(),
            vec![SyntaxKind::ReturnStmt]
        );

        let map_initializer = lets[1].initializer().expect("map initializer");
        assert_eq!(map_initializer.syntax().kind(), SyntaxKind::MapExpr);
        assert_eq!(
            SyntaxMapExpr::cast(map_initializer.syntax().clone())
                .expect("typed map expression")
                .entries()
                .count(),
            1
        );
    }
}
