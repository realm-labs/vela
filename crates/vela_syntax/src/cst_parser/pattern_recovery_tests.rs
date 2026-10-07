use crate::ast::{AstNode, SyntaxTuplePattern};
use crate::parse::parse_source;

#[test]
fn unclosed_tuple_patterns_retain_whole_final_pattern_through_repair_and_redamage() {
    for newline in ["\n", "\r\n"] {
        for shifted in [false, true] {
            for context in ["let result = ", "return ", ""] {
                for operand in [
                    "tail",
                    "_",
                    "true",
                    "Choice::Unit",
                    "Choice::Tuple(tail, _)",
                    "Choice::Record { value: tail }",
                ] {
                    for qualified in [true, false] {
                        for repaired in [false, true, false] {
                            let pattern = format!(
                                "{}(head, {operand}{}",
                                if qualified { "Choice::Tuple" } else { "" },
                                if repaired { ")" } else { "" }
                            );
                            let prefix = if shifted {
                                "// shifted 中😀\n/* second 😀 */\n"
                            } else {
                                ""
                            };
                            let source = format!("{prefix}/*中😀*/ fn run(row) {{ {context}match row {{ {pattern}{}\n", if repaired { " => 1, }; }" } else { "" }).replace('\n', newline);
                            let parsed = parse_source(&source);
                            assert_eq!(parsed.tree().syntax().to_string(), source);
                            let node = parsed
                                .tree()
                                .syntax()
                                .descendants()
                                .find_map(SyntaxTuplePattern::cast)
                                .expect("retained outer tuple pattern");
                            assert_eq!(node.syntax().to_string(), pattern);
                            assert_eq!(
                                node.patterns().count(),
                                2,
                                "retained complete final pattern {operand}"
                            );
                            assert_eq!(
                                node.patterns()
                                    .next()
                                    .expect("first")
                                    .binding_name()
                                    .as_deref(),
                                Some("head")
                            );
                            assert_eq!(node.separator_tokens().len(), 1);
                            assert_eq!(
                                node.path_text().as_deref(),
                                qualified.then_some("Choice::Tuple")
                            );
                            assert_eq!(node.r_paren_token().is_some(), repaired);
                            let value = node.patterns().last().expect("final");
                            assert_eq!(value.syntax().to_string(), operand);
                            let start = source.find(&pattern).expect("literal whole slice");
                            assert_eq!(
                                u32::from(node.syntax().text_range().start()) as usize,
                                start
                            );
                            assert_eq!(
                                u32::from(node.syntax().text_range().end()) as usize,
                                start + pattern.len()
                            );
                            let inner = start
                                + if qualified {
                                    "Choice::Tuple(head, ".len()
                                } else {
                                    "(head, ".len()
                                };
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
