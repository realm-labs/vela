use super::support::*;
use crate::LaunchConfiguration;
use serde_json::json;
use vela_language_service::{DocumentId, SourceVersion};

fn client_uri(layout: &Layout, file: &str) -> (String, String) {
    // Native client inputs encode lowercase drives. Expected outgoing file URIs
    // use standard fixture serialization and must retain the alias directory.
    let uri = layout.uri(file);
    #[cfg(windows)]
    {
        assert!(uri.starts_with("file:///"));
        assert_eq!(uri.as_bytes()[9], b':');
        let drive = char::from(uri.as_bytes()[8]).to_ascii_lowercase();
        let suffix = &uri[10..];
        (format!("file:///{drive}%3A{suffix}"), uri)
    }
    #[cfg(not(windows))]
    {
        (uri.clone(), uri)
    }
}

fn root_alias(layout: &Layout, owner: &str) {
    let target = layout.path(owner);
    let alias = layout.path(&format!("{owner}_alias"));
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, alias).expect("owned root alias");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let quote = |path: &std::path::Path| {
            format!("'{}'", path.display().to_string().replace('\'', "''"))
        };
        let command = format!(
            "New-Item -ItemType Junction -Path {} -Target {} -ErrorAction Stop | Out-Null",
            quote(&alias),
            quote(&target)
        );
        let mut child = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command"])
            .arg(command)
            .creation_flags(0x0800_0000)
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("create owned junction without a console window");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if let Some(status) = child.try_wait().expect("junction helper status") {
                assert!(status.success(), "owned junction helper failed: {status}");
                break;
            }
            if std::time::Instant::now() >= deadline {
                child.kill().expect("stop owned junction helper");
                child.wait().expect("reap owned junction helper");
                panic!("owned junction helper exceeded its 10-second budget");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

#[test]
fn independent_manifest_errors_keep_client_root_spelling_and_last_valid_dirty_sources() {
    for crlf in [false, true] {
        let mut s = load("workspace-packages");
        let ending = if crlf { "\r\n" } else { "\n" };
        if crlf {
            for text in s.files.values_mut() {
                *text = text.replace('\n', ending);
            }
        }
        let layout = Layout::new(&s);
        for owner in ["first", "second"] {
            root_alias(&layout, owner);
        }
        let owners = ["first_alias", "second_alias"];
        let first_file = format!("{}/first_probe.vela", owners[0]);
        let second_file = format!("{}/calls/second_probe.vela", owners[1]);
        let manifest_file = format!("{}/vela.toml", owners[1]);
        let (first_uri, _) = client_uri(&layout, &first_file);
        let (second_uri, _) = client_uri(&layout, &second_file);
        let (manifest_event_uri, manifest_owner_uri) = client_uri(&layout, &manifest_file);
        let prefix = s.oracle["dirtyPrefix"]
            .as_str()
            .expect("authored prefix")
            .replace('\n', ending);
        let first = parse_markers(&format!("{prefix}{}", s.files["first/first_probe.vela"]))
            .expect("first dirty caller");
        let second = parse_markers(&format!(
            "{prefix}{}",
            s.files["second/calls/second_probe.vela"]
        ))
        .expect("second dirty caller");
        let invalid = parse_markers(
            &s.oracle["invalidManifest"]
                .as_str()
                .expect("invalid manifest")
                .replace('\n', ending),
        )
        .expect("authored invalid span");
        let bad = invalid.markers["bad"];
        let error = s.oracle["invalidMessage"]
            .as_str()
            .expect("authored error")
            .replace("{START}", &bad.start.byte.to_string())
            .replace("{END}", &bad.end.byte.to_string());
        let mut launch = LaunchConfiguration::new();
        launch.set_watch_files_enabled(false);
        let mut server = TestServer::with_launch_configuration(launch);
        let init = send(
            &mut server,
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
                "processId":null,"capabilities":{},"workspaceFolders":owners.iter().map(|owner|json!({"uri":client_uri(&layout,owner).0,"name":owner})).collect::<Vec<_>>()
            }}),
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
        publications(
            &open(&mut server, &first_uri, &first.text, 73),
            vec![(first_uri.clone(), vec![])],
            false,
        );
        publications(
            &open(&mut server, &second_uri, &second.text, 91),
            vec![(second_uri.clone(), vec![])],
            false,
        );
        let frozen = server.snapshot();
        layout.write_disk("second/vela.toml", &invalid.text);
        publications(
            &send(
                &mut server,
                json!({"jsonrpc":"2.0","method":"workspace/didChangeWatchedFiles","params":{"changes":[{"uri":manifest_event_uri,"type":2}]}}),
            ),
            vec![
                (first_uri.clone(), vec![]),
                (second_uri.clone(), vec![]),
                (
                    manifest_owner_uri.clone(),
                    metadata_error("project::diagnostic", &error),
                ),
            ],
            false,
        );
        assert_eq!(server.snapshot().generation(), frozen.generation());
        for (file, uri, caller, target_file, target, version) in [
            (
                &first_file,
                &first_uri,
                &first,
                format!("{}/shared_defs.vela", owners[0]),
                doc(&s, "first/shared_defs.vela"),
                73,
            ),
            (
                &second_file,
                &second_uri,
                &second,
                format!("{}/original/shared_defs.vela", owners[1]),
                doc(&s, "second/original/shared_defs.vela"),
                91,
            ),
        ] {
            let point = caller.markers["call"].start;
            let target_uri = client_uri(&layout, &target_file).1;
            assert_eq!(
                send(
                    &mut server,
                    json!({"jsonrpc":"2.0","id":31,"method":"textDocument/definition","params":{"textDocument":{"uri":uri},"position":{"line":point.line,"character":point.character}}})
                ),
                vec![
                    json!({"jsonrpc":"2.0","id":31,"result":{"uri":target_uri,"range":span(&target,"decl")}})
                ],
                "{file}"
            );
            let id = DocumentId::from(uri.clone());
            assert_eq!(
                server
                    .snapshot()
                    .workspace()
                    .document(&id)
                    .expect("dirty owner")
                    .version(),
                SourceVersion::new(version)
            );
            assert_eq!(
                frozen.databases().source_db().records()[&id].text(),
                caller.text
            );
        }
        layout.write_disk("second/vela.toml", &s.files["second/vela.toml"]);
        publications(
            &send(
                &mut server,
                json!({"jsonrpc":"2.0","method":"workspace/didChangeWatchedFiles","params":{"changes":[{"uri":client_uri(&layout,&manifest_file).0,"type":2}]}}),
            ),
            vec![
                (first_uri, vec![]),
                (second_uri, vec![]),
                (manifest_owner_uri, vec![]),
            ],
            false,
        );
        layout.check_disk(&s);
    }
}
