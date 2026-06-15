use serde_json::{Value as JsonValue, json};
use vela_language_service::SignatureHelp;

pub(crate) fn lsp_signature_help(help: &SignatureHelp) -> JsonValue {
    json!({
        "signatures": help.signatures().iter().map(lsp_signature_information).collect::<Vec<_>>(),
        "activeSignature": help.active_signature(),
        "activeParameter": help.active_parameter()
    })
}

fn lsp_signature_information(signature: &vela_language_service::SignatureInformation) -> JsonValue {
    json!({
        "label": signature.label(),
        "parameters": signature.parameters().iter().map(lsp_signature_parameter).collect::<Vec<_>>()
    })
}

fn lsp_signature_parameter(parameter: &vela_language_service::SignatureParameter) -> JsonValue {
    json!({ "label": parameter.label() })
}
