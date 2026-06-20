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
    pub fn as_tuple_variant(&self) -> Option<SyntaxTuplePattern> {
        SyntaxTuplePattern::cast(self.syntax.clone())
    }

    #[must_use]
    pub fn as_record_variant(&self) -> Option<SyntaxRecordPattern> {
        SyntaxRecordPattern::cast(self.syntax.clone())
    }

    #[must_use]
    pub fn tuple_pattern(&self) -> Option<SyntaxTuplePattern> {
        self.as_tuple_variant()
    }

    #[must_use]
    pub fn record_pattern(&self) -> Option<SyntaxRecordPattern> {
        self.as_record_variant()
    }

    #[must_use]
    pub fn is_wildcard(&self) -> bool {
        self.wildcard_token().is_some()
    }

    #[must_use]
    pub fn wildcard_token(&self) -> Option<SyntaxToken> {
        let mut tokens = significant_tokens(&self.syntax);
        let token = tokens.next()?;
        (tokens.next().is_none() && token.kind() == SyntaxKind::Ident && token.text() == "_")
            .then_some(token)
    }

    #[must_use]
    pub fn binding_name(&self) -> Option<String> {
        self.binding_name_token()
            .map(|token| token.text().to_owned())
    }

    #[must_use]
    pub fn binding_name_token(&self) -> Option<SyntaxToken> {
        let mut tokens = significant_tokens(&self.syntax);
        let token = tokens.next()?;
        (tokens.next().is_none() && token.kind() == SyntaxKind::Ident && token.text() != "_")
            .then_some(token)
    }

    #[must_use]
    pub fn is_binding(&self) -> bool {
        self.binding_name_token().is_some()
    }

    #[must_use]
    pub fn path_text(&self) -> Option<String> {
        path_text_before_payload(&self.syntax)
    }

    #[must_use]
    pub fn path_tokens(&self) -> Vec<SyntaxToken> {
        path_tokens_before_payload(&self.syntax)
    }

    #[must_use]
    pub fn path_separator_tokens(&self) -> Vec<SyntaxToken> {
        path_separator_tokens_from_tokens(&self.path_tokens())
    }

    #[must_use]
    pub fn path_segments(&self) -> Vec<String> {
        path_segments_from_tokens(&self.path_tokens())
    }

    #[must_use]
    pub fn is_path(&self) -> bool {
        self.path_text().is_some()
    }

    #[must_use]
    pub fn literal_token(&self) -> Option<SyntaxToken> {
        first_significant_token(&self.syntax).filter(|token| literal_token_kind(token.kind()))
    }

    #[must_use]
    pub fn literal_token_kind(&self) -> Option<SyntaxKind> {
        self.literal_token().map(|token| token.kind())
    }

    #[must_use]
    pub fn literal_text(&self) -> Option<String> {
        self.literal_token().map(|token| token.text().to_owned())
    }

    #[must_use]
    pub fn is_literal(&self) -> bool {
        self.literal_token().is_some()
    }

    #[must_use]
    pub fn tuple_separator_token(&self) -> Option<SyntaxToken> {
        following_separator_token(
            &self.syntax,
            SyntaxKind::TuplePattern,
            SyntaxKind::Comma,
            SyntaxKind::Pattern,
            SyntaxKind::RParen,
        )
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
    pub fn path_tokens(&self) -> Vec<SyntaxToken> {
        path_tokens_before(&self.syntax, SyntaxKind::LParen)
    }

    #[must_use]
    pub fn path_separator_tokens(&self) -> Vec<SyntaxToken> {
        path_separator_tokens_from_tokens(&self.path_tokens())
    }

    #[must_use]
    pub fn path_segments(&self) -> Vec<String> {
        path_segments_from_tokens(&self.path_tokens())
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
    pub fn patterns(&self) -> AstChildren<SyntaxPattern> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn separator_tokens(&self) -> Vec<SyntaxToken> {
        separator_tokens(&self.syntax, SyntaxKind::Comma)
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
    pub fn path_tokens(&self) -> Vec<SyntaxToken> {
        path_tokens_before(&self.syntax, SyntaxKind::LBrace)
    }

    #[must_use]
    pub fn path_separator_tokens(&self) -> Vec<SyntaxToken> {
        path_separator_tokens_from_tokens(&self.path_tokens())
    }

    #[must_use]
    pub fn path_segments(&self) -> Vec<String> {
        path_segments_from_tokens(&self.path_tokens())
    }

    #[must_use]
    pub fn l_brace_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::LBrace)
    }

    #[must_use]
    pub fn r_brace_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::RBrace)
    }

    #[must_use]
    pub fn fields(&self) -> AstChildren<SyntaxRecordPatternField> {
        AstChildren::new(&self.syntax)
    }

    #[must_use]
    pub fn separator_tokens(&self) -> Vec<SyntaxToken> {
        separator_tokens(&self.syntax, SyntaxKind::Comma)
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
    pub fn label_token(&self) -> Option<SyntaxToken> {
        first_significant_token(&self.syntax).filter(|token| token.kind() == SyntaxKind::Ident)
    }

    #[must_use]
    pub fn label_kind(&self) -> Option<SyntaxKind> {
        self.label_token().map(|token| token.kind())
    }

    #[must_use]
    pub fn label_text(&self) -> Option<String> {
        self.label_token().map(|token| token.text().to_owned())
    }

    #[must_use]
    pub fn shorthand_binding_name_token(&self) -> Option<SyntaxToken> {
        self.is_shorthand().then(|| self.label_token()).flatten()
    }

    #[must_use]
    pub fn shorthand_binding_name(&self) -> Option<String> {
        self.shorthand_binding_name_token()
            .map(|token| token.text().to_owned())
    }

    #[must_use]
    pub fn colon_token(&self) -> Option<SyntaxToken> {
        token(&self.syntax, SyntaxKind::Colon)
    }

    #[must_use]
    pub fn pattern(&self) -> Option<SyntaxPattern> {
        child(&self.syntax)
    }

    #[must_use]
    pub fn is_shorthand(&self) -> bool {
        self.label_token().is_some() && self.colon_token().is_none() && self.pattern().is_none()
    }

    #[must_use]
    pub fn separator_token(&self) -> Option<SyntaxToken> {
        following_separator_token(
            &self.syntax,
            SyntaxKind::RecordPattern,
            SyntaxKind::Comma,
            SyntaxKind::RecordPatternField,
            SyntaxKind::RBrace,
        )
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

fn following_separator_token(
    node: &SyntaxNode,
    parent_kind: SyntaxKind,
    separator_kind: SyntaxKind,
    next_node_kind: SyntaxKind,
    close_kind: SyntaxKind,
) -> Option<SyntaxToken> {
    let parent = node.parent()?;
    if parent.kind() != parent_kind {
        return None;
    }

    let mut seen_node = false;
    for element in parent.children_with_tokens() {
        if let Some(child) = element.as_node() {
            if child == node {
                seen_node = true;
                continue;
            }
            if seen_node
                && (child.kind() == next_node_kind
                    || (parent_kind == SyntaxKind::TuplePattern
                        && SyntaxPattern::can_cast(child.kind())))
            {
                return None;
            }
        }

        let Some(token) = element.as_token() else {
            continue;
        };
        if !seen_node {
            continue;
        }
        if token.kind() == separator_kind {
            return Some(token.clone());
        }
        if token.kind() == close_kind {
            return None;
        }
    }
    None
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
    let tokens = path_tokens_before(parent, delimiter);
    path_text_from_tokens(&tokens)
}

fn path_text_before_payload(parent: &SyntaxNode) -> Option<String> {
    let tokens = path_tokens_before_payload(parent);
    path_text_from_tokens(&tokens)
}

fn path_text_from_tokens(tokens: &[SyntaxToken]) -> Option<String> {
    let mut path = String::new();
    for token in tokens {
        path.push_str(token.text());
    }
    (!path.is_empty()).then_some(path)
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

fn path_tokens_before(parent: &SyntaxNode, delimiter: SyntaxKind) -> Vec<SyntaxToken> {
    let mut tokens = Vec::new();
    for token in significant_tokens(parent) {
        if token.kind() == delimiter {
            break;
        }
        tokens.push(token);
    }
    tokens
}

fn path_tokens_before_payload(parent: &SyntaxNode) -> Vec<SyntaxToken> {
    let mut tokens = Vec::new();
    let mut has_path_separator = false;
    for token in significant_tokens(parent) {
        match token.kind() {
            SyntaxKind::LParen | SyntaxKind::LBrace => break,
            SyntaxKind::ColonColon => has_path_separator = true,
            _ => {}
        }
        tokens.push(token);
    }
    if has_path_separator {
        tokens
    } else {
        Vec::new()
    }
}

const fn literal_token_kind(kind: SyntaxKind) -> bool {
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
    use crate::SyntaxKind;
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
            arms[0]
                .pattern()
                .expect("path pattern")
                .path_tokens()
                .iter()
                .map(|token| (token.kind(), token.text().to_owned()))
                .collect::<Vec<_>>(),
            vec![
                (SyntaxKind::Ident, "Option".to_owned()),
                (SyntaxKind::ColonColon, "::".to_owned()),
                (SyntaxKind::Ident, "None".to_owned()),
            ]
        );
        assert_eq!(
            arms[0].pattern().expect("path pattern").path_segments(),
            vec!["Option".to_owned(), "None".to_owned()]
        );
        assert_eq!(
            arms[0]
                .pattern()
                .expect("path pattern")
                .path_separator_tokens()
                .iter()
                .map(|token| token.text().to_owned())
                .collect::<Vec<_>>(),
            vec!["::"]
        );
        assert_eq!(
            arms[1]
                .pattern()
                .expect("binding pattern")
                .binding_name()
                .as_deref(),
            Some("binding")
        );
        assert_eq!(
            arms[1]
                .pattern()
                .expect("binding pattern")
                .binding_name_token()
                .expect("binding name token")
                .text(),
            "binding"
        );
        assert!(
            arms[1]
                .pattern()
                .expect("binding pattern")
                .path_text()
                .is_none()
        );
        assert!(
            arms[1]
                .pattern()
                .expect("binding pattern")
                .path_separator_tokens()
                .is_empty()
        );
        assert!(
            arms[1]
                .pattern()
                .expect("binding pattern")
                .path_segments()
                .is_empty()
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
        assert_eq!(
            arms[3]
                .pattern()
                .expect("wildcard pattern")
                .wildcard_token()
                .expect("wildcard token")
                .text(),
            "_"
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
        Option::Some(payload, fallback) => 4,
        Result::Err { error, code } => 5,
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
        let patterns = arms
            .iter()
            .map(|arm| arm.pattern().expect("pattern"))
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
        assert!(patterns[0].is_wildcard());
        assert!(patterns[1].is_literal());
        assert!(patterns[2].is_binding());
        assert!(patterns[3].is_path());
        assert!(patterns[4].as_tuple_variant().is_some());
        assert!(patterns[5].as_record_variant().is_some());
        assert!(patterns[0].as_tuple_variant().is_none());
        assert!(!patterns[1].is_binding());

        let literal_pattern = arms[1].pattern().expect("literal pattern");
        assert_eq!(
            literal_pattern
                .literal_token()
                .expect("literal pattern token")
                .text(),
            "\"ready\""
        );
        assert_eq!(
            literal_pattern.literal_token_kind(),
            Some(SyntaxKind::String)
        );
        assert_eq!(literal_pattern.literal_text().as_deref(), Some("\"ready\""));
        assert!(
            arms[0]
                .pattern()
                .expect("wildcard pattern")
                .literal_token()
                .is_none()
        );
        assert_eq!(
            arms[0]
                .pattern()
                .expect("wildcard pattern")
                .wildcard_token()
                .expect("wildcard token")
                .kind(),
            SyntaxKind::Ident
        );

        let binding_pattern = arms[2].pattern().expect("binding pattern");
        assert_eq!(
            binding_pattern
                .binding_name_token()
                .expect("binding name token")
                .kind(),
            SyntaxKind::Ident
        );
        assert_eq!(binding_pattern.binding_name().as_deref(), Some("binding"));

        let tuple_pattern = arms[4]
            .pattern()
            .expect("tuple pattern")
            .tuple_pattern()
            .expect("typed tuple pattern");
        assert_eq!(tuple_pattern.path_text().as_deref(), Some("Option::Some"));
        assert_eq!(
            tuple_pattern.path_segments(),
            vec!["Option".to_owned(), "Some".to_owned()]
        );
        assert_eq!(
            tuple_pattern
                .path_separator_tokens()
                .iter()
                .map(|token| token.text().to_owned())
                .collect::<Vec<_>>(),
            vec!["::"]
        );
        assert_eq!(
            tuple_pattern
                .path_tokens()
                .iter()
                .map(|token| (token.kind(), token.text().to_owned()))
                .collect::<Vec<_>>(),
            vec![
                (SyntaxKind::Ident, "Option".to_owned()),
                (SyntaxKind::ColonColon, "::".to_owned()),
                (SyntaxKind::Ident, "Some".to_owned()),
            ]
        );
        assert_eq!(
            tuple_pattern
                .l_paren_token()
                .expect("tuple pattern open")
                .kind(),
            SyntaxKind::LParen
        );
        assert_eq!(
            tuple_pattern
                .r_paren_token()
                .expect("tuple pattern close")
                .kind(),
            SyntaxKind::RParen
        );
        assert_eq!(tuple_pattern.patterns().count(), 2);
        assert_eq!(
            tuple_pattern
                .separator_tokens()
                .iter()
                .map(|token| token.text().to_owned())
                .collect::<Vec<_>>(),
            vec![","]
        );
        let tuple_fields = tuple_pattern.patterns().collect::<Vec<_>>();
        assert_eq!(
            tuple_fields[0]
                .tuple_separator_token()
                .expect("tuple payload separator")
                .text(),
            ","
        );
        assert!(tuple_fields[1].tuple_separator_token().is_none());

        let record_pattern = arms[5]
            .pattern()
            .expect("record pattern")
            .record_pattern()
            .expect("typed record pattern");
        assert_eq!(record_pattern.path_text().as_deref(), Some("Result::Err"));
        assert_eq!(
            record_pattern.path_segments(),
            vec!["Result".to_owned(), "Err".to_owned()]
        );
        assert_eq!(
            record_pattern
                .path_separator_tokens()
                .iter()
                .map(|token| token.text().to_owned())
                .collect::<Vec<_>>(),
            vec!["::"]
        );
        assert_eq!(
            record_pattern
                .path_tokens()
                .iter()
                .map(|token| (token.kind(), token.text().to_owned()))
                .collect::<Vec<_>>(),
            vec![
                (SyntaxKind::Ident, "Result".to_owned()),
                (SyntaxKind::ColonColon, "::".to_owned()),
                (SyntaxKind::Ident, "Err".to_owned()),
            ]
        );
        assert_eq!(
            record_pattern
                .l_brace_token()
                .expect("record pattern open")
                .kind(),
            SyntaxKind::LBrace
        );
        assert_eq!(
            record_pattern
                .r_brace_token()
                .expect("record pattern close")
                .kind(),
            SyntaxKind::RBrace
        );
        let fields = record_pattern.fields().collect::<Vec<_>>();
        assert_eq!(fields.len(), 2);
        assert_eq!(
            record_pattern
                .separator_tokens()
                .iter()
                .map(|token| token.text().to_owned())
                .collect::<Vec<_>>(),
            vec![","]
        );
        assert_eq!(fields[0].label_kind(), Some(SyntaxKind::Ident));
        assert_eq!(
            fields[0].label_token().expect("field label").text(),
            "error"
        );
        assert_eq!(
            fields[0]
                .separator_token()
                .expect("record field separator")
                .text(),
            ","
        );
        assert!(fields[0].colon_token().is_none());
        assert!(fields[0].is_shorthand());
        assert_eq!(fields[1].label_text().as_deref(), Some("code"));
        assert!(fields[1].separator_token().is_none());
    }

    #[test]
    fn ast_record_pattern_fields_expose_labels_and_explicit_payloads() {
        let parse = parse_source(
            r#"fn update(result) {
    let value = match result {
        Result::Err { error: reason, code } => 1,
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
        let arm = match_expr
            .arm_list()
            .expect("match arm list")
            .arms()
            .next()
            .expect("match arm");
        let record_pattern = arm
            .pattern()
            .expect("record pattern")
            .record_pattern()
            .expect("typed record pattern");
        let fields = record_pattern.fields().collect::<Vec<_>>();

        assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].label_text().as_deref(), Some("error"));
        assert_eq!(fields[0].label_kind(), Some(SyntaxKind::Ident));
        assert_eq!(
            fields[0]
                .colon_token()
                .expect("explicit field colon")
                .kind(),
            SyntaxKind::Colon
        );
        assert_eq!(
            fields[0]
                .pattern()
                .expect("explicit field payload")
                .binding_name()
                .as_deref(),
            Some("reason")
        );
        assert!(!fields[0].is_shorthand());
        assert!(fields[0].shorthand_binding_name_token().is_none());
        assert!(fields[0].shorthand_binding_name().is_none());

        assert_eq!(fields[1].label_text().as_deref(), Some("code"));
        assert!(fields[1].colon_token().is_none());
        assert!(fields[1].pattern().is_none());
        assert!(fields[1].is_shorthand());
        assert_eq!(
            fields[1]
                .shorthand_binding_name_token()
                .expect("shorthand binding token")
                .text(),
            "code"
        );
        assert_eq!(fields[1].shorthand_binding_name().as_deref(), Some("code"));
    }
}
