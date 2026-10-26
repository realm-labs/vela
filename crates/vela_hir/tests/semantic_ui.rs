use std::collections::BTreeSet;
use std::path::Path;

use vela_common::diagnostic_render::{DiagnosticRenderer, DiagnosticSource};
use vela_common::{Diagnostic, SourceId, Span};
use vela_hir::binding::BindingResolution;
use vela_hir::module_graph::{ImportResolution, ModuleSource};
use vela_hir::source_ingestion::{HirSourceBuildErrorKind, build_module_source_set};
use vela_package::{ModulePath, PackageId};

#[rstest::rstest]
#[case::unresolved_name("unresolved_name", include_str!("fixtures/ui/semantic/unresolved_name.vela"), include_str!("fixtures/ui/semantic/unresolved_name.expected"))]
#[case::unresolved_import("unresolved_import", include_str!("fixtures/ui/semantic/unresolved_import.vela"), include_str!("fixtures/ui/semantic/unresolved_import.expected"))]
#[case::private_import("private_import", include_str!("fixtures/ui/semantic/private_import.vela"), include_str!("fixtures/ui/semantic/private_import.expected"))]
#[case::valid_import("valid_import", include_str!("fixtures/ui/semantic/valid_import.vela"), include_str!("fixtures/ui/semantic/valid_import.expected"))]
#[case::valid_name("valid_name", include_str!("fixtures/ui/semantic/valid_name.vela"), include_str!("fixtures/ui/semantic/valid_name.expected"))]
fn semantic_ui(#[case] name: &str, #[case] text: &str, #[case] expected: &str) {
    assert!(!text.is_empty());
    let source = |id, module, text| {
        ModuleSource::new(
            SourceId::new(id),
            PackageId::anonymous(),
            ModulePath::from_qualified(module),
            text,
        )
    };
    // Preserve the original unit-test identities and dependency-first insertion order.
    let sources = match name {
        "unresolved_name" | "valid_name" => vec![source(1, "game::player", text)],
        "private_import" => vec![
            source(
                1,
                "game::reward",
                include_str!("fixtures/modules/private_reward.vela"),
            ),
            source(2, "game::main", text),
        ],
        "unresolved_import" | "valid_import" => vec![
            source(
                1,
                "game::reward",
                include_str!("fixtures/modules/public_reward.vela"),
            ),
            source(2, "game::main", text),
        ],
        _ => panic!("unregistered semantic case {name}"),
    };
    let diagnostics = match name {
        "unresolved_name" => {
            let primary = span_of(1, text, "plaeyr");
            vec![
                Diagnostic::error("unresolved name `plaeyr`")
                    .with_code("hir::unresolved_name")
                    .with_span(primary)
                    .with_label(primary, "did you mean `player`?")
                    .with_label(
                        span_of(1, text, "player"),
                        "candidate `player` is declared here",
                    ),
            ]
        }
        "unresolved_import" => {
            let primary = span_of(2, text, text);
            vec![
                Diagnostic::error("unresolved import `grant_reward` in module `game::reward`")
                    .with_code("hir::unresolved_import")
                    .with_span(primary)
                    .with_label(primary, "did you mean `grant`?"),
            ]
        }
        "private_import" => {
            let primary = span_of(2, text, text);
            vec![
                Diagnostic::error("declaration `secret` in module `game::reward` is private")
                    .with_code("hir::private_import")
                    .with_span(primary)
                    .with_label(
                        primary,
                        "private declaration cannot be imported from another module",
                    )
                    .with_label(
                        span_of(1, &sources[0].text, &sources[0].text),
                        "declaration is private",
                    ),
            ]
        }
        "valid_import" | "valid_name" => Vec::new(),
        _ => unreachable!(),
    };
    let actual = match build_module_source_set(&sources) {
        Ok(built) => {
            assert!(diagnostics.is_empty(), "error case unexpectedly built");
            let graph = built.graph();
            assert_eq!(built.modules().len(), sources.len());
            for (module, source) in built.modules().iter().zip(&sources) {
                assert_eq!(graph.module_path(*module), Some(&source.path));
                assert_eq!(graph.module_package(*module), Some(&source.package));
            }
            let module = *built.modules().last().expect("main module");
            let main = graph
                .module(module)
                .expect("declarations")
                .get("main")
                .expect("main");
            let bindings = graph.bindings(main).expect("main bindings");
            if name == "valid_import" {
                let Some(ImportResolution::Declaration(grant)) =
                    graph.imports(module).expect("imports")[0].resolution
                else {
                    panic!("resolved grant import");
                };
                assert_eq!(
                    graph.declaration(grant).expect("grant").span.source,
                    SourceId::new(1)
                );
                assert!(bindings.resolutions().any(|(_, resolution)| resolution == &BindingResolution::Declaration(grant)));
            } else {
                let [player] = bindings.locals_named("player") else {
                    panic!("player parameter")
                };
                assert!(
                    bindings
                        .resolutions()
                        .any(|(_, resolution)| resolution == &BindingResolution::Local(*player))
                );
            }
            Vec::new()
        }
        Err(error) => {
            assert!(
                !diagnostics.is_empty(),
                "positive control failed: {error:?}"
            );
            assert_eq!(error.kind(), HirSourceBuildErrorKind::Semantic);
            error.into_diagnostics()
        }
    };
    assert_eq!(actual, diagnostics, "typed diagnostic contract for {name}");
    for diagnostic in &actual {
        for span in diagnostic
            .span
            .into_iter()
            .chain(diagnostic.labels.iter().map(|label| label.span))
        {
            let source = sources
                .iter()
                .find(|source| source.id == span.source)
                .expect("logical source ID");
            assert!(span.start < span.end);
            assert!(
                source
                    .text
                    .get(span.start as usize..span.end as usize)
                    .is_some(),
                "valid UTF-8 range: {span:?}"
            );
        }
    }
    let renderer = DiagnosticRenderer::new(sources.iter().map(|source| {
        DiagnosticSource::new(
            source.id,
            format!("{}.vela", source.path.join().replace("::", "/")),
            &source.text,
        )
    }));
    let rendered = actual
        .iter()
        .flat_map(|diagnostic| renderer.render(diagnostic))
        .collect::<Vec<_>>()
        .join("\n");
    // Exact UTF-8, including EOF; successful controls intentionally have empty goldens.
    assert_eq!(rendered, expected, "full semantic output for {name}");
}

