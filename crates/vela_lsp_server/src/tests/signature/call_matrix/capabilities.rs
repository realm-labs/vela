use serde_json::{Value, json};

pub(super) struct Client {
    pub(super) id: &'static str,
    pub(super) capabilities: Value,
    pub(super) contexts: Vec<Value>,
}

pub(super) fn minimal() -> Client {
    Client {
        id: "minimal",
        capabilities: json!({}),
        contexts: vec![Value::Null],
    }
}

pub(super) fn profiles() -> Vec<Client> {
    let contexts = vec![
        Value::Null,
        json!({"triggerKind":1,"isRetrigger":false}),
        json!({"triggerKind":2,"triggerCharacter":"(","isRetrigger":false}),
        json!({"triggerKind":2,"triggerCharacter":",","isRetrigger":true,
            "activeSignatureHelp":{"activeSignature":0,"activeParameter":0,
                "signatures":[{"label":"stale previous target","parameters":[]}]}}),
        json!({"triggerKind":3,"isRetrigger":true,
            "activeSignatureHelp":{"activeSignature":0,"activeParameter":0,
                "signatures":[{"label":"stale previous target","parameters":[]}]}}),
    ];
    let mut clients = vec![minimal()];
    clients[0].contexts = contexts.clone();
    for (id, offsets, active, context_support, dynamic, formats) in [
        (
            "editor",
            true,
            true,
            true,
            true,
            vec!["markdown", "plaintext"],
        ),
        ("legacy", false, false, false, false, vec!["plaintext"]),
        (
            "offsets-without-active",
            true,
            false,
            true,
            false,
            vec!["markdown"],
        ),
        (
            "active-without-offsets",
            false,
            true,
            false,
            true,
            vec!["plaintext", "markdown"],
        ),
    ] {
        clients.push(Client {
            id,
            capabilities: json!({"textDocument":{"signatureHelp":{
                "dynamicRegistration":dynamic,"contextSupport":context_support,
                "signatureInformation":{"documentationFormat":formats,
                    "parameterInformation":{"labelOffsetSupport":offsets},
                    "activeParameterSupport":active}
            }}}),
            contexts: contexts.clone(),
        });
    }
    clients
}
