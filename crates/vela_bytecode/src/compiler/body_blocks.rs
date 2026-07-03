use vela_common::SourceId;
use vela_common::Span;
use vela_syntax::Parse as SyntaxParse;
use vela_syntax::ast::AstNode;
use vela_syntax::ast::SyntaxBlock;
use vela_syntax::ast::SyntaxSourceFile;
use vela_syntax::ast::{Block, Stmt};
use vela_syntax::parse_body_blocks_at_spans;

use crate::compiler::body_payloads::CompilerBodyFallback;
use crate::compiler::body_payloads::CompilerBodyPayload;

pub(super) struct BodyBlockLookup {
    bodies: Vec<BodyBlockEntry>,
}

struct BodyBlockEntry {
    span: Span,
    statements: Vec<Stmt>,
    block: Block,
}

impl BodyBlockLookup {
    pub(super) fn from_syntax(
        source: SourceId,
        text: &str,
        syntax: &SyntaxParse<SyntaxSourceFile>,
    ) -> Self {
        let required_spans = syntax_body_spans(source, syntax);
        let bodies = if required_spans.is_empty() {
            Vec::new()
        } else {
            parse_body_blocks_at_spans(source, text, &required_spans)
                .into_iter()
                .map(BodyBlockEntry::new)
                .collect()
        };
        Self { bodies }
    }

    pub(super) fn body_for_syntax(
        &self,
        source: SourceId,
        body: &SyntaxBlock,
    ) -> Option<CompilerBodyFallback<'_>> {
        self.body_by_span(syntax_body_span(source, body))
    }

    fn body_by_span(&self, span: Span) -> Option<CompilerBodyFallback<'_>> {
        self.bodies
            .iter()
            .find(|body| body.span == span)
            .map(BodyBlockEntry::fallback)
    }
}

impl BodyBlockEntry {
    fn new(block: Block) -> Self {
        Self {
            span: block.span,
            statements: block.statements.clone(),
            block,
        }
    }

    fn fallback(&self) -> CompilerBodyFallback<'_> {
        CompilerBodyFallback::statements_with_block(&self.statements, &self.block)
    }
}

fn syntax_body_spans(source: SourceId, syntax: &SyntaxParse<SyntaxSourceFile>) -> Vec<Span> {
    syntax
        .tree()
        .functions()
        .filter_map(|function| function.body())
        .chain(
            syntax
                .tree()
                .impls()
                .flat_map(|item| item.methods().filter_map(|method| method.body())),
        )
        .chain(
            syntax
                .tree()
                .traits()
                .flat_map(|item| item.methods().filter_map(|method| method.body())),
        )
        .filter(CompilerBodyPayload::requires_body_block_lookup)
        .map(|body| syntax_body_span(source, &body))
        .collect()
}

fn syntax_body_span(source: SourceId, body: &SyntaxBlock) -> Span {
    let range = body.syntax().text_range();
    let start: u32 = range.start().into();
    let end: u32 = range.end().into();
    Span::new(source, start, end)
}

#[cfg(test)]
mod tests {
    use vela_common::SourceId;
    use vela_syntax::parse::parse_source_with_id;

    use super::BodyBlockLookup;

