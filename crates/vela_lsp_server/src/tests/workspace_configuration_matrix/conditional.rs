use super::support::*;
use serde_json::{Value, json};
use vela_language_service::SourceVersion;

#[test]
fn conditional_configuration_absence_and_unsolicited_responses_preserve_defaults() {
    for crlf in [false, true] {
        let s = spec(crlf);
        let layout = Layout::new(&s);
        let caller = doc(&s, "roots/left/caller.vela");
        let alpha = doc(&s, "roots/left/alpha.vela");
        let schema = doc(&s, "roots/left/schema_caller.vela");
        for mut profile in profiles() {
            for supported in [false, true] {
                profile["workspace"]["configuration"] = json!(supported);
                let mut server = session(&layout, profile.clone(), &[""], Value::Null);
                publications(
                    &open(
                        &mut server,
                        &layout.uri("roots/left/caller.vela"),
                        &caller.text,
                        73,
                    ),
                    vec![(layout.uri("roots/left/caller.vela"), vec![])],
                    false,
                );
                let frozen = server.snapshot();
                let settings = json!({"workspace":{"roots":[layout.uri("roots/right")]},"host":{"schema":layout.uri("schema-two.json")}});
                for id in [
                    json!(0),
                    json!(41),
                    json!("41"),
                    json!("workspace-init 中😀"),
                ] {
                    for result in [
                        Value::Null,
                        json!(false),
                        json!([]),
                        json!([settings.clone()]),
                        settings.clone(),
                    ] {
                        assert!(
                            send(
                                &mut server,
                                json!({"jsonrpc":"2.0","id":id,"result":result})
                            )
                            .is_empty()
                        );
                        let current = server.snapshot();
                        assert_eq!(current.generation(), frozen.generation());
                        assert_eq!(current.workspace_config(), frozen.workspace_config());
                        let buffer = current
                            .workspace()
                            .document(&layout.id("roots/left/caller.vela"))
                            .expect("overlay");
                        assert_eq!(
                            (buffer.text(), buffer.version()),
                            (caller.text.as_str(), SourceVersion::new(73))
                        );
                        definition(
                            &mut server,
                            &layout,
                            "roots/left/caller.vela",
                            &caller,
                            "call",
                            Some(("roots/left/alpha.vela", &alpha)),
                        );
                        schema_queries(
                            &mut server,
                            &layout,
                            &schema,
                            &s.oracle["schemaStates"][0]["fields"],
                        );
                    }
                    assert!(send(&mut server, json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":"client settings unavailable"}})).is_empty());
                }
                assert_eq!(
                    send(
                        &mut server,
                        json!({"jsonrpc":"2.0","id":"config中😀","method":"workspace/configuration","params":{"items":[{"section":"vela"}]}})
                    ),
                    vec![
                        json!({"jsonrpc":"2.0","id":"config中😀","error":{"code":-32601,"message":"method `workspace/configuration` is not implemented"}})
                    ]
                );
                assert!(send(&mut server,json!({"jsonrpc":"2.0","method":"workspace/configuration","params":{"items":[]}})).is_empty());
                assert_eq!(server.snapshot().generation(), frozen.generation());
            }
        }
        layout.check_disk(&s);
    }
}

#[test]
fn configuration_notifications_and_responses_obey_session_boundaries() {
    let s = fallback(false);
    let layout = Layout::new(&s);
    let caller = doc(&s, "roots/left/caller.vela");
    let alpha = doc(&s, "roots/left/alpha.vela");
    let mut launch = crate::LaunchConfiguration::new();
    launch.set_watch_files_enabled(false);
    launch.add_workspace_root(layout.uri("roots/left"));
    let mut server = TestServer::with_launch_configuration(launch);
    let settings = json!({"workspace":{"roots":[layout.uri("roots/right")]}});
    let before = server.snapshot();
    for phase in [0, 1, 2] {
        if phase == 1 {
            let init = send(
                &mut server,
                json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"capabilities":{}}}),
            );
            assert_eq!(init.len(), 1);
            assert!(init[0].get("error").is_none());
            assert!(
                send(
                    &mut server,
                    json!({"jsonrpc":"2.0","method":"initialized","params":{}})
                )
                .is_empty()
            );
            open(
                &mut server,
                &layout.uri("roots/left/caller.vela"),
                &caller.text,
                73,
            );
        }
        if phase == 2 {
            assert_eq!(
                send(
                    &mut server,
                    json!({"jsonrpc":"2.0","id":2,"method":"shutdown","params":null})
                ),
                vec![json!({"jsonrpc":"2.0","id":2,"result":null})]
            );
        }
        let frozen = server.snapshot();
        for params in [
            Value::Null,
            json!({}),
            json!({"settingsMissing":settings}),
            json!({"settings":settings.clone()}),
        ] {
            if phase == 1 && params.get("settings").is_some() {
                continue;
            }
            assert!(send(&mut server,json!({"jsonrpc":"2.0","method":"workspace/didChangeConfiguration","params":params})).is_empty());
        }
        if phase != 1 {
            assert!(folders(&mut server, &layout, &["roots/right"], &["roots/left"]).is_empty());
        }
        assert!(
            send(
                &mut server,
                json!({"jsonrpc":"2.0","id":41,"result":[settings.clone()]})
            )
            .is_empty()
        );
        assert_eq!(server.snapshot().generation(), frozen.generation());
        assert_eq!(
            server.snapshot().workspace_config(),
            frozen.workspace_config()
        );
        if phase == 1 {
            definition(
                &mut server,
                &layout,
                "roots/left/caller.vela",
                &caller,
                "call",
                Some(("roots/left/alpha.vela", &alpha)),
            );
        }
    }
    assert_eq!(
        before.workspace_config(),
        server.snapshot().workspace_config()
    );
    layout.check_disk(&s);
}
