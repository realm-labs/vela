use std::collections::BTreeMap;

use vela_syntax::ast::{FunctionItem, ItemKind, SourceFile};

pub(super) fn function_body_payloads(parsed: &SourceFile) -> BTreeMap<&str, &FunctionItem> {
    let mut payloads = BTreeMap::new();
    for (name, function) in parsed.items.iter().filter_map(|item| match &item.kind {
        ItemKind::Function(function) => Some((function.name.as_str(), function)),
        _ => None,
    }) {
        payloads.entry(name).or_insert(function);
    }
    payloads
}
