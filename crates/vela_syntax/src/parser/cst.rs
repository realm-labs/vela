use vela_common::Diagnostic;

use crate::lexer::Lexed;
use crate::token::LosslessToken;
use crate::{SyntaxKind, SyntaxTreeBuilder};

pub(crate) fn build_source_tree(lexed: &Lexed, builder: &mut SyntaxTreeBuilder) -> Vec<Diagnostic> {
    let mut parser = CstParser::new(&lexed.lossless_tokens, builder);
    parser.source_file();
    parser.diagnostics
}

struct CstParser<'tokens, 'builder> {
    tokens: &'tokens [LosslessToken],
    pos: usize,
    builder: &'builder mut SyntaxTreeBuilder,
    diagnostics: Vec<Diagnostic>,
}

impl<'tokens, 'builder> CstParser<'tokens, 'builder> {
    fn new(tokens: &'tokens [LosslessToken], builder: &'builder mut SyntaxTreeBuilder) -> Self {
        Self {
            tokens,
            pos: 0,
            builder,
            diagnostics: Vec::new(),
        }
    }

    fn source_file(&mut self) {
        self.builder.start_node(SyntaxKind::SourceFile);
        while !self.at_eof() {
            if self.current_kind().is_some_and(SyntaxKind::is_trivia) {
                self.emit_current_token();
            } else if let Some(item) = self.current_item() {
                self.item(item);
            } else {
                self.error_run();
            }
        }
        self.builder.finish_node();
    }

    fn item(&mut self, item: ItemBoundary) {
        self.builder.start_node(item.kind);
        while self.pos < item.end {
            self.emit_current_token();
        }
        self.builder.finish_node();
    }

    fn error_run(&mut self) {
        let start = self.pos;
        while !self.at_eof() {
            if self.current_kind().is_some_and(SyntaxKind::is_trivia) {
                break;
            }
            if self.pos != start && self.current_item().is_some() {
                break;
            }
            self.pos += 1;
        }

        if self.pos == start {
            self.emit_current_token();
            return;
        }

        if let Some(span) = self.tokens.get(start).map(|token| token.span) {
            self.diagnostics.push(
                Diagnostic::error("expected item")
                    .with_code("E_PARSE")
                    .with_span(span),
            );
        }

        self.builder.start_node(SyntaxKind::Error);
        for token in &self.tokens[start..self.pos] {
            self.builder.token(token.kind, &token.text);
        }
        self.builder.finish_node();
    }

    fn current_item(&self) -> Option<ItemBoundary> {
        self.item_boundary_at(self.pos)
    }

    fn item_boundary_at(&self, start: usize) -> Option<ItemBoundary> {
        let mut cursor = start;
        loop {
            cursor = self.skip_trivia(cursor);
            if self.at_attribute_start(cursor) {
                cursor = self.skip_attribute(cursor);
                continue;
            }
            break;
        }

        cursor = self.skip_trivia(cursor);
        if self.at_kind(cursor, SyntaxKind::PubKw) {
            cursor = self.skip_trivia(cursor + 1);
        }

        let kind = match self.kind_at(cursor)? {
            SyntaxKind::UseKw => SyntaxKind::UseItem,
            SyntaxKind::ConstKw => SyntaxKind::ConstItem,
            SyntaxKind::GlobalKw => SyntaxKind::GlobalItem,
            SyntaxKind::FnKw => SyntaxKind::FunctionItem,
            SyntaxKind::StructKw => SyntaxKind::StructItem,
            SyntaxKind::EnumKw => SyntaxKind::EnumItem,
            SyntaxKind::TraitKw => SyntaxKind::TraitItem,
            SyntaxKind::ImplKw => SyntaxKind::ImplItem,
            _ => return None,
        };
        let end = self.find_item_end(kind, cursor);
        Some(ItemBoundary { kind, end })
    }

    fn find_item_end(&self, kind: SyntaxKind, keyword_pos: usize) -> usize {
        match kind {
            SyntaxKind::UseItem | SyntaxKind::GlobalItem | SyntaxKind::ConstItem => {
                self.find_semicolon_item_end(keyword_pos)
            }
            SyntaxKind::FunctionItem
            | SyntaxKind::StructItem
            | SyntaxKind::EnumItem
            | SyntaxKind::TraitItem
            | SyntaxKind::ImplItem => self.find_braced_item_end(keyword_pos),
            _ => keyword_pos.saturating_add(1),
        }
    }

