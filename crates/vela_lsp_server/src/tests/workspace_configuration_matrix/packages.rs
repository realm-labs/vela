use super::support::*;
use serde_json::Value;
use vela_language_service::SourceVersion;

fn package_spec(crlf: bool) -> Spec {
    let mut s = load("workspace-packages");
    if crlf {
        for text in s.files.values_mut() {
            *text = text.replace('\n', "\r\n");
        }
    }
    s
}

fn dirty(s: &Spec, file: &str, crlf: bool) -> Document {
    let prefix = s.oracle["dirtyPrefix"].as_str().expect("authored prefix");
    let prefix = prefix.replace('\n', if crlf { "\r\n" } else { "\n" });
    parse_markers(&format!("{prefix}{}", s.files[file])).expect("authored dirty source")
}

#[test]
fn independent_package_roots_keep_same_named_modules_in_their_own_packages() {
    for crlf in [false, true] {
        let s = package_spec(crlf);
        let layout = Layout::new(&s);
        for profile in profiles() {
            let mut server = session(&layout, profile.clone(), &["first", "second"], Value::Null);
            for owner in ["first", "second"] {
                let caller_file = s.oracle["packages"][owner]["caller"]
                    .as_str()
                    .expect("authored caller");
                let caller = dirty(&s, caller_file, crlf);
                let target_file = s.oracle["packages"][owner]["target"]
                    .as_str()
                    .expect("authored target");
                let target = doc(&s, target_file);
                publications(
                    &open(&mut server, &layout.uri(caller_file), &caller.text, 73),
                    vec![(layout.uri(caller_file), vec![])],
                    false,
                );
                definition(
                    &mut server,
                    &layout,
                    caller_file,
                    &caller,
                    "call",
                    Some((target_file, &target)),
                );
            }
            let caller = dirty(&s, "first/first_probe.vela", crlf);
            let target = doc(&s, "first/shared_defs.vela");
            folders(&mut server, &layout, &[], &["first"]);
            definition(
                &mut server,
                &layout,
                "first/first_probe.vela",
                &caller,
                "call",
                None,
            );
            folders(&mut server, &layout, &["first"], &[]);
            definition(
                &mut server,
                &layout,
                "first/first_probe.vela",
                &caller,
                "call",
                Some(("first/shared_defs.vela", &target)),
            );
            layout.check_disk(&s);
        }
    }
}

