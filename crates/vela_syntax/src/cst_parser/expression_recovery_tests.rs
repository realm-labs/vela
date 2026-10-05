use crate::ast::{AstNode, SyntaxLambdaBody, SyntaxLambdaExpr};
use crate::parse::parse_source;

#[test]
fn unclosed_lambda_keeps_the_last_inner_semicolon_and_repair_excludes_the_outer_one() {
    for newline in ["\n", "\r\n"] {
        for shifted in [false, true] {
            for context in ["let callback = ", "return ", ""] {
                let prefix = if shifted {
                    "// shifted 中😀\n/* second 😀 */\n"
                } else {
                    ""
                };
                for repaired in [false, true, false] {
                    let body = if repaired {
                        "{\n let count = item;\n return count;\n}"
                    } else {
                        "{\n let count = item;\n return count;"
                    };
                    let lambda = format!("|item| {body}").replace('\n', newline);
                    let tail = if repaired { ";\n}" } else { "" };
                    let source =
                        format!("{prefix}/* 中😀 */ fn main() {{\n {context}|item| {body}{tail}")
                            .replace('\n', newline);
                    let parsed = parse_source(&source);
                    assert_eq!(parsed.tree().syntax().to_string(), source);
                    let node = parsed
                        .tree()
                        .syntax()
                        .descendants()
                        .find_map(SyntaxLambdaExpr::cast)
                        .expect("recoverable lambda");
                    assert_eq!(node.syntax().to_string(), lambda, "{context}/{repaired}");
                    let Some(SyntaxLambdaBody::Block(block)) = node.body() else {
                        panic!("lambda retains its owned block");
                    };
                    assert_eq!(block.syntax().to_string(), body.replace('\n', newline));
                    let start = source.find("|item|").expect("literal lambda start");
                    assert_eq!(
                        u32::from(node.syntax().text_range().start()) as usize,
                        start
                    );
                    assert_eq!(
                        u32::from(node.syntax().text_range().end()) as usize,
                        start + lambda.len()
                    );
                    assert_eq!(parsed.diagnostics().is_empty(), repaired);
                }
            }
        }
    }
}
