use std::collections::BTreeMap;

use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingMap, BindingResolution, LocalBinding, LocalBindingKind};
use vela_hir::module_graph::{Declaration, DeclarationKind};
use vela_syntax::lexer::lex;
use vela_syntax::token::{Keyword, Symbol, Token, TokenKind};

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
    Comment,
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
    pub const LEGEND: [Self; 17] = [
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
        Self::Comment,
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
            Self::Comment => "comment",
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

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct SemanticTokenClassification {
    token_type: SemanticTokenType,
    modifiers: SemanticTokenModifiers,
}

impl SemanticTokenClassification {
    const fn new(token_type: SemanticTokenType, modifiers: SemanticTokenModifiers) -> Self {
        Self {
            token_type,
            modifiers,
        }
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
        let classifications =
            self.semantic_token_classifications(source.source_id(), source.text(), &lexed.tokens);
        let mut semantic_tokens = Vec::new();

        for range in comment_ranges(source.text(), &lexed.tokens) {
            push_semantic_token_slices(
                source.text(),
                &line_index,
                range,
                SemanticTokenType::Comment,
                SemanticTokenModifiers::NONE,
                &mut semantic_tokens,
            );
        }

        for token in lexed.tokens {
            let Some(range) = token_range(token.span) else {
                continue;
            };
            let classification = classifications
                .get(&(range.start, range.end))
                .copied()
                .or_else(|| {
                    token_type(&token.kind).map(|token_type| {
                        SemanticTokenClassification::new(token_type, SemanticTokenModifiers::NONE)
                    })
                });
            let Some(classification) = classification else {
                continue;
            };
            push_semantic_token_slices(
                source.text(),
                &line_index,
                range,
                classification.token_type,
                classification.modifiers,
                &mut semantic_tokens,
            );
        }

        semantic_tokens.sort_by_key(|token| {
            let start = token.start();
            (start.line, start.character)
        });
        SemanticTokens::new(semantic_tokens)
    }

    fn semantic_token_classifications(
        &self,
        source_id: SourceId,
        text: &str,
        tokens: &[Token],
    ) -> BTreeMap<(usize, usize), SemanticTokenClassification> {
        let mut classifications = BTreeMap::new();
        for token in tokens {
            let TokenKind::Ident(name) = &token.kind else {
                continue;
            };
            let Some(range) = token_range(token.span) else {
                continue;
            };
            if let Some(classification) =
                self.semantic_classification_for_identifier(source_id, text, name, range)
            {
                classifications.insert((range.start, range.end), classification);
            }
        }
        classifications
    }

    fn semantic_classification_for_identifier(
        &self,
        source_id: SourceId,
        text: &str,
        name: &str,
        range: TextRange,
    ) -> Option<SemanticTokenClassification> {
        let span = span_for_range(source_id, range)?;
        let graph = self.hir_db().graph();

        for declaration in graph.declarations() {
            if declaration.span.source != source_id || !declaration.span.contains(span.start) {
                continue;
            }
            if let Some(bindings) = graph.bindings(declaration.id) {
                if let Some(classification) =
                    local_declaration_classification(bindings, name, range)
                {
                    return Some(classification);
                }
                if let Some(classification) =
                    resolved_identifier_classification(bindings, span, self)
                {
                    return Some(classification);
                }
            }
        }

        graph
            .declarations()
            .find(|declaration| {
                declaration.span.source == source_id
                    && declaration.span.contains(span.start)
                    && declaration.name == name
                    && token_text(text, range) == Some(name)
            })
            .map(declaration_classification)
    }
}

fn local_declaration_classification(
    bindings: &BindingMap,
    name: &str,
    range: TextRange,
) -> Option<SemanticTokenClassification> {
    bindings
        .locals()
        .find(|binding| binding.name == name && span_contains_range(binding.span, range))
        .map(local_declaration_token_classification)
}

fn resolved_identifier_classification(
    bindings: &BindingMap,
    span: Span,
    databases: &LanguageServiceDatabases,
) -> Option<SemanticTokenClassification> {
    let resolution = bindings.resolution_at_span(span)?;
    match resolution {
        BindingResolution::Local(local) => bindings.local(*local).map(local_use_classification),
        BindingResolution::Declaration(declaration) => databases
            .hir_db()
            .graph()
            .declaration(*declaration)
            .map(declaration_use_classification),
        BindingResolution::Import(_) | BindingResolution::QualifiedPath(_) => {
            Some(SemanticTokenClassification::new(
                SemanticTokenType::Variable,
                SemanticTokenModifiers::UNRESOLVED,
            ))
        }
    }
}

fn local_declaration_token_classification(binding: &LocalBinding) -> SemanticTokenClassification {
    SemanticTokenClassification::new(
        local_token_type(binding.kind),
        SemanticTokenModifiers::DECLARATION,
    )
}

fn local_use_classification(binding: &LocalBinding) -> SemanticTokenClassification {
    SemanticTokenClassification::new(local_token_type(binding.kind), SemanticTokenModifiers::NONE)
}

fn local_token_type(kind: LocalBindingKind) -> SemanticTokenType {
    match kind {
        LocalBindingKind::Parameter | LocalBindingKind::LambdaParameter => {
            SemanticTokenType::Parameter
        }
        LocalBindingKind::Let | LocalBindingKind::For | LocalBindingKind::Pattern => {
            SemanticTokenType::Variable
        }
    }
}

fn declaration_classification(declaration: &Declaration) -> SemanticTokenClassification {
    SemanticTokenClassification::new(
        declaration_token_type(declaration.kind),
        SemanticTokenModifiers::DECLARATION.union(SemanticTokenModifiers::DEFINITION),
    )
}

fn declaration_use_classification(declaration: &Declaration) -> SemanticTokenClassification {
    let modifiers = if matches!(declaration.kind, DeclarationKind::Const) {
        SemanticTokenModifiers::READONLY
    } else {
        SemanticTokenModifiers::NONE
    };
    SemanticTokenClassification::new(declaration_token_type(declaration.kind), modifiers)
}

fn declaration_token_type(kind: DeclarationKind) -> SemanticTokenType {
    match kind {
        DeclarationKind::Const | DeclarationKind::Global => SemanticTokenType::Variable,
        DeclarationKind::Function => SemanticTokenType::Function,
        DeclarationKind::Struct | DeclarationKind::Enum | DeclarationKind::Trait => {
            SemanticTokenType::Type
        }
        DeclarationKind::Impl => SemanticTokenType::Type,
    }
}

fn span_contains_range(span: Span, range: TextRange) -> bool {
    let Ok(start) = u32::try_from(range.start) else {
        return false;
    };
    let Ok(end) = u32::try_from(range.end) else {
        return false;
    };
    span.start <= start && end <= span.end
}

fn span_for_range(source_id: SourceId, range: TextRange) -> Option<Span> {
    let start = u32::try_from(range.start).ok()?;
    let end = u32::try_from(range.end).ok()?;
    Some(Span::new(source_id, start, end))
}

fn token_text(text: &str, range: TextRange) -> Option<&str> {
    text.get(range.start..range.end)
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

fn comment_ranges(text: &str, tokens: &[Token]) -> Vec<TextRange> {
    let token_ranges = sorted_token_ranges(tokens);
    let mut ranges = Vec::new();
    let mut token_index = 0;
    let mut offset = 0;

    if text.starts_with("#!") {
        let end = text.find('\n').unwrap_or(text.len());
        ranges.push(TextRange::new(0, end));
        offset = end;
    }

    while offset < text.len() {
        while token_index < token_ranges.len() && token_ranges[token_index].end <= offset {
            token_index += 1;
        }

        if token_index < token_ranges.len() && token_ranges[token_index].start == offset {
            offset = token_ranges[token_index].end;
            token_index += 1;
            continue;
        }

        let Some(rest) = text.get(offset..) else {
            break;
        };
        if rest.starts_with("//") {
            let end = offset + rest.find('\n').unwrap_or(rest.len());
            ranges.push(TextRange::new(offset, end));
            offset = end;
            continue;
        }
        if rest.starts_with("/*") {
            let end = block_comment_end(text, offset);
            ranges.push(TextRange::new(offset, end));
            offset = end;
            continue;
        }

        offset += rest.chars().next().map_or(1, |ch| ch.len_utf8());
    }

    ranges
}

fn sorted_token_ranges(tokens: &[Token]) -> Vec<TextRange> {
    let mut ranges: Vec<_> = tokens
        .iter()
        .filter_map(|token| token_range(token.span))
        .collect();
    ranges.sort_by_key(|range| (range.start, range.end));
    ranges
}

fn block_comment_end(text: &str, start: usize) -> usize {
    let mut offset = start + "/*".len();
    let mut depth = 1_u32;

    while offset < text.len() {
        let Some(rest) = text.get(offset..) else {
            return text.len();
        };
        if rest.starts_with("/*") {
            depth = depth.saturating_add(1);
            offset += "/*".len();
        } else if rest.starts_with("*/") {
            depth = depth.saturating_sub(1);
            offset += "*/".len();
            if depth == 0 {
                return offset;
            }
        } else {
            offset += rest.chars().next().map_or(1, |ch| ch.len_utf8());
        }
    }

    text.len()
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

        assert_token_at(
            &tokens,
            0,
            text.find("pub").expect("keyword should exist"),
            "pub".len(),
            SemanticTokenType::Keyword,
            SemanticTokenModifiers::NONE,
        );
        assert_token_at(
            &tokens,
            0,
            text.find("b\"ok\"").expect("bytes literal should exist"),
            "b\"ok\"".len(),
            SemanticTokenType::Bytes,
            SemanticTokenModifiers::NONE,
        );
        assert_token_at(
            &tokens,
            0,
            text.find('+').expect("operator should exist"),
            1,
            SemanticTokenType::Operator,
            SemanticTokenModifiers::NONE,
        );
        assert_token_at(
            &tokens,
            0,
            text.find('1').expect("number should exist"),
            1,
            SemanticTokenType::Number,
            SemanticTokenModifiers::NONE,
        );
    }

    #[test]
    fn semantic_tokens_mark_resolved_symbols() {
        let main = DocumentId::from("/workspace/scripts/game/main.vela");
        let helper = DocumentId::from("/workspace/scripts/game/reward.vela");
        let main_text = "\
use game::reward::grant
pub fn main(amount: i64) -> i64 {
    let next = grant(amount)
    return next
}";
        let databases = databases_for(vec![
            SourceFileSnapshot::new(main.clone(), main_text),
            SourceFileSnapshot::new(helper, "pub fn grant(amount: i64) -> i64 { return amount }"),
        ]);

        let tokens = databases.semantic_tokens(&main);

        assert_token_at(
            &tokens,
            1,
            line(main_text, 1).find("main").expect("main should exist"),
            "main".len(),
            SemanticTokenType::Function,
            SemanticTokenModifiers::DECLARATION.union(SemanticTokenModifiers::DEFINITION),
        );
        assert_token_at(
            &tokens,
            1,
            line(main_text, 1)
                .find("amount")
                .expect("parameter should exist"),
            "amount".len(),
            SemanticTokenType::Parameter,
            SemanticTokenModifiers::DECLARATION,
        );
        assert_token_at(
            &tokens,
            2,
            line(main_text, 2).find("next").expect("local should exist"),
            "next".len(),
            SemanticTokenType::Variable,
            SemanticTokenModifiers::DECLARATION,
        );
        assert_token_at(
            &tokens,
            2,
            line(main_text, 2).find("grant").expect("call should exist"),
            "grant".len(),
            SemanticTokenType::Function,
            SemanticTokenModifiers::NONE,
        );
        assert_token_at(
            &tokens,
            2,
            line(main_text, 2)
                .find("amount")
                .expect("argument should exist"),
            "amount".len(),
            SemanticTokenType::Parameter,
            SemanticTokenModifiers::NONE,
        );
        assert_token_at(
            &tokens,
            3,
            line(main_text, 3)
                .find("next")
                .expect("return value should exist"),
            "next".len(),
            SemanticTokenType::Variable,
            SemanticTokenModifiers::NONE,
        );
    }

    #[test]
    fn semantic_tokens_include_comments() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "\
#! /usr/bin/env vela
// setup
pub fn main() {
    let text = \"not // a comment\"
    /* outer
       /* nested */
       done */
    return text
}";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

        let tokens = databases.semantic_tokens(&document);

        assert_token_at(
            &tokens,
            0,
            0,
            line(text, 0).len(),
            SemanticTokenType::Comment,
            SemanticTokenModifiers::NONE,
        );
        assert_token_at(
            &tokens,
            1,
            0,
            line(text, 1).len(),
            SemanticTokenType::Comment,
            SemanticTokenModifiers::NONE,
        );
        assert_token_at(
            &tokens,
            4,
            line(text, 4)
                .find("/* outer")
                .expect("block comment should exist"),
            "/* outer".len(),
            SemanticTokenType::Comment,
            SemanticTokenModifiers::NONE,
        );
        assert_token_at(
            &tokens,
            5,
            0,
            line(text, 5).len(),
            SemanticTokenType::Comment,
            SemanticTokenModifiers::NONE,
        );
        assert_token_at(
            &tokens,
            6,
            0,
            line(text, 6).len(),
            SemanticTokenType::Comment,
            SemanticTokenModifiers::NONE,
        );
        assert!(
            tokens.tokens().iter().all(|token| {
                token.start().line != 3
                    || token.token_type() != SemanticTokenType::Comment
                    || token.start().character
                        != line(text, 3)
                            .find("//")
                            .expect("string should contain comment marker")
            }),
            "string contents must not produce comment tokens: {:?}",
            tokens.tokens()
        );
    }

    fn assert_token_at(
        tokens: &SemanticTokens,
        line: usize,
        character: usize,
        length: usize,
        token_type: SemanticTokenType,
        modifiers: SemanticTokenModifiers,
    ) {
        assert!(
            tokens.tokens().iter().any(|token| {
                token.start().line == line
                    && token.start().character == character
                    && token.length() == length
                    && token.token_type() == token_type
                    && token.modifiers() == modifiers
            }),
            "{:?}",
            tokens.tokens()
        );
    }

    fn line(text: &str, line: usize) -> &str {
        text.lines().nth(line).expect("line should exist")
    }

    fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        databases
    }
}