#[test]
fn second_project_manifest_switch_and_repair_keep_first_owner_and_dirty_callers() {
    for crlf in [false, true] {
        let s = package_spec(crlf);
        let layout = Layout::new(&s);
        let first = dirty(&s, "first/first_probe.vela", crlf);
        let second = dirty(&s, "second/calls/second_probe.vela", crlf);
        let first_target = doc(&s, "first/shared_defs.vela");
        let original = doc(&s, "second/original/shared_defs.vela");
        let replacement = doc(&s, "second/replacement/shared_defs.vela");
        let endings = |text: &str| text.replace('\n', if crlf { "\r\n" } else { "\n" });
        let changed = endings(
            s.oracle["manifestReplacement"]
                .as_str()
                .expect("authored manifest"),
        );
        let invalid = parse_markers(&endings(
            s.oracle["invalidManifest"]
                .as_str()
                .expect("authored invalid manifest"),
        ))
        .expect("authored invalid span");
        let bad = invalid.markers["bad"];
        let error = s.oracle["invalidMessage"]
            .as_str()
            .expect("authored diagnostic")
            .replace("{START}", &bad.start.byte.to_string())
            .replace("{END}", &bad.end.byte.to_string());
        for profile in profiles() {
            let mut server = session(&layout, profile.clone(), &["first", "second"], Value::Null);
            open(
                &mut server,
                &layout.uri("first/first_probe.vela"),
                &first.text,
                73,
            );
            open(
                &mut server,
                &layout.uri("second/calls/second_probe.vela"),
                &second.text,
                91,
            );
            let frozen = server.snapshot();
            layout.write_disk("second/vela.toml", &changed);
            publications(
                &watch(&mut server, &layout, &[("second/vela.toml", 2)]),
                vec![
                    (layout.uri("first/first_probe.vela"), vec![]),
                    (layout.uri("second/calls/second_probe.vela"), vec![]),
                ],
                has_progress(&profile),
            );
            definition(
                &mut server,
                &layout,
                "first/first_probe.vela",
                &first,
                "call",
                Some(("first/shared_defs.vela", &first_target)),
            );
            definition(
                &mut server,
                &layout,
                "second/calls/second_probe.vela",
                &second,
                "call",
                Some(("second/replacement/shared_defs.vela", &replacement)),
            );
            let valid_generation = server.snapshot().generation();
            layout.write_disk("second/vela.toml", &invalid.text);
            publications(
                &watch(&mut server, &layout, &[("second/vela.toml", 2)]),
                vec![
                    (layout.uri("first/first_probe.vela"), vec![]),
                    (layout.uri("second/calls/second_probe.vela"), vec![]),
                    (
                        layout.uri("second/vela.toml"),
                        metadata_error("project::diagnostic", &error),
                    ),
                ],
                has_progress(&profile),
            );
            assert_eq!(server.snapshot().generation(), valid_generation);
            definition(
                &mut server,
                &layout,
                "first/first_probe.vela",
                &first,
                "call",
                Some(("first/shared_defs.vela", &first_target)),
            );
            definition(
                &mut server,
                &layout,
                "second/calls/second_probe.vela",
                &second,
                "call",
                Some(("second/replacement/shared_defs.vela", &replacement)),
            );
            layout.write_disk("second/vela.toml", &s.files["second/vela.toml"]);
            publications(
                &watch(&mut server, &layout, &[("second/vela.toml", 2)]),
                vec![
                    (layout.uri("first/first_probe.vela"), vec![]),
                    (layout.uri("second/calls/second_probe.vela"), vec![]),
                    (layout.uri("second/vela.toml"), vec![]),
                ],
                has_progress(&profile),
            );
            definition(
                &mut server,
                &layout,
                "second/calls/second_probe.vela",
                &second,
                "call",
                Some(("second/original/shared_defs.vela", &original)),
            );
            let current = server.snapshot();
            for (file, caller, version) in [
                ("first/first_probe.vela", &first, 73),
                ("second/calls/second_probe.vela", &second, 91),
            ] {
                let id = layout.id(file);
                assert_eq!(
                    current
                        .workspace()
                        .document(&id)
                        .expect("dirty caller")
                        .version(),
                    SourceVersion::new(version)
                );
                assert_eq!(
                    current.databases().source_db().records()[&id].text(),
                    caller.text
                );
                assert_eq!(
                    frozen.databases().source_db().records()[&id].text(),
                    caller.text
                );
            }
            assert_eq!(
                frozen.databases().source_db().records()
                    [&layout.id("second/original/shared_defs.vela")]
                    .text(),
                original.text
            );
            assert!(
                !frozen
                    .databases()
                    .source_db()
                    .records()
                    .contains_key(&layout.id("second/replacement/shared_defs.vela"))
            );
            let mut fresh = session(&layout, profile, &["first", "second"], Value::Null);
            open(
                &mut fresh,
                &layout.uri("first/first_probe.vela"),
                &first.text,
                73,
            );
            open(
                &mut fresh,
                &layout.uri("second/calls/second_probe.vela"),
                &second.text,
                91,
            );
            definition(
                &mut fresh,
                &layout,
                "first/first_probe.vela",
                &first,
                "call",
                Some(("first/shared_defs.vela", &first_target)),
            );
            definition(
                &mut fresh,
                &layout,
                "second/calls/second_probe.vela",
                &second,
                "call",
                Some(("second/original/shared_defs.vela", &original)),
            );
            layout.check_disk(&s);
        }
    }
}

#[test]
fn manifest_and_plain_workspace_roots_both_discover_sources_without_watch_events() {
    for crlf in [false, true] {
        let s = package_spec(crlf);
        let layout = Layout::new(&s);
        for profile in profiles() {
            let mut server = session(&layout, profile, &["first", "plain"], Value::Null);
            for (caller_file, target_file) in [
                ("first/first_probe.vela", "first/shared_defs.vela"),
                ("plain/plain_probe.vela", "plain/sprout.vela"),
            ] {
                let caller = dirty(&s, caller_file, crlf);
                let target = doc(&s, target_file);
                publications(
                    &open(&mut server, &layout.uri(caller_file), &caller.text, 73),
                    vec![(layout.uri(caller_file), vec![])],
                    false,
                );
                definition(
                    &mut server,
                    &layout,
                    caller_file,
                    &caller,
                    "call",
                    Some((target_file, &target)),
                );
            }
            layout.check_disk(&s);
        }
    }
}