    #[test]
    fn syntax_only_cst_bodies_do_not_require_owned_body_lookup() {
        let source = SourceId::new(1);
        let text = r#"
fn empty() {
}

fn bare_return() {
    return;
}

fn empty_let() {
    let value;
}

fn syntax_only_block() {
    {
        let value;
        return;
    }
}

fn valued_return() {
    return 1;
}

fn typed_valued_return() -> i8 {
    return 12;
}

fn valued_let() {
    let value = 1;
}

fn nonliteral_valued_let(input) {
    let value = input;
}

fn binary_valued_let(input) {
    let value = input + 1;
}

fn binary_valued_return(input) {
    return input + 1;
}

fn path_binary_valued_let(input, other) {
    let value = input == other;
}

fn path_binary_valued_return(input, other) {
    return input == other;
}

fn path_comparison_valued_let(input, other) {
    let value = input < other;
}

fn path_comparison_valued_return(input, other) {
    return input >= other;
}

fn path_arithmetic_valued_let(input, other) {
    let value = input * other;
}

fn path_arithmetic_valued_return(input, other) {
    return input * other;
}

fn path_numeric_comparison_let(input) {
    let value = input > 0;
}

fn path_numeric_comparison_return(input) {
    return input < 10;
}

fn path_numeric_equality_let(input) {
    let value = input == 0;
}

fn path_numeric_equality_return(input) {
    return input != 10;
}

fn path_numeric_subtraction_let(input) {
    let value = input - 1;
}

fn path_numeric_subtraction_return(input) {
    return input - 1;
}

fn path_numeric_multiplication_let(input) {
    let value = input * 2;
}

fn path_numeric_multiplication_return(input) {
    return input * 2;
}

fn path_numeric_division_let(input) {
    let value = input / 2;
}

fn path_numeric_division_return(input) {
    return input / 2;
}

fn path_numeric_remainder_let(input) {
    let value = input % 3;
}

fn path_numeric_remainder_return(input) {
    return input % 3;
}

fn unary_valued_let(input) {
    let value = !input;
}

fn unary_valued_return(input) {
    return -input;
}

fn self_valued_let() {
    let value = self;
}

fn self_valued_return() {
    return self;
}

fn block_valued_let() {
    let value = {
        let nested;
        return;
    };
}

fn block_valued_return() {
    return {
        let nested;
        return;
    };
}

fn path_value_expression_statements(input, other) {
    !input;
    input == other;
    input < other;
    input * other;
    input > 0;
    input == 0;
}

fn parenthesized_simple_values() {
    let literal = (1);
    let local = (literal);
    let receiver = (self);
    return (local);
}
"#;
        let parsed = parse_source_with_id(source, text);
        assert!(parsed.diagnostics().is_empty());
        let lookup = BodyBlockLookup::from_syntax(source, text, &parsed);
        let bodies = parsed.tree().functions().collect::<Vec<_>>();
        let empty_body = bodies[0].body().expect("empty body");
        let bare_return_body = bodies[1].body().expect("bare return body");
        let empty_let_body = bodies[2].body().expect("empty let body");
        let syntax_only_block_body = bodies[3].body().expect("syntax-only block body");
        let valued_return_body = bodies[4].body().expect("valued return body");
        let typed_valued_return_body = bodies[5].body().expect("typed return body");
        let valued_let_body = bodies[6].body().expect("valued let body");
        let nonliteral_valued_let_body = bodies[7].body().expect("path valued let body");
        let binary_valued_let_body = bodies[8].body().expect("binary valued let body");
        let binary_valued_return_body = bodies[9].body().expect("binary valued return body");
        let path_binary_valued_let_body = bodies[10].body().expect("path binary valued let body");
        let path_binary_valued_return_body =
            bodies[11].body().expect("path binary valued return body");
        let path_comparison_valued_let_body =
            bodies[12].body().expect("path comparison valued let body");
        let path_comparison_valued_return_body = bodies[13]
            .body()
            .expect("path comparison valued return body");
        let path_arithmetic_valued_let_body =
            bodies[14].body().expect("path arithmetic valued let body");
        let path_arithmetic_valued_return_body = bodies[15]
            .body()
            .expect("path arithmetic valued return body");
        let path_numeric_comparison_let_body =
            bodies[16].body().expect("path numeric comparison let body");
        let path_numeric_comparison_return_body = bodies[17]
            .body()
            .expect("path numeric comparison return body");
        let path_numeric_equality_let_body =
            bodies[18].body().expect("path numeric equality let body");
        let path_numeric_equality_return_body = bodies[19]
            .body()
            .expect("path numeric equality return body");
        let path_numeric_subtraction_let_body = bodies[20]
            .body()
            .expect("path numeric subtraction let body");
        let path_numeric_subtraction_return_body = bodies[21]
            .body()
            .expect("path numeric subtraction return body");
        let path_numeric_multiplication_let_body = bodies[22]
            .body()
            .expect("path numeric multiplication let body");
        let path_numeric_multiplication_return_body = bodies[23]
            .body()
            .expect("path numeric multiplication return body");
        let path_numeric_division_let_body =
            bodies[24].body().expect("path numeric division let body");
        let path_numeric_division_return_body = bodies[25]
            .body()
            .expect("path numeric division return body");
        let path_numeric_remainder_let_body =
            bodies[26].body().expect("path numeric remainder let body");
        let path_numeric_remainder_return_body = bodies[27]
            .body()
            .expect("path numeric remainder return body");
        let unary_valued_let_body = bodies[28].body().expect("unary valued let body");
        let unary_valued_return_body = bodies[29].body().expect("unary valued return body");
        let self_valued_let_body = bodies[30].body().expect("self valued let body");
        let self_valued_return_body = bodies[31].body().expect("self valued return body");
        let block_valued_let_body = bodies[32].body().expect("block valued let body");
        let block_valued_return_body = bodies[33].body().expect("block valued return body");
        let path_value_expression_statements_body = bodies[34]
            .body()
            .expect("path value expression statements body");
        let parenthesized_simple_values_body =
            bodies[35].body().expect("parenthesized simple values body");

        assert!(lookup.body_for_syntax(source, &empty_body).is_none());
        assert!(lookup.body_for_syntax(source, &bare_return_body).is_none());
        assert!(lookup.body_for_syntax(source, &empty_let_body).is_none());
        assert!(
            lookup
                .body_for_syntax(source, &syntax_only_block_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &valued_return_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &typed_valued_return_body)
                .is_none()
        );
        assert!(lookup.body_for_syntax(source, &valued_let_body).is_none());
        assert!(
            lookup
                .body_for_syntax(source, &nonliteral_valued_let_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &binary_valued_let_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &binary_valued_return_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &path_binary_valued_let_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &path_binary_valued_return_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &path_comparison_valued_let_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &path_comparison_valued_return_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &path_arithmetic_valued_let_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &path_arithmetic_valued_return_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &path_numeric_comparison_let_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &path_numeric_comparison_return_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &path_numeric_equality_let_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &path_numeric_equality_return_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &path_numeric_subtraction_let_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &path_numeric_subtraction_return_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &path_numeric_multiplication_let_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &path_numeric_multiplication_return_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &path_numeric_division_let_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &path_numeric_division_return_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &path_numeric_remainder_let_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &path_numeric_remainder_return_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &unary_valued_let_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &unary_valued_return_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &self_valued_let_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &self_valued_return_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &block_valued_let_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &block_valued_return_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &path_value_expression_statements_body)
                .is_none()
        );
        assert!(
            lookup
                .body_for_syntax(source, &parenthesized_simple_values_body)
                .is_none()
        );
    }
}
