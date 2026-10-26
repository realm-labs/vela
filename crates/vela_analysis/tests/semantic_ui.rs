use std::collections::BTreeSet;
use std::path::Path;

use vela_analysis::callable::{
    CallableParameterFact, CallableParameterRequirementFact, CallableSignatureFact,
};
use vela_analysis::executable::{ExecutableAnalysisGeneration, ExecutableAnalysisInput};
use vela_analysis::registry::RegistryFacts;
use vela_analysis::type_fact::TypeFact;
use vela_analysis::validation::{CallParameterSlotValueFact, CallPlacementModeFact};
use vela_common::diagnostic_render::{DiagnosticRenderer, DiagnosticSource};
use vela_common::{Severity, SourceId};
use vela_def::FunctionId;
use vela_hir::body::HirExprKind;
use vela_hir::module_graph::{ModuleGraph, ModuleSource};
use vela_package::{ModulePath, PackageId};

#[rstest::rstest]
#[case::identity("identity", include_str!("fixtures/ui/semantic/identity.vela"), include_str!("fixtures/ui/semantic/identity.expected"))]
#[case::call_placement("call_placement", include_str!("fixtures/ui/semantic/call_placement.vela"), include_str!("fixtures/ui/semantic/call_placement.expected"))]
#[case::constructors("constructors", include_str!("fixtures/ui/semantic/constructors.vela"), include_str!("fixtures/ui/semantic/constructors.expected"))]
#[case::registered_calls("registered_calls", include_str!("fixtures/ui/semantic/registered_calls.vela"), include_str!("fixtures/ui/semantic/registered_calls.expected"))]
#[case::valid_source("valid_source", include_str!("fixtures/ui/semantic/valid_source.vela"), include_str!("fixtures/ui/semantic/valid_source.expected"))]
#[case::valid_registry("valid_registry", include_str!("fixtures/ui/semantic/valid_registry.vela"), include_str!("fixtures/ui/semantic/valid_registry.expected"))]
fn semantic_ui(#[case] name: &str, #[case] text: &str, #[case] expected: &str) {
    assert!(!text.is_empty());
    // These are analysis diagnostics, not HIR build errors or bytecode errors.
    // Retain the source and executable identities of the corresponding unit cases.
    let (source, function) = match name {
        "identity" => (101, 10_101),
        "call_placement" => (106, 10_601),
        "constructors" => (202, 20_201),
        "registered_calls" => (108, 10_801),
        "valid_source" => (301, 30_101),
        "valid_registry" => (302, 30_201),
        _ => panic!("unregistered analysis case {name}"),
    };
    let source = SourceId::new(source);
    let function = FunctionId::new(function);
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(ModuleSource::new(
        source,
        PackageId::anonymous(),
        ModulePath::from_qualified("game"),
        text,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[], "HIR must succeed before analysis");
    let main = graph
        .module(module)
        .expect("game declarations")
        .get("main")
        .expect("main declaration");
    let body = graph.function_body(main).expect("main body");
    let input = [ExecutableAnalysisInput::new(function, body.id)];
    let generation = if matches!(name, "registered_calls" | "valid_registry") {
        let mut schema = RegistryFacts::default();
        schema.insert_function(
            "game::add",
            TypeFact::function(vec![TypeFact::I64, TypeFact::I64], TypeFact::I64),
        );
        schema.insert_function_signature(
            "game::add",
            CallableSignatureFact::new(
                [
                    CallableParameterFact::new(
                        "lhs",
                        TypeFact::I64,
                        CallableParameterRequirementFact::Required,
                    ),
                    CallableParameterFact::new(
                        "rhs",
                        TypeFact::I64,
                        CallableParameterRequirementFact::Defaulted,
                    ),
                ],
                TypeFact::I64,
            ),
        );
        ExecutableAnalysisGeneration::from_module_graph_and_schema(&graph, &schema, input)
    } else {
        ExecutableAnalysisGeneration::from_module_graph(&graph, input)
    }
    .expect("analysis generation");
    let view = generation
        .view(function)
        .expect("function-qualified main view");
    assert_eq!(view.root_body(), body.id);
    let diagnostics = view.validation_diagnostics();
    let contracts: &[(&str, &str, usize)] = match name {
        "identity" => &[
            ("compiler::invalid_identity_comparison", "1 === 1", 2),
            (
                "compiler::invalid_identity_comparison",
                "(1..2) === (1..2)",
                2,
            ),
        ],
        "call_placement" => &[
            ("compiler::unknown_named_argument", "amunt = 2", 2),
            ("compiler::positional_after_named_argument", "2", 1),
            ("compiler::too_many_arguments", "3", 1),
            ("compiler::duplicate_argument", "base = 2", 2),
            ("compiler::missing_required_argument", "grant()", 2),
        ],
        "constructors" => &[
            ("compiler::duplicate_constructor_field", "extra", 2),
            ("compiler::unknown_constructor_field", "extra", 2),
            ("compiler::unknown_constructor_field", "extra", 2),
            (
                "compiler::missing_constructor_field",
                "Reward { extra: 1, extra: 2 }",
                1,
            ),
            (
                "compiler::missing_constructor_field",
                "Reward { extra: 1, extra: 2 }",
                1,
            ),
            ("compiler::missing_constructor_field", "Item::Pair()", 1),
            ("compiler::unknown_constructor_field", "2", 2),
            (
                "compiler::unknown_constructor_variant",
                "Item::Missing { amount: 1 }",
                1,
            ),
        ],
        "registered_calls" => &[(
            "compiler::missing_required_argument",
            "game::add(rhs = 2)",
            1,
        )],
        "valid_source" | "valid_registry" => &[],
        _ => unreachable!(),
    };
    assert_eq!(
        diagnostics.len(),
        contracts.len(),
        "ordered diagnostic count: {diagnostics:?}"
    );
    for (diagnostic, (code, primary_text, labels)) in diagnostics.iter().zip(contracts) {
        assert_eq!(diagnostic.severity, Severity::Error);
        assert_eq!(diagnostic.code.as_deref(), Some(*code));
        assert_eq!(diagnostic.labels.len(), *labels, "{diagnostic:?}");
        assert!(diagnostic.candidates.is_empty());
        assert!(diagnostic.repairs.is_empty());
        let primary = diagnostic.span.expect("primary span");
        for span in std::iter::once(primary).chain(diagnostic.labels.iter().map(|label| label.span))
        {
            assert_eq!(span.source, source);
            assert!(span.start < span.end);
            assert!(
                text.get(span.start as usize..span.end as usize).is_some(),
                "valid UTF-8 range: {span:?}"
            );
        }
        assert_eq!(
            &text[primary.start as usize..primary.end as usize],
            *primary_text
        );
    }
    if matches!(name, "valid_source" | "valid_registry") {
        let calls = body
            .expressions
            .values()
            .filter(|expression| matches!(expression.kind, HirExprKind::Call(_)))
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        let placement = view
            .call_argument_placement(calls[0].id)
            .expect("positive call placement");
        assert_eq!(
            placement.mode,
            if name == "valid_registry" {
                CallPlacementModeFact::ExternalNamed
            } else {
                CallPlacementModeFact::Strict
            }
        );
        let slots = placement
            .parameter_slots
            .as_ref()
            .expect("resolved parameter slots");
        assert_eq!(
            slots
                .iter()
                .map(|slot| match slot.value {
                    CallParameterSlotValueFact::Explicit { source_index, .. } => Some(source_index),
                    CallParameterSlotValueFact::MissingDefault => None,
                })
                .collect::<Vec<_>>(),
            [Some(1), Some(0)]
        );
        assert_eq!(
            placement
                .source_order
                .iter()
                .map(|argument| argument.name.as_deref())
                .collect::<Vec<_>>(),
            if name == "valid_registry" {
                [Some("rhs"), Some("lhs")]
            } else {
                [Some("amount"), Some("base")]
            }
        );
    }
    let renderer = DiagnosticRenderer::new([DiagnosticSource::new(source, "game.vela", text)]);
    let rendered = diagnostics
        .iter()
        .flat_map(|diagnostic| renderer.render(diagnostic))
        .collect::<Vec<_>>()
        .join("\n");
    // No trimming, newline conversion, path rewriting, or output generation during comparison.
    assert_eq!(rendered, expected, "full analysis output for {name}");
}

fn inventory(files: &BTreeSet<String>) -> Result<(), String> {
    if files.is_empty() {
        return Err("empty semantic corpus".into());
    }
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
    assert!(inventory(&BTreeSet::from(["identity.vela".into()])).is_err());
    assert!(inventory(&BTreeSet::from(["orphan.expected".into()])).is_err());
    assert!(
        inventory(&BTreeSet::from([
            "unregistered.vela".into(),
            "unregistered.expected".into()
        ]))
        .is_err()
    );
}
