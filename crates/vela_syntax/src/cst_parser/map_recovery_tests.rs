use crate::ast::{AstNode, SyntaxMapExpr};
use crate::parse::parse_source;

#[test]
fn unclosed_map_retains_its_final_value_expression() {
    for source in [
        "fn main() { let map = { key: source",
        "fn main() { let map = { key: 1, other: source",
        "fn main() { let map = { key: source }; }",
    ] {
        let parsed = parse_source(source);
        assert_eq!(parsed.tree().syntax().to_string(), source);
        let map = parsed
            .tree()
            .syntax()
            .descendants()
            .find_map(SyntaxMapExpr::cast)
            .expect("map even without its closing brace");
        let value = map
            .entries()
            .last()
            .and_then(|entry| entry.value())
            .expect("final map value");
        assert_eq!(value.syntax().to_string(), "source");
    }
    for source in [
        "fn main() { let value = { let inside = 1; inside",
        "fn main() { let value = { inside",
    ] {
        let parsed = parse_source(source);
        assert_eq!(parsed.tree().syntax().to_string(), source);
        assert!(
            !parsed
                .tree()
                .syntax()
                .descendants()
                .any(|node| SyntaxMapExpr::cast(node).is_some())
        );
    }
}
