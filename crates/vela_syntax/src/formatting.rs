use vela_common::{Diagnostic, SourceId};

use crate::parse::parse_source_with_id;
use crate::{SyntaxKind, SyntaxNode, SyntaxToken};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormattedSource {
    text: String,
    diagnostics: Vec<Diagnostic>,
}

impl FormattedSource {
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

#[must_use]
pub fn format_source(source: SourceId, text: &str) -> FormattedSource {
    let parsed = parse_source_with_id(source, text);
    let tokens = parsed
        .syntax_node()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .map(LayoutToken::from_token);
    let mut formatter = CstLayoutWriter::new();
    formatter.format(tokens);
    FormattedSource {
        text: formatter.finish(),
        diagnostics: parsed.into_diagnostics(),
    }
}

#[derive(Debug, Default)]
struct CstLayoutWriter {
    output: String,
    indent: usize,
    line_start: bool,
    pending_blank_lines: usize,
    last_token: Option<WrittenToken>,
    brace_context_stack: Vec<BraceContext>,
    declaration_brace_pending: bool,
    use_item_pending: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BraceContext {
    Code,
    DeclarationMembers,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WrittenToken {
    kind: SyntaxKind,
    text: String,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct LayoutContext {
    in_type_arguments: bool,
    opens_declaration_members: bool,
    comma_breaks_line: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LayoutToken {
    kind: SyntaxKind,
    text: String,
    context: LayoutContext,
}

impl LayoutToken {
    fn from_token(token: SyntaxToken) -> Self {
        let kind = token.kind();
        Self {
            kind,
            text: token.text().to_owned(),
            context: LayoutContext {
                in_type_arguments: token_has_ancestor(&token, SyntaxKind::TypeArgList),
                opens_declaration_members: token_opens_declaration_members(&token),
                comma_breaks_line: token_comma_breaks_line(&token),
            },
        }
    }
}

impl CstLayoutWriter {
    fn new() -> Self {
        Self {
            line_start: true,
            ..Self::default()
        }
    }

    fn format(&mut self, tokens: impl IntoIterator<Item = LayoutToken>) {
        for token in tokens {
            self.write_token(token);
        }
    }

    fn finish(mut self) -> String {
        self.trim_trailing_horizontal_space();
        if !self.output.is_empty() && !self.output.ends_with('\n') {
            self.output.push('\n');
        }
        self.output
    }

    fn write_trivia(&mut self, token: &LayoutToken) {
        let kind = token.kind;
        let text = token.text.as_str();
        match kind {
            SyntaxKind::Whitespace => {
                if token.context.in_type_arguments {
                    return;
                }
                let newline_count = text.matches('\n').count();
                if newline_count > 0 && self.use_item_pending && self.brace_context_stack.is_empty()
                {
                    self.newline();
                    self.pending_blank_lines = self
                        .pending_blank_lines
                        .max(newline_count.saturating_sub(1));
                    self.use_item_pending = false;
                    return;
                }
                self.pending_blank_lines = self
                    .pending_blank_lines
                    .max(newline_count.saturating_sub(1));
            }
            SyntaxKind::LineComment | SyntaxKind::Shebang => self.write_line_comment(text),
            SyntaxKind::BlockComment => self.write_block_comment(text),
            SyntaxKind::Unknown => self.write_unknown_trivia(text),
            _ => {}
        }
    }

    fn write_line_comment(&mut self, text: &str) {
        if !self.line_start {
            self.output.push(' ');
        }
        self.write_indent_if_needed();
        self.output.push_str(text.trim_end());
        self.newline();
    }

    fn write_block_comment(&mut self, text: &str) {
        if text.contains('\n') {
            self.ensure_line_start();
            self.write_indent_if_needed();
            self.output.push_str(text.trim_end());
            self.newline();
        } else {
            if !self.line_start {
                self.output.push(' ');
            }
            self.write_indent_if_needed();
            self.output.push_str(text.trim_end());
        }
    }

    fn write_unknown_trivia(&mut self, text: &str) {
        self.write_indent_if_needed();
        self.output.push_str(text);
    }

    fn write_token(&mut self, token: LayoutToken) {
        let kind = token.kind;
        let text = token.text.as_str();
        match kind {
            SyntaxKind::Eof => {}
            kind if kind.is_trivia() || kind == SyntaxKind::Unknown => {
                self.write_trivia(&token);
                return;
            }
            kind if kind.is_symbol() => self.write_symbol(&token),
            _ => {
                self.write_space_before_word(kind);
                self.write_indent_if_needed();
                self.output.push_str(text);
            }
        }
        self.observe_token(kind);
        self.last_token = Some(WrittenToken {
            kind,
            text: token.text,
        });
    }

    fn write_symbol(&mut self, token: &LayoutToken) {
        let symbol = token.kind;
        let text = token.text.as_str();
        match symbol {
            SyntaxKind::LBrace => {
                self.write_space_before_open_brace();
                self.write_indent_if_needed();
                self.output.push_str(text);
                self.brace_context_stack
                    .push(self.next_brace_context(token.context));
                self.declaration_brace_pending = false;
                self.indent = self.indent.saturating_add(1);
                self.newline();
            }
            SyntaxKind::RBrace => {
                self.indent = self.indent.saturating_sub(1);
                self.brace_context_stack.pop();
                self.ensure_line_start();
                self.write_indent_if_needed();
                self.output.push_str(text);
            }
            SyntaxKind::LParen | SyntaxKind::LBracket => {
                self.write_indent_if_needed();
                self.output.push_str(text);
            }
            SyntaxKind::RParen => {
                self.trim_trailing_horizontal_space();
                self.output.push_str(text);
            }
            SyntaxKind::RBracket => {
                self.trim_trailing_horizontal_space();
                self.output.push_str(text);
            }
            SyntaxKind::Comma => {
                self.trim_trailing_horizontal_space();
                self.output.push_str(text);
                if token.context.in_type_arguments {
                    self.output.push(' ');
                } else if token.context.comma_breaks_line {
                    self.newline();
                } else {
                    self.output.push(' ');
                }
            }
            SyntaxKind::Semicolon => {
                self.trim_trailing_horizontal_space();
                self.output.push_str(text);
                self.newline();
            }
            SyntaxKind::Dot | SyntaxKind::ColonColon | SyntaxKind::Question => {
                self.trim_trailing_horizontal_space();
                self.output.push_str(text);
            }
            SyntaxKind::Colon => {
                self.trim_trailing_horizontal_space();
                self.output.push_str(text);
                self.output.push(' ');
            }
            SyntaxKind::Less if token.context.in_type_arguments => {
                self.trim_trailing_horizontal_space();
                self.write_indent_if_needed();
                self.output.push_str(text);
            }
            SyntaxKind::Greater if token.context.in_type_arguments => {
                self.write_type_argument_close(text);
            }
            SyntaxKind::GreaterEqual if token.context.in_type_arguments => {
                self.write_type_argument_close(">");
                self.output.push(' ');
                self.output.push('=');
                self.output.push(' ');
            }
            SyntaxKind::Arrow | SyntaxKind::FatArrow => self.write_spaced_symbol(text),
            symbol if is_assignment_or_binary_symbol(symbol) => self.write_spaced_symbol(text),
            SyntaxKind::Pipe => {
                if matches!(
                    self.previous_kind(),
                    None | Some(SyntaxKind::LParen | SyntaxKind::Equal | SyntaxKind::Comma)
                ) {
                    self.write_indent_if_needed();
                    self.output.push_str(text);
                } else {
                    self.write_spaced_symbol(text);
                }
            }
            _ => {
                self.write_indent_if_needed();
                self.output.push_str(text);
            }
        }
    }

    fn write_space_before_word(&mut self, token: SyntaxKind) {
        if self.previous_kind() == Some(SyntaxKind::RBrace) && !self.line_start {
            self.trim_trailing_horizontal_space();
            if token == SyntaxKind::ElseKw {
                self.output.push(' ');
            } else {
                self.newline();
            }
            return;
        }

        if self.should_start_declaration_member(token) {
            self.newline();
            return;
        }

        if self.line_start || !needs_space_between(self.last_token.as_ref(), token) {
            return;
        }
        self.trim_trailing_horizontal_space();
        self.output.push(' ');
    }

    fn write_space_before_open_brace(&mut self) {
        if self.line_start {
            return;
        }
        match self.previous_kind() {
            Some(
                SyntaxKind::LBrace
                | SyntaxKind::LBracket
                | SyntaxKind::ColonColon
                | SyntaxKind::Dot,
            ) => {}
            _ => {
                self.trim_trailing_horizontal_space();
                self.output.push(' ');
            }
        }
    }

    fn write_spaced_symbol(&mut self, text: &str) {
        if !self.line_start {
            self.trim_trailing_horizontal_space();
            self.output.push(' ');
        }
        self.write_indent_if_needed();
        self.output.push_str(text);
        self.output.push(' ');
    }

    fn write_indent_if_needed(&mut self) {
        if !self.line_start {
            return;
        }
        self.flush_blank_lines();
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
        self.line_start = false;
    }

    fn flush_blank_lines(&mut self) {
        if self.output.is_empty() {
            self.pending_blank_lines = 0;
            return;
        }
        for _ in 0..self.pending_blank_lines.min(2) {
            if !self.output.ends_with('\n') {
                self.output.push('\n');
            }
            self.output.push('\n');
        }
        self.pending_blank_lines = 0;
    }

    fn ensure_line_start(&mut self) {
        if !self.line_start {
            self.newline();
        }
    }

    fn newline(&mut self) {
        self.trim_trailing_horizontal_space();
        if !self.output.ends_with('\n') {
            self.output.push('\n');
        }
        self.line_start = true;
    }

    fn trim_trailing_horizontal_space(&mut self) {
        while self.output.ends_with([' ', '\t']) {
            self.output.pop();
        }
    }

    fn in_declaration_members(&self) -> bool {
        self.brace_context_stack.last() == Some(&BraceContext::DeclarationMembers)
    }

    fn write_type_argument_close(&mut self, text: &str) {
        self.trim_trailing_horizontal_space();
        self.write_indent_if_needed();
        self.output.push_str(text);
    }

    fn next_brace_context(&self, context: LayoutContext) -> BraceContext {
        if context.opens_declaration_members
            || self.declaration_brace_pending
            || self.starts_nested_declaration_members()
        {
            BraceContext::DeclarationMembers
        } else {
            BraceContext::Code
        }
    }

    fn starts_nested_declaration_members(&self) -> bool {
        self.in_declaration_members() && self.previous_kind() == Some(SyntaxKind::Ident)
    }

    fn should_start_declaration_member(&self, token: SyntaxKind) -> bool {
        self.in_declaration_members()
            && !self.line_start
            && token == SyntaxKind::Ident
            && preceding_significant_can_end_declaration_member(self.previous_kind())
    }

    fn observe_token(&mut self, token: SyntaxKind) {
        match token {
            SyntaxKind::UseKw => {
                self.declaration_brace_pending = false;
                self.use_item_pending = true;
            }
            SyntaxKind::StructKw
            | SyntaxKind::EnumKw
            | SyntaxKind::TraitKw
            | SyntaxKind::ImplKw => {
                self.declaration_brace_pending = true;
            }
            SyntaxKind::FnKw
            | SyntaxKind::ConstKw
            | SyntaxKind::ExternKw
            | SyntaxKind::LetKw
            | SyntaxKind::IfKw
            | SyntaxKind::ElseKw
            | SyntaxKind::MatchKw
            | SyntaxKind::ReturnKw
            | SyntaxKind::BreakKw
            | SyntaxKind::ContinueKw => {
                self.declaration_brace_pending = false;
                self.use_item_pending = false;
            }
            SyntaxKind::PubKw
            | SyntaxKind::ForKw
            | SyntaxKind::InKw
            | SyntaxKind::AsKw
            | SyntaxKind::TrueKw
            | SyntaxKind::FalseKw
            | SyntaxKind::SelfKw => {}
            _ => {}
        }
    }

    fn previous_kind(&self) -> Option<SyntaxKind> {
        self.last_token.as_ref().map(|token| token.kind)
    }
}

fn needs_space_between(previous: Option<&WrittenToken>, current: SyntaxKind) -> bool {
    let Some(previous) = previous else {
        return false;
    };
    if matches!(
        previous.kind,
        SyntaxKind::LParen
            | SyntaxKind::LBracket
            | SyntaxKind::Dot
            | SyntaxKind::ColonColon
            | SyntaxKind::Bang
    ) || matches!(
        current,
        SyntaxKind::RParen
            | SyntaxKind::RBracket
            | SyntaxKind::RBrace
            | SyntaxKind::Comma
            | SyntaxKind::Dot
            | SyntaxKind::ColonColon
            | SyntaxKind::Semicolon
            | SyntaxKind::Question
    ) {
        return false;
    }
    is_word_like(previous.kind) && is_word_like(current)
}

fn is_word_like(token: SyntaxKind) -> bool {
    matches!(
        token,
        SyntaxKind::Ident
            | SyntaxKind::Int
            | SyntaxKind::Float
            | SyntaxKind::Char
            | SyntaxKind::String
            | SyntaxKind::InterpolatedString
            | SyntaxKind::Bytes
            | SyntaxKind::UseKw
            | SyntaxKind::PubKw
            | SyntaxKind::ConstKw
            | SyntaxKind::ExternKw
            | SyntaxKind::LetKw
            | SyntaxKind::FnKw
            | SyntaxKind::StructKw
            | SyntaxKind::EnumKw
            | SyntaxKind::TraitKw
            | SyntaxKind::ImplKw
            | SyntaxKind::ForKw
            | SyntaxKind::IfKw
            | SyntaxKind::ElseKw
            | SyntaxKind::MatchKw
            | SyntaxKind::ReturnKw
            | SyntaxKind::BreakKw
            | SyntaxKind::ContinueKw
            | SyntaxKind::TrueKw
            | SyntaxKind::FalseKw
            | SyntaxKind::SelfKw
            | SyntaxKind::InKw
            | SyntaxKind::AsKw
            | SyntaxKind::AsyncKw
            | SyntaxKind::AwaitKw
    )
}

fn preceding_significant_can_end_declaration_member(previous: Option<SyntaxKind>) -> bool {
    matches!(
        previous,
        Some(
            SyntaxKind::Ident
                | SyntaxKind::Int
                | SyntaxKind::Float
                | SyntaxKind::Char
                | SyntaxKind::String
                | SyntaxKind::InterpolatedString
                | SyntaxKind::Bytes
                | SyntaxKind::TrueKw
                | SyntaxKind::FalseKw
                | SyntaxKind::RParen
                | SyntaxKind::RBrace
                | SyntaxKind::RBracket
        )
    )
}

fn token_has_ancestor(token: &SyntaxToken, kind: SyntaxKind) -> bool {
    token.parent_ancestors().any(|node| node.kind() == kind)
}

fn token_opens_declaration_members(token: &SyntaxToken) -> bool {
    token.kind() == SyntaxKind::LBrace
        && token
            .parent_ancestors()
            .find(is_braced_layout_owner)
            .is_some_and(|node| {
                matches!(
                    node.kind(),
                    SyntaxKind::StructItem
                        | SyntaxKind::EnumItem
                        | SyntaxKind::TraitItem
                        | SyntaxKind::ImplItem
                        | SyntaxKind::StructFieldList
                        | SyntaxKind::EnumVariantList
                        | SyntaxKind::RecordFieldList
                )
            })
}

fn token_comma_breaks_line(token: &SyntaxToken) -> bool {
    token.kind() == SyntaxKind::Comma
        && token
            .parent_ancestors()
            .find(is_comma_layout_owner)
            .is_some_and(|node| {
                matches!(
                    node.kind(),
                    SyntaxKind::MapExpr
                        | SyntaxKind::RecordExprFieldList
                        | SyntaxKind::StructFieldList
                        | SyntaxKind::EnumVariantList
                        | SyntaxKind::RecordFieldList
                        | SyntaxKind::MatchArmList
                )
            })
}

fn is_comma_layout_owner(node: &SyntaxNode) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::TypeArgList
            | SyntaxKind::ArgList
            | SyntaxKind::ArrayExpr
            | SyntaxKind::ParamList
            | SyntaxKind::TupleFieldList
            | SyntaxKind::TuplePattern
            | SyntaxKind::MapExpr
            | SyntaxKind::RecordExprFieldList
            | SyntaxKind::StructFieldList
            | SyntaxKind::EnumVariantList
            | SyntaxKind::RecordFieldList
            | SyntaxKind::RecordPattern
            | SyntaxKind::MatchArmList
    )
}

fn is_braced_layout_owner(node: &SyntaxNode) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::Block
            | SyntaxKind::MapExpr
            | SyntaxKind::RecordExpr
            | SyntaxKind::RecordPattern
            | SyntaxKind::StructItem
            | SyntaxKind::EnumItem
            | SyntaxKind::TraitItem
            | SyntaxKind::ImplItem
            | SyntaxKind::StructFieldList
            | SyntaxKind::EnumVariantList
            | SyntaxKind::RecordFieldList
            | SyntaxKind::RecordExprFieldList
    )
}

fn is_assignment_or_binary_symbol(symbol: SyntaxKind) -> bool {
    matches!(
        symbol,
        SyntaxKind::Equal
            | SyntaxKind::PlusEqual
            | SyntaxKind::MinusEqual
            | SyntaxKind::StarEqual
            | SyntaxKind::SlashEqual
            | SyntaxKind::PercentEqual
            | SyntaxKind::Plus
            | SyntaxKind::Minus
            | SyntaxKind::Star
            | SyntaxKind::Slash
            | SyntaxKind::Percent
            | SyntaxKind::BangEqual
            | SyntaxKind::BangEqualEqual
            | SyntaxKind::EqualEqual
            | SyntaxKind::EqualEqualEqual
            | SyntaxKind::Less
            | SyntaxKind::LessEqual
            | SyntaxKind::Greater
            | SyntaxKind::GreaterEqual
            | SyntaxKind::AndAnd
            | SyntaxKind::OrOr
            | SyntaxKind::DotDot
            | SyntaxKind::DotDotEqual
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_id() -> SourceId {
        SourceId::new(1)
    }

    #[test]
    fn formatting_extracts_tokens_and_trivia_in_source_order() {
        let source = "pub fn main() {\n    // keep\n    return 1\n}\n";
        let tokens = syntax_tokens(source);

        assert_eq!(reconstruct_tokens(&tokens), source);
        assert_eq!(
            tokens
                .iter()
                .filter(|token| !token.kind().is_trivia())
                .count(),
            9
        );
        assert!(
            tokens
                .iter()
                .any(|token| token.kind() == SyntaxKind::ReturnKw)
        );
    }

    #[test]
    fn formatting_extracts_comments_and_blank_line_groups() {
        let source = "fn main() {\n    /* one\n\n       two */\n\n    // tail\n}\n";
        let tokens = syntax_tokens(source);
        let comments = tokens
            .iter()
            .filter_map(|token| match token.kind() {
                SyntaxKind::LineComment | SyntaxKind::BlockComment => Some(token.text()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let blank_line_group = tokens.iter().any(|token| {
            token.kind() == SyntaxKind::Whitespace && token.text().matches('\n').count() >= 2
        });

        assert_eq!(reconstruct_tokens(&tokens), source);
        assert_eq!(comments, vec!["/* one\n\n       two */", "// tail"]);
        assert!(blank_line_group);
    }

    #[test]
    fn formatting_extracts_shebang_as_trivia() {
        let source = "#!/usr/bin/env vela\nfn main() { return 1 }\n";
        let tokens = syntax_tokens(source);

        assert_eq!(
            tokens.first().map(SyntaxToken::kind),
            Some(SyntaxKind::Shebang)
        );
        assert_eq!(u32::from(tokens[0].text_range().start()), 0);
        assert_eq!(u32::from(tokens[0].text_range().end()), 20);
        assert!(
            tokens
                .iter()
                .any(|token| token.kind() == SyntaxKind::LBrace)
        );
        assert_eq!(reconstruct_tokens(&tokens), source);
    }

    #[test]
    fn formatting_formats_expressions_and_function_blocks() {
        let source = "pub fn main(){return 1+2*3}";
        let formatted = format_source(source_id(), source);

        assert!(formatted.diagnostics().is_empty());
        assert_eq!(
            formatted.text(),
            "pub fn main() {\n    return 1 + 2 * 3\n}\n"
        );
    }

    #[test]
    fn formatting_preserves_newline_after_use_item() {
        let source = "use game::reward::grant\npub fn main(){return 1}";
        let formatted = format_source(source_id(), source);

        assert!(formatted.diagnostics().is_empty());
        assert_eq!(
            formatted.text(),
            "\
use game::reward::grant
pub fn main() {
    return 1
}
"
        );
    }

    #[test]
    fn formatting_preserves_comments_while_formatting_blocks() {
        let source = "fn main(){// keep\nlet value=1\n/* block\n\ncomment */\nreturn value}";
        let formatted = format_source(source_id(), source);

        assert_eq!(
            formatted.text(),
            "fn main() {\n    // keep\n    let value = 1\n    /* block\n\ncomment */\n    return value\n}\n"
        );
    }

    #[test]
    fn formatting_formats_item_declarations() {
        let source = "pub struct Player{level:i64 name:String}pub enum Reward{None Coins(amount:i64) Item{id:String}}pub trait Damageable{fn damage(amount:i64)->bool;}impl Damageable for Player{fn damage(amount:i64)->bool{return amount>0}}impl Player{fn heal(amount:i64)->i64{return amount}}";
        let formatted = format_source(source_id(), source);

        assert!(formatted.diagnostics().is_empty());
        assert_eq!(
            formatted.text(),
            "\
pub struct Player {
    level: i64
    name: String
}
pub enum Reward {
    None
    Coins(amount: i64)
    Item {
        id: String
    }
}
pub trait Damageable {
    fn damage(amount: i64) -> bool;
}
impl Damageable for Player {
    fn damage(amount: i64) -> bool {
        return amount > 0
    }
}
impl Player {
    fn heal(amount: i64) -> i64 {
        return amount
    }
}
"
        );
    }

    #[test]
    fn formatting_formats_state_forms_without_reserving_state_identifiers() {
        let source = "state counter:i64=0;pub extern state world:World;fn state(state:i64){let state=state;}";
        let formatted = format_source(source_id(), source);

        assert!(formatted.diagnostics().is_empty());
        assert_eq!(
            formatted.text(),
            "\
state counter: i64 = 0;
pub extern state world: World;
fn state(state: i64) {
    let state = state;
}
"
        );
    }

    #[test]
    fn formatting_compacts_builtin_container_type_arguments() {
        let source = "fn score(scores:Array < i64 >, rewards:Map< String,i64 >, tags:Set <String>)->Result < Map < String , i64 > , String >{return result::ok(rewards)}";
        let formatted = format_source(source_id(), source);

        assert!(formatted.diagnostics().is_empty());
        assert_eq!(
            formatted.text(),
            "\
fn score(scores: Array<i64>, rewards: Map<String, i64>, tags: Set<String>) -> Result<Map<String, i64>, String> {
    return result::ok(rewards)
}
"
        );
    }

    #[test]
    fn formatting_compacts_nested_result_container_type_arguments() {
        let source =
            "struct Loader{cache:Option < Result < Array < Map < String , i64 > > , String > >}";
        let formatted = format_source(source_id(), source);

        assert!(formatted.diagnostics().is_empty());
        assert_eq!(
            formatted.text(),
            "\
struct Loader {
    cache: Option<Result<Array<Map<String, i64>>, String>>
}
"
        );
    }

    #[test]
    fn formatting_preserves_container_type_arguments_on_one_line() {
        let source = "\
fn load(input: Result<Map<String, i64>, String>) -> Option<Array<Set<String>>> {
    return option::some([])
}
";
        let formatted = format_source(source_id(), source);

        assert!(formatted.diagnostics().is_empty());
        assert_eq!(formatted.text(), source);
    }

    #[test]
    fn formatting_formats_container_type_hint_example() {
        let source = "\
fn load_rewards(rewards:Map < String,i64 >)->Result < Map<String , i64>,String >{return result::ok(rewards)}

fn main(){let scores:Array < i64 > = [1,2,3];let rewards:Map < String,i64 >={\"xp\":5};let tags:Set < String > = set::from_array([\"daily\",\"vip\"]);return score(scores,rewards,tags).unwrap_or(0)}
";
        let formatted = format_source(source_id(), source);

        assert!(formatted.diagnostics().is_empty());
        assert_eq!(
            formatted.text(),
            "\
fn load_rewards(rewards: Map<String, i64>) -> Result<Map<String, i64>, String> {
    return result::ok(rewards)
}

fn main() {
    let scores: Array<i64> = [1, 2, 3];
    let rewards: Map<String, i64> = {
        \"xp\": 5
    };
    let tags: Set<String> = set::from_array([\"daily\", \"vip\"]);
    return score(scores, rewards, tags).unwrap_or(0)
}
"
        );

        let reformatted = format_source(source_id(), formatted.text());
        assert_eq!(reformatted.text(), formatted.text());
    }

    #[test]
    fn formatting_keeps_else_attached_to_closing_block() {
        let source = "fn main(){if true{return 1}else{return 2}}";
        let formatted = format_source(source_id(), source);

        assert_eq!(
            formatted.text(),
            "\
fn main() {
    if true {
        return 1
    } else {
        return 2
    }
}
"
        );
    }

    #[test]
    fn formatting_preserves_async_and_await_spacing() {
        let formatted = format_source(source_id(), "pub  async  fn run(){service::load(1).await;}");

        assert_eq!(
            formatted.text(),
            "pub async fn run() {\n    service::load(1).await;\n}\n"
        );
    }

    fn syntax_tokens(source: &str) -> Vec<SyntaxToken> {
        let parsed = parse_source_with_id(source_id(), source);
        assert!(parsed.diagnostics().is_empty());
        parsed
            .syntax_node()
            .descendants_with_tokens()
            .filter_map(|element| element.into_token())
            .collect()
    }

    fn reconstruct_tokens(tokens: &[SyntaxToken]) -> String {
        tokens.iter().map(SyntaxToken::text).collect::<String>()
    }
}
