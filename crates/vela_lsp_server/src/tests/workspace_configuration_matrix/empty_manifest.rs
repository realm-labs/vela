use super::support::*;
use serde_json::Value;

#[test]
fn empty_then_invalid_native_manifest_keeps_scratch_until_package_repair_and_delete() {
    for crlf in [false, true] {
        let mut s = load("input-workspace-roots");
        if crlf {
            for text in s.files.values_mut() {
                *text = text.replace('\n', "\r\n");
            }
        }
        let layout = Layout::new(&s);
        let native = load("input-invalid-config-schema");
        let marked = native.oracle["invalidManifest"]
            .as_str()
            .expect("invalid TOML");
        let bad = parse_markers(&marked.replace('\n', if crlf { "\r\n" } else { "\n" }))
            .expect("independent byte markers");
        let good = native.oracle["validManifest"]
            .as_str()
            .expect("valid TOML")
            .replace('\n', if crlf { "\r\n" } else { "\n" });
        let left = "ux17-roots/left/cross.vela";
        let target = "ux17-roots/left/alpha.vela";
        let right = "ux17-roots/right/other_caller.vela";
        let beta = "ux17-roots/right/beta.vela";
        let manifest = "ux17-roots/left/vela.toml";
        let prefix = s.oracle["dirtyPrefix"]
            .as_str()
            .expect("dirty prefix")
            .replace('\n', if crlf { "\r\n" } else { "\n" });
        let caller = parse_markers(&format!("{prefix}{}", s.files[left])).expect("dirty caller");
        let alpha = parse_markers(
            &s.oracle["overlay"]
                .as_str()
                .expect("dirty target")
                .replace('\n', if crlf { "\r\n" } else { "\n" }),
        )
        .expect("dirty definition");
        let other = doc(&s, right);
        let beta_source = doc(&s, beta);
        for profile in profiles() {
            let mut server = session(
                &layout,
                profile.clone(),
                &["ux17-roots/right", "ux17-roots/left"],
                Value::Null,
            );
            open(&mut server, &layout.uri(left), &caller.text, 73);
            open(&mut server, &layout.uri(target), &alpha.text, 91);
            open(&mut server, &layout.uri(right), &other.text, 47);
            let frozen = server.snapshot();
            definition(
                &mut server,
                &layout,
                left,
                &caller,
                "alpha",
                Some((target, &alpha)),
            );
            definition(
                &mut server,
                &layout,
                left,
                &caller,
                "beta",
                Some((beta, &beta_source)),
            );
            let scratch = vec![
                missing(&s, &layout, left, &caller, "import-alpha", "alpha"),
                missing(&s, &layout, left, &caller, "import-beta", "beta"),
            ];
            layout.write_disk(manifest, "");
            publications(
                &watch(&mut server, &layout, &[(manifest, 1)]),
                vec![
                    (layout.uri(left), scratch.clone()),
                    (layout.uri(target), vec![]),
                    (layout.uri(right), vec![]),
                ],
                has_progress(&profile),
            );
            definition(&mut server, &layout, left, &caller, "alpha", None);
            definition(&mut server, &layout, left, &caller, "beta", None);
            definition(
                &mut server,
                &layout,
                right,
                &other,
                "call",
                Some((beta, &beta_source)),
            );
            layout.write_disk(manifest, &bad.text);
            let marker = bad.markers["bad"];
            let message = format!(
                "source.roots must be an array of strings at bytes {}..{}",
                marker.start.byte, marker.end.byte
            );
            publications(
                &watch(&mut server, &layout, &[(manifest, 2)]),
                vec![
                    (layout.uri(left), scratch),
                    (layout.uri(target), vec![]),
                    (layout.uri(right), vec![]),
                    (
                        layout.uri(manifest),
                        metadata_error("project::diagnostic", &message),
                    ),
                ],
                has_progress(&profile),
            );
            definition(&mut server, &layout, left, &caller, "alpha", None);
            definition(&mut server, &layout, left, &caller, "beta", None);
            layout.write_disk(manifest, &good);
            publications(
                &watch(&mut server, &layout, &[(manifest, 2)]),
                vec![
                    (
                        layout.uri(left),
                        vec![missing(&s, &layout, left, &caller, "import-beta", "beta")],
                    ),
                    (layout.uri(target), vec![]),
                    (layout.uri(right), vec![]),
                    (layout.uri(manifest), vec![]),
                    (layout.uri("ux17-roots/left/schema-one.json"), vec![]),
                ],
                has_progress(&profile),
            );
            definition(
                &mut server,
                &layout,
                left,
                &caller,
                "alpha",
                Some((target, &alpha)),
            );
            definition(&mut server, &layout, left, &caller, "beta", None);
            layout.remove_disk(manifest);
            publications(
                &watch(&mut server, &layout, &[(manifest, 3)]),
                vec![
                    (layout.uri(left), vec![]),
                    (layout.uri(target), vec![]),
                    (layout.uri(right), vec![]),
                    (layout.uri(manifest), vec![]),
                    (layout.uri("ux17-roots/left/schema-one.json"), vec![]),
                ],
                has_progress(&profile),
            );
            definition(
                &mut server,
                &layout,
                left,
                &caller,
                "alpha",
                Some((target, &alpha)),
            );
            definition(
                &mut server,
                &layout,
                left,
                &caller,
                "beta",
                Some((beta, &beta_source)),
            );
            for snapshot in [frozen, server.snapshot()] {
                for (file, version, text) in [
                    (left, 73, &caller.text),
                    (target, 91, &alpha.text),
                    (right, 47, &other.text),
                ] {
                    let document = snapshot
                        .workspace()
                        .document(&layout.id(file))
                        .expect("retained overlay");
                    assert_eq!(document.version().get(), version);
                    assert_eq!(document.text(), text);
                }
            }
        }
        layout.check_disk(&s);
    }
}
