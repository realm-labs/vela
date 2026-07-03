use vela_common::{SourceId, Span};
use vela_syntax::Parse as SyntaxParse;
use vela_syntax::ast::SyntaxSourceFile;
use vela_syntax::ast::{AstNode, Block, SyntaxBlock};
use vela_syntax::legacy::parse_body_blocks_at_spans;

use crate::compiler::body_payloads::CompilerBodyPayload;

pub(super) struct BodyBlockLookup {
    bodies: Vec<Block>,
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
        };
        Self { bodies }
    }

    pub(super) fn body_for_syntax(&self, source: SourceId, body: &SyntaxBlock) -> Option<&Block> {
        self.body_by_span(syntax_body_span(source, body))
    }

    fn body_by_span(&self, span: Span) -> Option<&Block> {
        self.bodies.iter().find(|body| body.span == span)
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
        let nonliteral_valued_let_body = bodies[7].body().expect("nonliteral valued let body");

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
                .is_some()
        );
    }
}
