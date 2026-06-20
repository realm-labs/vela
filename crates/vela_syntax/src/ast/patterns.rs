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
    pub fn pattern_kind(&self) -> Option<SyntaxPatternKind> {
        match self.syntax.kind() {
            SyntaxKind::TuplePattern => Some(SyntaxPatternKind::TupleVariant),
            SyntaxKind::RecordPattern => Some(SyntaxPatternKind::RecordVariant),
            SyntaxKind::Pattern if self.is_wildcard() => Some(SyntaxPatternKind::Wildcard),
            SyntaxKind::Pattern if self.literal_text().is_some() => {
                Some(SyntaxPatternKind::Literal)
            }
            SyntaxKind::Pattern if self.binding_name().is_some() => {
                Some(SyntaxPatternKind::Binding)
            }
            SyntaxKind::Pattern if self.path_text().is_some() => Some(SyntaxPatternKind::Path),
            _ => None,
        }
    }

    #[must_use]
    pub fn tuple_pattern(&self) -> Option<SyntaxTuplePattern> {
        SyntaxTuplePattern::cast(self.syntax.clone())
    }

    #[must_use]
    pub fn record_pattern(&self) -> Option<SyntaxRecordPattern> {
        SyntaxRecordPattern::cast(self.syntax.clone())
    }

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
    pub fn path_text(&self) -> Option<String> {
        path_text_before_payload(&self.syntax)
    }

    #[must_use]
    pub fn literal_text(&self) -> Option<String> {
        let token = first_significant_token(&self.syntax)?;
        literal_token(token.kind()).then(|| self.syntax.text().to_string())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxPatternKind {
    Wildcard,
    Literal,
    Binding,
    Path,
    TupleVariant,
    RecordVariant,
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

fn path_text_before_payload(parent: &SyntaxNode) -> Option<String> {
    let mut path = String::new();
    let mut has_path_separator = false;
    for token in significant_tokens(parent) {
        match token.kind() {
            SyntaxKind::LParen | SyntaxKind::LBrace => break,
            SyntaxKind::ColonColon => has_path_separator = true,
            _ => {}
        }
        path.push_str(token.text());
    }
    (has_path_separator && !path.is_empty()).then_some(path)
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

#[cfg(test)]
mod tests {
    use crate::ast::{AstNode, SyntaxMatchExpr, SyntaxPatternKind};
    use crate::parse::parse_source;

    #[test]
    fn ast_basic_pattern_exposes_path_text_without_confusing_bindings() {
        let parse = parse_source(
            r#"fn update(state) {
    let value = match state {
        Option::None => 0,
        binding => 1,
        null => 2,
        _ => 3,
    };
}
"#,
        );
        let tree = parse.tree();
        let match_expr = tree
            .syntax()
            .descendants()
            .find_map(SyntaxMatchExpr::cast)
            .expect("match expression");
        let arms = match_expr
            .arm_list()
            .expect("match arm list")
            .arms()
            .collect::<Vec<_>>();

        assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
        assert_eq!(
            arms[0]
                .pattern()
                .expect("path pattern")
                .path_text()
                .as_deref(),
            Some("Option::None")
        );
        assert_eq!(
            arms[1]
                .pattern()
                .expect("binding pattern")
                .binding_name()
                .as_deref(),
            Some("binding")
        );
        assert!(
            arms[1]
                .pattern()
                .expect("binding pattern")
                .path_text()
                .is_none()
        );
        assert!(
            arms[2]
                .pattern()
                .expect("literal pattern")
                .path_text()
                .is_none()
        );
        assert!(
            arms[3]
                .pattern()
                .expect("wildcard pattern")
                .path_text()
                .is_none()
        );
    }

    #[test]
    fn ast_pattern_classifies_owned_pattern_surface() {
        let parse = parse_source(
            r#"fn update(state) {
    let value = match state {
        _ => 0,
        "ready" => 1,
        binding => 2,
        Option::None => 3,
        Option::Some(payload) => 4,
        Result::Err { error } => 5,
    };
}
"#,
        );
        let tree = parse.tree();
        let match_expr = tree
            .syntax()
            .descendants()
            .find_map(SyntaxMatchExpr::cast)
            .expect("match expression");
        let arms = match_expr
            .arm_list()
            .expect("match arm list")
            .arms()
            .collect::<Vec<_>>();
        let pattern_kinds = arms
            .iter()
            .map(|arm| arm.pattern().expect("pattern").pattern_kind())
            .collect::<Vec<_>>();

        assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
        assert_eq!(
            pattern_kinds,
            vec![
                Some(SyntaxPatternKind::Wildcard),
                Some(SyntaxPatternKind::Literal),
                Some(SyntaxPatternKind::Binding),
                Some(SyntaxPatternKind::Path),
                Some(SyntaxPatternKind::TupleVariant),
                Some(SyntaxPatternKind::RecordVariant),
            ]
        );

        let tuple_pattern = arms[4]
            .pattern()
            .expect("tuple pattern")
            .tuple_pattern()
            .expect("typed tuple pattern");
        assert_eq!(tuple_pattern.path_text().as_deref(), Some("Option::Some"));
        assert_eq!(tuple_pattern.patterns().count(), 1);

        let record_pattern = arms[5]
            .pattern()
            .expect("record pattern")
            .record_pattern()
            .expect("typed record pattern");
        assert_eq!(record_pattern.path_text().as_deref(), Some("Result::Err"));
        assert_eq!(record_pattern.fields().count(), 1);
    }
}
