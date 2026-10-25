use vela_common::SourceId;
use vela_def::DefPath;
use vela_hir::body::HirExprKind;
use vela_hir::module_graph::{ModuleGraph, ModuleSource};
use vela_package::ModulePath;
use vela_registry::{DefinitionRegistry, FieldDef, TypeDef, TypeKindDef, VariantDef};

use crate::facts::AnalysisFacts;
use crate::registry::RegistryFacts;
use crate::semantic_facts::MemberTargetFact;
use crate::type_fact::TypeFact;

#[test]
fn registered_enum_fields_require_an_exact_variant_not_transparent_host_fallback() {
    for variant_names in [&["Active"][..], &["Active", "Complete"][..]] {
        let mut registry = DefinitionRegistry::new();
        let progress = registry
            .register_type(
                TypeDef::new(DefPath::ty("host", ["records"], "Progress"))
                    .kind(TypeKindDef::ScriptEnum),
            )
            .expect("valid enum metadata and count field");
        for name in variant_names {
            let variant = registry
                .register_variant(VariantDef::new(
                    DefPath::variant("host", ["records"], "Progress", *name),
                    progress,
                ))
                .expect("valid enum metadata and count field");
            registry
                .register_field(
                    FieldDef::new(
                        DefPath::field("host", ["records"], format!("Progress::{name}"), "count"),
                        progress,
                    )
                    .variant_owner(variant)
                    .type_hint(Some("i64")),
                )
                .expect("valid enum metadata and count field");
        }
        let schema = RegistryFacts::from_compile_view(registry.compile_view())
            .expect("valid enum metadata and count field");
        assert_eq!(
            schema.field_fact("records::Progress::Active", "count"),
            Some(&TypeFact::I64)
        );
        assert!(schema.field_fact("records::Progress", "count").is_none());
        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            SourceId::new(91),
            vela_package::PackageId::anonymous(),
            ModulePath::from_qualified("game"),
            "fn main(progress: records::Progress) { return progress.count; }",
        ));
        graph.resolve_imports();
        assert_eq!(graph.diagnostics(), &[]);
        let facts = AnalysisFacts::from_module_graph_and_schema(&graph, &schema);
        let count = graph.bodies().flat_map(|body| body.expressions.values())
            .find(|expression| matches!(&expression.kind, HirExprKind::Field(field) if field.name == "count"))
            .expect("valid enum metadata and count field");
        assert_eq!(facts.expression(count.id), None);
        assert_eq!(
            facts.member_target(count.id),
            Some(&MemberTargetFact::Unresolved)
        );
    }
}
