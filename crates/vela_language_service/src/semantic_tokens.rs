use vela_syntax::lexer::lex;
use vela_syntax::token::{Keyword, Symbol, TokenKind};

use crate::{DocumentId, LanguageServiceDatabases, LineIndex, Position, TextRange};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SemanticTokens {
    tokens: Vec<SemanticToken>,
}

impl SemanticTokens {
    #[must_use]
    pub fn new(tokens: Vec<SemanticToken>) -> Self {
        Self { tokens }
    }

    #[must_use]
    pub fn tokens(&self) -> &[SemanticToken] {
        &self.tokens
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct SemanticToken {
    start: Position,
    length: usize,
    token_type: SemanticTokenType,
    modifiers: SemanticTokenModifiers,
}

impl SemanticToken {
    #[must_use]
    pub const fn new(
        start: Position,
        length: usize,
        token_type: SemanticTokenType,
        modifiers: SemanticTokenModifiers,
    ) -> Self {
        Self {
            start,
            length,
            token_type,
            modifiers,
        }
    }

    #[must_use]
    pub const fn start(self) -> Position {
        self.start
    }

    #[must_use]
    pub const fn length(self) -> usize {
        self.length
    }

    #[must_use]
    pub const fn token_type(self) -> SemanticTokenType {
        self.token_type
    }

    #[must_use]
    pub const fn modifiers(self) -> SemanticTokenModifiers {
        self.modifiers
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SemanticTokenType {
    Attribute,
    Bytes,
    EnumMember,
    Field,
    Function,
    Keyword,
    Macro,
    Method,
    Module,
    Number,
    Operator,
    Parameter,
    Property,
    String,
    Type,
    Variable,
}

impl SemanticTokenType {
    pub const LEGEND: [Self; 16] = [
        Self::Module,
        Self::Function,
        Self::Method,
        Self::Field,
        Self::Variable,
        Self::Parameter,
        Self::Type,
        Self::EnumMember,
        Self::Property,
        Self::Keyword,
        Self::Number,
        Self::String,
        Self::Bytes,
        Self::Operator,
        Self::Attribute,
        Self::Macro,
    ];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Module => "namespace",
            Self::Function => "function",
            Self::Method => "method",
            Self::Field => "field",
            Self::Variable => "variable",
            Self::Parameter => "parameter",
            Self::Type => "type",
            Self::EnumMember => "enumMember",
            Self::Property => "property",
            Self::Keyword => "keyword",
            Self::Number => "number",
            Self::String => "string",
            Self::Bytes => "bytes",
            Self::Operator => "operator",
            Self::Attribute => "decorator",
            Self::Macro => "macro",
        }
    }

    #[must_use]
    pub fn legend_index(self) -> u32 {
        Self::LEGEND
            .iter()
            .position(|kind| *kind == self)
            .and_then(|index| u32::try_from(index).ok())
            .expect("semantic token type should be in legend")
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct SemanticTokenModifiers(u32);

impl SemanticTokenModifiers {
    pub const NONE: Self = Self(0);
    pub const DECLARATION: Self = Self(1 << 0);
    pub const DEFINITION: Self = Self(1 << 1);
    pub const READONLY: Self = Self(1 << 2);
    pub const DEPRECATED: Self = Self(1 << 3);
    pub const BUILTIN: Self = Self(1 << 4);
    pub const HOST: Self = Self(1 << 5);
    pub const UNRESOLVED: Self = Self(1 << 6);

    pub const LEGEND: [&'static str; 7] = [
        "declaration",
        "definition",
        "readonly",
        "deprecated",
        "defaultLibrary",
        "host",
        "unresolved",
    ];

    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn semantic_tokens(&self, document_id: &DocumentId) -> SemanticTokens {
        let Some(source) = self.source_db().records().get(document_id) else {
            return SemanticTokens::new(Vec::new());
        };
        let line_index = LineIndex::new(source.text());
        let lexed = lex(source.source_id(), source.text());
        let mut semantic_tokens = Vec::new();

        for token in lexed.tokens {
            let Some(token_type) = token_type(&token.kind) else {
                continue;
            };
            let Some(range) = token_range(token.span) else {
                continue;
            };
            push_semantic_token_slices(
                source.text(),
                &line_index,
                range,
                token_type,
                SemanticTokenModifiers::NONE,
                &mut semantic_tokens,
            );
        }

        semantic_tokens.sort_by_key(|token| {
            let start = token.start();
            (start.line, start.character)
        });
        SemanticTokens::new(semantic_tokens)
    }
}

fn token_type(kind: &TokenKind) -> Option<SemanticTokenType> {
    match kind {
        TokenKind::Keyword(keyword) => Some(keyword_token_type(*keyword)),
        TokenKind::Int(_) | TokenKind::Float(_) => Some(SemanticTokenType::Number),
        TokenKind::Char(_) | TokenKind::String(_) | TokenKind::InterpolatedString(_) => {
            Some(SemanticTokenType::String)
        }
        TokenKind::Bytes(_) => Some(SemanticTokenType::Bytes),
        TokenKind::Symbol(symbol) => symbol_token_type(*symbol),
        TokenKind::Ident(_) => Some(SemanticTokenType::Variable),
        TokenKind::Eof => None,
    }
}

fn keyword_token_type(keyword: Keyword) -> SemanticTokenType {
    match keyword {
        Keyword::True | Keyword::False | Keyword::Null => SemanticTokenType::Keyword,
        _ => SemanticTokenType::Keyword,
    }
}

fn symbol_token_type(symbol: Symbol) -> Option<SemanticTokenType> {
    match symbol {
        Symbol::Hash => Some(SemanticTokenType::Attribute),
        _ => Some(SemanticTokenType::Operator),
    }
}

fn push_semantic_token_slices(
    text: &str,
    line_index: &LineIndex,
    range: TextRange,
    token_type: SemanticTokenType,
    modifiers: SemanticTokenModifiers,
    tokens: &mut Vec<SemanticToken>,
) {
    let Some(slice) = text.get(range.start..range.end) else {
        return;
    };
    let mut line_start = range.start;
    let mut offset = range.start;

    for part in slice.split_inclusive('\n') {
        let line_end = offset + part.trim_end_matches('\n').len();
        push_non_empty_token(
            line_index, line_start, line_end, token_type, modifiers, tokens,
        );
        offset += part.len();
        line_start = offset;
    }

    if !slice.ends_with('\n') && offset < range.end {
        push_non_empty_token(line_index, offset, range.end, token_type, modifiers, tokens);
    }
}

fn push_non_empty_token(
    line_index: &LineIndex,
    start: usize,
    end: usize,
    token_type: SemanticTokenType,
    modifiers: SemanticTokenModifiers,
    tokens: &mut Vec<SemanticToken>,
) {
    if start >= end {
        return;
    }
    let position = line_index.position(start);
    let length = end.saturating_sub(start);
    tokens.push(SemanticToken::new(position, length, token_type, modifiers));
}

fn token_range(span: vela_common::Span) -> Option<TextRange> {
    let start = usize::try_from(span.start).ok()?;
    let end = usize::try_from(span.end).ok()?;
    (start < end).then(|| TextRange::new(start, end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
    };

    #[test]
    fn semantic_tokens_cover_lexical_classes() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub fn main() { let bytes = b\"ok\" return bytes + 1 }";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

        let tokens = databases.semantic_tokens(&document);

        assert_token(
            &tokens,
            text,
            "pub",
            SemanticTokenType::Keyword,
            SemanticTokenModifiers::NONE,
        );
        assert_token(
            &tokens,
            text,
            "main",
            SemanticTokenType::Variable,
            SemanticTokenModifiers::NONE,
        );
        assert_token(
            &tokens,
            text,
            "b\"ok\"",
            SemanticTokenType::Bytes,
            SemanticTokenModifiers::NONE,
        );
        assert_token(
            &tokens,
            text,
            "+",
            SemanticTokenType::Operator,
            SemanticTokenModifiers::NONE,
        );
        assert_token(
            &tokens,
            text,
            "1",
            SemanticTokenType::Number,
            SemanticTokenModifiers::NONE,
        );
    }

    fn assert_token(
        tokens: &SemanticTokens,
        text: &str,
        needle: &str,
        token_type: SemanticTokenType,
        modifiers: SemanticTokenModifiers,
    ) {
        let start = text.find(needle).expect("token text should exist");
        assert!(
            tokens.tokens().iter().any(|token| {
                token.start().line == 0
                    && token.start().character == start
                    && token.length() == needle.len()
                    && token.token_type() == token_type
                    && token.modifiers() == modifiers
            }),
            "{:?}",
            tokens.tokens()
        );
    }

    fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        databases
    }
}
