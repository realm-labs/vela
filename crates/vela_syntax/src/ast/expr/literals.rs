use vela_common::SourceId;

use crate::ast::{AstNode, Literal};
use crate::lexer::lex;
use crate::token::TokenKind;
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
        self.token().map(|token| token.text().to_owned())
    }

    #[must_use]
    pub fn literal(&self) -> Option<Literal> {
        let token = self.token()?;
        match token.kind() {
            SyntaxKind::TrueKw => Some(Literal::Bool(true)),
            SyntaxKind::FalseKw => Some(Literal::Bool(false)),
            SyntaxKind::NullKw => Some(Literal::Null),
            SyntaxKind::Int
            | SyntaxKind::Float
            | SyntaxKind::Char
            | SyntaxKind::String
            | SyntaxKind::Bytes => literal_from_token_text(token.text()),
            SyntaxKind::InterpolatedString => None,
            _ => None,
        }
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

fn literal_from_token_text(text: &str) -> Option<Literal> {
    lex(SourceId::new(0), text)
        .tokens
        .into_iter()
        .find_map(|token| match token.kind {
            TokenKind::Int(value) => Some(Literal::Integer(value)),
            TokenKind::Float(value) => Some(Literal::Float(value)),
            TokenKind::Char(value) => Some(Literal::Char(value)),
            TokenKind::String(value) => Some(Literal::String(value)),
            TokenKind::Bytes(value) => Some(Literal::Bytes(value)),
            _ => None,
        })
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

#[cfg(test)]
mod tests {
    use crate::SyntaxKind;
    use crate::ast::{AstNode, FloatSuffix, IntRadix, IntegerSuffix, Literal, SyntaxLiteral};
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
}