fn span_of(id: u32, text: &str, needle: &str) -> Span {
    assert_eq!(
        text.matches(needle).count(),
        1,
        "unique source selection: {needle}"
    );
    let start = text.find(needle).expect("source selection");
    Span::new(
        SourceId::new(id),
        start as u32,
        (start + needle.len()) as u32,
    )
}

fn inventory(files: &BTreeSet<String>) -> Result<(), String> {
    if files.is_empty() {
        return Err("empty semantic corpus".into());
    }
    // Derive only the names of explicit rstest registrations, not a fixture schema.
    let registered = include_str!("semantic_ui.rs")
        .lines()
        .filter_map(|line| {
            line.strip_prefix("#[case::")
                .and_then(|line| line.split_once('('))
                .map(|(name, _)| name)
        })
        .collect::<BTreeSet<_>>();
    assert!(!registered.is_empty());
    let expected = registered
        .iter()
        .flat_map(|name| [format!("{name}.vela"), format!("{name}.expected")])
        .collect::<BTreeSet<_>>();
    if files != &expected {
        return Err(format!(
            "missing/orphan/unregistered semantic files: expected {expected:?}, got {files:?}"
        ));
    }
    Ok(())
}

#[test]
fn semantic_ui_inventory() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ui/semantic");
    let files = std::fs::read_dir(root)
        .expect("semantic corpus")
        .map(|entry| {
            let entry = entry.expect("semantic entry");
            assert!(entry.file_type().expect("file type").is_file());
            entry.file_name().into_string().expect("UTF-8 filename")
        })
        .collect();
    inventory(&files).expect("complete registered source/expected pairs");
}

#[test]
fn semantic_ui_inventory_rejects_missing_orphan_empty_and_unregistered_files() {
    assert!(inventory(&BTreeSet::new()).is_err());
    assert!(inventory(&BTreeSet::from(["unresolved_name.vela".into()])).is_err());
    assert!(inventory(&BTreeSet::from(["orphan.expected".into()])).is_err());
    assert!(
        inventory(&BTreeSet::from([
            "unregistered.vela".into(),
            "unregistered.expected".into()
        ]))
        .is_err()
    );
}