    fn find_semicolon_item_end(&self, start: usize) -> usize {
        let mut cursor = start;
        let mut depth = DelimiterDepth::default();
        while let Some(kind) = self.kind_at(cursor) {
            if kind == SyntaxKind::Eof {
                return cursor;
            }
            if depth.is_root() {
                if kind == SyntaxKind::Semicolon {
                    return cursor + 1;
                }
                if kind.is_trivia()
                    && self.tokens[cursor].text.contains('\n')
                    && self.next_significant_starts_item(cursor + 1)
                {
                    return cursor;
                }
            }
            depth.bump(kind);
            cursor += 1;
        }
        self.tokens.len()
    }

    fn find_braced_item_end(&self, start: usize) -> usize {
        let mut cursor = start;
        while let Some(kind) = self.kind_at(cursor) {
            if kind == SyntaxKind::Eof {
                return cursor;
            }
            if kind == SyntaxKind::LBrace {
                return self.find_matching_brace_end(cursor);
            }
            cursor += 1;
        }
        self.tokens.len()
    }

    fn find_matching_brace_end(&self, open_brace: usize) -> usize {
        let mut cursor = open_brace;
        let mut depth = 0_u32;
        while let Some(kind) = self.kind_at(cursor) {
            match kind {
                SyntaxKind::LBrace => depth = depth.saturating_add(1),
                SyntaxKind::RBrace => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return cursor + 1;
                    }
                }
                SyntaxKind::Eof => return cursor,
                _ => {}
            }
            cursor += 1;
        }
        self.tokens.len()
    }

    fn skip_attribute(&self, hash: usize) -> usize {
        let mut cursor = self.skip_trivia(hash + 1);
        let mut bracket_depth = 0_u32;
        while let Some(kind) = self.kind_at(cursor) {
            match kind {
                SyntaxKind::LBracket => bracket_depth = bracket_depth.saturating_add(1),
                SyntaxKind::RBracket => {
                    bracket_depth = bracket_depth.saturating_sub(1);
                    cursor += 1;
                    if bracket_depth == 0 {
                        return cursor;
                    }
                    continue;
                }
                SyntaxKind::Eof => return cursor,
                _ => {}
            }
            cursor += 1;
        }
        self.tokens.len()
    }

    fn at_attribute_start(&self, hash: usize) -> bool {
        self.at_kind(hash, SyntaxKind::Hash)
            && self.at_kind(self.skip_trivia(hash + 1), SyntaxKind::LBracket)
    }

    fn next_significant_starts_item(&self, cursor: usize) -> bool {
        let next = self.skip_trivia(cursor);
        self.item_boundary_at(next).is_some()
    }

    fn skip_trivia(&self, mut cursor: usize) -> usize {
        while self.kind_at(cursor).is_some_and(SyntaxKind::is_trivia) {
            cursor += 1;
        }
        cursor
    }

    fn emit_current_token(&mut self) {
        if let Some(token) = self.tokens.get(self.pos) {
            if token.kind != SyntaxKind::Eof {
                self.builder.token(token.kind, &token.text);
            }
            self.pos += 1;
        }
    }

    fn at_eof(&self) -> bool {
        self.current_kind()
            .is_none_or(|kind| kind == SyntaxKind::Eof)
    }

    fn current_kind(&self) -> Option<SyntaxKind> {
        self.kind_at(self.pos)
    }

    fn at_kind(&self, cursor: usize, kind: SyntaxKind) -> bool {
        self.kind_at(cursor) == Some(kind)
    }

    fn kind_at(&self, cursor: usize) -> Option<SyntaxKind> {
        self.tokens.get(cursor).map(|token| token.kind)
    }
}

#[derive(Clone, Copy)]
struct ItemBoundary {
    kind: SyntaxKind,
    end: usize,
}

#[derive(Default)]
struct DelimiterDepth {
    paren: u32,
    bracket: u32,
    brace: u32,
}

impl DelimiterDepth {
    fn is_root(&self) -> bool {
        self.paren == 0 && self.bracket == 0 && self.brace == 0
    }

    fn bump(&mut self, kind: SyntaxKind) {
        match kind {
            SyntaxKind::LParen => self.paren = self.paren.saturating_add(1),
            SyntaxKind::RParen => self.paren = self.paren.saturating_sub(1),
            SyntaxKind::LBracket => self.bracket = self.bracket.saturating_add(1),
            SyntaxKind::RBracket => self.bracket = self.bracket.saturating_sub(1),
            SyntaxKind::LBrace => self.brace = self.brace.saturating_add(1),
            SyntaxKind::RBrace => self.brace = self.brace.saturating_sub(1),
            _ => {}
        }
    }
}
