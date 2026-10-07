use crate::ast::{AstNode, SyntaxArrayExpr, SyntaxIndexExpr};
use crate::parse::parse_source;

#[test]
fn unclosed_brackets_retain_whole_final_expression_through_repair_and_redamage() {
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
                    for indexed in [false, true] {
                        for repaired in [false, true, false] {
                            let expression = format!(
                                "{}{operand}{}",
                                if indexed { "rows[ " } else { "[ " },
                                if repaired { "]" } else { "" }
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
                            let (node, value, closed) = if indexed {
                                let index = parsed
                                    .tree()
                                    .syntax()
                                    .descendants()
                                    .find_map(SyntaxIndexExpr::cast)
                                    .expect("retained index");
                                assert_eq!(
                                    index.receiver().expect("receiver").syntax().to_string(),
                                    "rows"
                                );
                                (
                                    index.syntax().clone(),
                                    index.index().expect("index expression"),
                                    index.r_bracket_token().is_some(),
                                )
                            } else {
                                let array = parsed
                                    .tree()
                                    .syntax()
                                    .descendants()
                                    .find_map(SyntaxArrayExpr::cast)
                                    .expect("retained array");
                                (
                                    array.syntax().clone(),
                                    array.expressions().last().expect("final element"),
                                    array.r_bracket_token().is_some(),
                                )
                            };
                            assert_eq!(node.to_string(), expression);
                            assert_eq!(
                                value.syntax().to_string(),
                                operand,
                                "{context}/{indexed}/{repaired}"
                            );
                            assert_eq!(closed, repaired);
                            let start = source.find(&expression).expect("literal source slice");
                            assert_eq!(u32::from(node.text_range().start()) as usize, start);
                            assert_eq!(
                                u32::from(node.text_range().end()) as usize,
                                start + expression.len()
                            );
                            let inner = source.find(operand).expect("literal inner expression");
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
