use crate::ast::{AstNode, SyntaxParenExpr, SyntaxTupleExpr};
use crate::parse::parse_source;

#[test]
fn unclosed_parens_retain_whole_final_expression_through_repair_and_redamage() {
    for newline in ["\n", "\r\n"] {
        for shifted in [false, true] {
            for context in ["let result = ", "return ", ""] {
                for operand in [
                    "row.field",
                    "row.field + 2",
                    "apply(value = row.field)",
                    "[ row.field]",
                    "innerRows[ row.field]",
                    "{ key: row.field }",
                ] {
                    for tuple in [false, true] {
                        for repaired in [false, true, false] {
                            let expression = format!(
                                "({}{operand}{}",
                                if tuple { "0, " } else { " " },
                                if repaired { ")" } else { "" }
                            );
                            let prefix = if shifted {
                                "// shifted 中😀\n/* second 😀 */\n"
                            } else {
                                ""
                            };
                            let source = format!(
                                "{prefix}/*中😀*/ fn run(row) {{ {context}{expression}{}\n",
                                if repaired { "; }" } else { "" }
                            )
                            .replace('\n', newline);
                            let parsed = parse_source(&source);
                            assert_eq!(parsed.tree().syntax().to_string(), source);
                            let (node, value, closed) = if tuple {
                                let node = parsed
                                    .tree()
                                    .syntax()
                                    .descendants()
                                    .find_map(SyntaxTupleExpr::cast)
                                    .expect("retained tuple");
                                assert_eq!(node.expressions().count(), 2);
                                assert_eq!(
                                    node.expressions()
                                        .next()
                                        .expect("first")
                                        .syntax()
                                        .to_string(),
                                    "0"
                                );
                                assert_eq!(node.separator_tokens().len(), 1);
                                (
                                    node.syntax().clone(),
                                    node.expressions().last().expect("final"),
                                    node.r_paren_token().is_some(),
                                )
                            } else {
                                let node = parsed
                                    .tree()
                                    .syntax()
                                    .descendants()
                                    .find_map(SyntaxParenExpr::cast)
                                    .expect("retained grouping");
                                (
                                    node.syntax().clone(),
                                    node.expression().expect("inner"),
                                    node.r_paren_token().is_some(),
                                )
                            };
                            assert_eq!(node.to_string(), expression);
                            assert_eq!(
                                value.syntax().to_string(),
                                operand,
                                "{context}/{tuple}/{repaired}"
                            );
                            assert_eq!(closed, repaired);
                            let start = source.find(&expression).expect("literal whole slice");
                            assert_eq!(u32::from(node.text_range().start()) as usize, start);
                            assert_eq!(
                                u32::from(node.text_range().end()) as usize,
                                start + expression.len()
                            );
                            let inner = source.find(operand).expect("literal child slice");
                            assert_eq!(
                                u32::from(value.syntax().text_range().start()) as usize,
                                inner
                            );
                            assert_eq!(
                                u32::from(value.syntax().text_range().end()) as usize,
                                inner + operand.len()
                            );
                            assert_eq!(parsed.diagnostics().is_empty(), repaired);
                        }
                    }
                }
            }
        }
    }
}
