use std::collections::BTreeMap;

use vela_syntax::Parse as SyntaxParse;
use vela_syntax::ast::{SyntaxExpression, SyntaxSourceFile};

pub(super) fn const_value_payloads(
    parsed: &SyntaxParse<SyntaxSourceFile>,
) -> BTreeMap<String, SyntaxExpression> {
    let mut payloads = BTreeMap::new();
    for item in parsed.tree().consts() {
        let Some(name) = item.name_text() else {
            continue;
        };
        let Some(value) = item.value() else {
            continue;
        };
        payloads.entry(name).or_insert(value);
    }
    payloads
}
