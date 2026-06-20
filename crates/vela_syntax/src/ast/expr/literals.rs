use crate::ast::literal_semantics::literal_from_token;
use crate::ast::{AstChildren, AstNode, Literal, SyntaxExpression};
use crate::{SyntaxKind, SyntaxNode, SyntaxToken};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxLiteral {
    syntax: SyntaxNode,
}

impl SyntaxLiteral {
    #[must_use]
    pub fn token(&self) -> Option<SyntaxToken> {
        self.syntax
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| literal_token_kind(token.kind()))
    }

    #[must_use]
    pub fn token_kind(&self) -> Option<SyntaxKind> {
        self.token().map(|token| token.kind())
    }

    #[must_use]
    pub fn token_text(&self) -> Option<String> {
        self.token().map(|token| {
            if token.kind() == SyntaxKind::InterpolatedString {
                self.syntax.text().to_string()
            } else {
                token.text().to_owned()
            }
        })
    }

    #[must_use]
    pub fn interpolations(&self) -> AstChildren<SyntaxInterpolation> {
        AstChildren::new(&self.syntax)
    }

    pub fn interpolation_expressions(&self) -> impl Iterator<Item = SyntaxExpression> {
        self.interpolations()
            .filter_map(|interpolation| interpolation.expression())
    }

    #[must_use]
    pub fn literal(&self) -> Option<Literal> {
        let token = self.token()?;
        literal_from_token(token.kind(), token.text())
    }
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
pub struct SyntaxInterpolation {
    syntax: SyntaxNode,
}

impl SyntaxInterpolation {
    #[must_use]
    pub fn l_brace_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::LBrace)
    }

    #[must_use]
    pub fn r_brace_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::RBrace)
    }

    #[must_use]
    pub fn expression(&self) -> Option<SyntaxExpression> {
        child(&self.syntax)
    }
}

impl AstNode for SyntaxInterpolation {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::Interpolation
    }

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

fn literal_token_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::TrueKw
            | SyntaxKind::FalseKw
            | SyntaxKind::NullKw
            | SyntaxKind::Int
            | SyntaxKind::Float
            | SyntaxKind::Char
            | SyntaxKind::String
            | SyntaxKind::InterpolatedString
            | SyntaxKind::Bytes
    )
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

#[cfg(test)]
mod tests {
    use crate::SyntaxKind;
    use crate::ast::{
        AstNode, FloatSuffix, IntRadix, IntegerSuffix, Literal, SyntaxExpressionKind, SyntaxLiteral,
    };
    use crate::parse::parse_source;

    #[test]
    fn ast_literal_expression_exposes_token_text_kind_and_semantic_value() {
        let source = r#"fn literals(name) {
    let truthy = true;
    let falsey = false;
    let empty = null;
    let count = 42;
    let hex = 0x2a;
    let typed_int = 12i8;
    let ratio = 3.5;
    let typed_float = 12.0f32;
    let label = "gold";
    let marker = 'x';
    let packet = b"\x00\xff";
    let message = f"hello {name}";
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
        let literals = body
            .let_statements()
            .map(|statement| {
                let initializer = statement.initializer().expect("initializer");
                let literal =
                    SyntaxLiteral::cast(initializer.syntax().clone()).expect("literal expr");
                (literal.token_kind(), literal.token_text())
            })
            .collect::<Vec<_>>();
        let semantic_literals = body
            .let_statements()
            .map(|statement| {
                let initializer = statement.initializer().expect("initializer");
                let literal =
                    SyntaxLiteral::cast(initializer.syntax().clone()).expect("literal expr");
                literal.literal()
            })
            .collect::<Vec<_>>();

        assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
        assert_eq!(
            literals,
            vec![
                (Some(SyntaxKind::TrueKw), Some("true".to_owned())),
                (Some(SyntaxKind::FalseKw), Some("false".to_owned())),
                (Some(SyntaxKind::NullKw), Some("null".to_owned())),
                (Some(SyntaxKind::Int), Some("42".to_owned())),
                (Some(SyntaxKind::Int), Some("0x2a".to_owned())),
                (Some(SyntaxKind::Int), Some("12i8".to_owned())),
                (Some(SyntaxKind::Float), Some("3.5".to_owned())),
                (Some(SyntaxKind::Float), Some("12.0f32".to_owned())),
                (Some(SyntaxKind::String), Some(r#""gold""#.to_owned())),
                (Some(SyntaxKind::Char), Some("'x'".to_owned())),
                (Some(SyntaxKind::Bytes), Some(r#"b"\x00\xff""#.to_owned())),
                (
                    Some(SyntaxKind::InterpolatedString),
                    Some(r#"f"hello {name}""#.to_owned()),
                ),
            ]
        );
        assert_eq!(
            semantic_literals[0..4],
            [
                Some(Literal::Bool(true)),
                Some(Literal::Bool(false)),
                Some(Literal::Null),
                Some(Literal::integer("42")),
            ]
        );
        assert!(matches!(
            &semantic_literals[4],
            Some(Literal::Integer(value))
                if value.source_text() == "0x2a"
                    && value.radix == IntRadix::Hex
                    && value.suffix.is_none()
        ));
        assert!(matches!(
            &semantic_literals[5],
            Some(Literal::Integer(value))
                if value.source_text() == "12"
                    && value.radix == IntRadix::Decimal
                    && value.suffix == Some(IntegerSuffix::I8)
        ));
        assert_eq!(semantic_literals[6], Some(Literal::float("3.5")));
        assert!(matches!(
            &semantic_literals[7],
            Some(Literal::Float(value))
                if value.source_text() == "12.0" && value.suffix == Some(FloatSuffix::F32)
        ));
        assert_eq!(
            semantic_literals[8..],
            [
                Some(Literal::String("gold".to_owned())),
                Some(Literal::Char('x')),
                Some(Literal::Bytes(vec![0, 255])),
                None,
            ]
        );
    }

    #[test]
    fn ast_interpolated_literal_exposes_embedded_expressions() {
        let source = r#"fn greet(name, player) {
    let message = f"hello {name} {player.level + 1} {{ok}}";
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
        let initializer = body
            .let_statements()
            .next()
            .expect("let statement")
            .initializer()
            .expect("initializer");
        let literal = SyntaxLiteral::cast(initializer.syntax().clone()).expect("literal expr");
        let source_text = r#"f"hello {name} {player.level + 1} {{ok}}""#;

        assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
        assert_eq!(literal.syntax().text().to_string(), source_text);
        assert_eq!(literal.token_text(), Some(source_text.to_owned()));

        let interpolations = literal.interpolations().collect::<Vec<_>>();
        assert_eq!(interpolations.len(), 2);
        assert!(interpolations[0].l_brace_token().is_some());
        assert!(interpolations[0].r_brace_token().is_some());

        let expressions = literal.interpolation_expressions().collect::<Vec<_>>();
        assert_eq!(expressions.len(), 2);
        assert_eq!(expressions[0].expression_kind(), SyntaxExpressionKind::Path);
        assert_eq!(expressions[0].syntax().text().to_string(), "name");
        assert_eq!(
            expressions[1].expression_kind(),
            SyntaxExpressionKind::Binary
        );
        assert_eq!(
            expressions[1].syntax().text().to_string(),
            "player.level + 1"
        );
    }
}
