use crate::{facts::AnalysisFacts, registry::RegistryFacts, type_fact::TypeFact};
use vela_common::SourceId;
use vela_hir::{
    body::HirExprKind,
    module_graph::{ModuleGraph, ModuleSource},
};
use vela_package::{ModulePath, PackageId};

#[test]
fn registry_record_constructor_results_keep_exact_owner_and_refresh_fields() {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(221),
        PackageId::anonymous(),
        ModulePath::from_qualified("main"),
        r#"
struct Local { tag: i64 }
fn run() {
    let a = host::Token { tag: 1 }; a.tag;
    let b = other::Token { tag: true }; b.tag;
    let c = Local { tag: 1 }; c.tag;
    let d = missing::Token { tag: 1 }; d.tag;
}
"#,
    ));
    graph.resolve_imports();
    for value in [
        Some(TypeFact::I64),
        Some(TypeFact::BOOL),
        None,
        Some(TypeFact::STRING),
    ] {
        let mut schema = RegistryFacts::default();
        if let Some(value) = &value {
            schema.insert_type("host::Token", TypeFact::record("host::Token"));
            schema.insert_field("host::Token", "tag", value.clone());
        }
        schema.insert_type("other::Token", TypeFact::record("other::Token"));
        schema.insert_field("other::Token", "tag", TypeFact::BOOL);
        schema.insert_type("main::Local", TypeFact::record("main::Local"));
        schema.insert_field("main::Local", "tag", TypeFact::STRING);
        let facts = AnalysisFacts::from_module_graph_and_schema(&graph, &schema);
        let body = graph
            .bodies()
            .find(|b| {
                b.expressions
                    .values()
                    .any(|e| matches!(e.kind, HirExprKind::Field(_)))
            })
            .expect("body");
        let mut fields = body
            .expressions
            .values()
            .filter(|e| matches!(e.kind, HirExprKind::Field(_)))
            .collect::<Vec<_>>();
        fields.sort_by_key(|e| e.origin.span.start);
        for (field, (receiver, expected)) in fields.iter().zip([
            (
                value
                    .as_ref()
                    .map_or(TypeFact::Unknown, |_| TypeFact::record("host::Token")),
                value.clone().unwrap_or(TypeFact::Unknown),
            ),
            (TypeFact::record("other::Token"), TypeFact::BOOL),
            (TypeFact::record("main::Local"), TypeFact::I64),
            (TypeFact::Unknown, TypeFact::Unknown),
        ]) {
            let HirExprKind::Field(access) = &field.kind else {
                unreachable!()
            };
            assert_eq!(
                facts.expression(access.receiver),
                (receiver != TypeFact::Unknown).then_some(&receiver)
            );
            assert_eq!(
                facts.expression(field.id),
                (expected != TypeFact::Unknown).then_some(&expected)
            );
        }
        assert_eq!(fields.len(), 4);
    }
}
