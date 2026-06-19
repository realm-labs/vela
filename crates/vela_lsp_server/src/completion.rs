use serde_json::{Value as JsonValue, json};
use vela_language_service::{CompletionResolvePayload, CompletionSymbol};

pub(crate) fn service_completion_resolve_payload(
    item: &JsonValue,
) -> Result<Option<CompletionResolvePayload>, &'static str> {
    let Some(resolve) = item.get("data").and_then(|data| data.get("resolve")) else {
        return Ok(None);
    };
    let kind = resolve
        .get("kind")
        .and_then(JsonValue::as_str)
        .ok_or("missing resolve kind")?;
    if kind != "documentation" {
        return Err("unsupported resolve kind");
    }
    let symbol = resolve
        .get("symbol")
        .and_then(service_completion_symbol)
        .ok_or("invalid resolve symbol")?;
    Ok(Some(CompletionResolvePayload::Documentation { symbol }))
}

pub(crate) fn lsp_completion_resolved_item(
    mut item: JsonValue,
    documentation: Option<String>,
) -> JsonValue {
    if let Some(documentation) = documentation {
        item["documentation"] = json!({
            "kind": "markdown",
            "value": documentation
        });
    }
    item
}

fn service_completion_symbol(value: &JsonValue) -> Option<CompletionSymbol> {
    let kind = value.get("kind")?.as_str()?;
    let name = value.get("name")?.as_str()?.to_owned();
    match kind {
        "source" => Some(CompletionSymbol::Source(name)),
        "schema" => Some(CompletionSymbol::Schema(name)),
        "builtin" => Some(CompletionSymbol::Builtin(name)),
        "local" => Some(CompletionSymbol::local(name)),
        _ => None,
    }
}
