use crate::{facts::AnalysisFacts, registry::RegistryFacts, type_fact::TypeFact};
use vela_common::{CollectionViewMutation, SourceId};
use vela_hir::{
    body::HirExprKind,
    module_graph::{ModuleGraph, ModuleSource},
};
use vela_package::{ModulePath, PackageId};

#[test]
fn nested_registered_hints_preserve_structural_facts_through_calls_fields_and_globals() {
    let row = TypeFact::Host {
        name: "host::Row".into(),
    };
    for (hint, expected) in [
        ("Alias", row.clone()),
        ("Array<Alias>", TypeFact::array(row.clone())),
        ("ArrayView<Alias>", TypeFact::array_view(row.clone())),
        (
            "ArrayMut<Alias>",
            TypeFact::array_mut(row.clone(), CollectionViewMutation::Fixed),
        ),
        (
            "Map<String, Alias>",
            TypeFact::map(TypeFact::STRING, row.clone()),
        ),
        (
            "MapView<String, Alias>",
            TypeFact::map_view(TypeFact::STRING, row.clone()),
        ),
        (
            "MapMut<String, Alias>",
            TypeFact::map_mut(
                TypeFact::STRING,
                row.clone(),
                CollectionViewMutation::Growable,
            ),
        ),
        ("Set<Alias>", TypeFact::set(row.clone())),
        ("SetView<Alias>", TypeFact::set_view(row.clone())),
        (
            "SetMut<Alias>",
            TypeFact::set_mut(row.clone(), CollectionViewMutation::Growable),
        ),
        ("Iterator<Alias>", TypeFact::iterator(row.clone())),
        ("Option<Alias>", TypeFact::option(row.clone())),
        (
            "Result<Array<Alias>, Alias>",
            TypeFact::result(TypeFact::array(row.clone()), row.clone()),
        ),
        (
            "(Alias, Array<Alias>)",
            TypeFact::tuple([row.clone(), TypeFact::array(row.clone())]),
        ),
    ] {
        check("use host::Row as Alias;", hint, expected);
    }
    for (imports, expected) in [
        ("use api::Row as Alias;", TypeFact::record("api::Row")),
        ("use api::Hidden as Alias;", TypeFact::Unknown),
        ("use api::Mask as Alias;", TypeFact::Unknown),
        ("use missing::Row as Alias;", TypeFact::Unknown),
        (
            "use host::Row as Alias; use api::Row as Alias;",
            TypeFact::Unknown,
        ),
    ] {
        check(imports, "Array<Alias>", TypeFact::array(expected));
    }
}

fn check(imports: &str, hint: &str, expected: TypeFact) {
    let mut graph = ModuleGraph::new();
    let text = format!("{imports}
        fn accept(value: {hint}) -> {hint} {{ value }}
        trait Provider {{ fn echo(self, value: {hint}) -> {hint} {{ value }} }}
        pub struct Worker {{}}
        #[provider(id = \"worker\")] impl Provider for Worker {{}}
        struct Holder {{ value: {hint} }}
        const CONSTANT: {hint} = [];
        extern state STATE: {hint};
        fn inspect(input: {hint}, worker: Worker, holder: Holder) {{ accept(input); worker.echo(input); holder.value; CONSTANT; STATE; }}");
    let module = graph.add_source(ModuleSource::new(
        SourceId::new(71),
        PackageId::anonymous(),
        ModulePath::from_qualified("main"),
        text,
    ));
    graph.add_source(ModuleSource::new(
        SourceId::new(72),
        PackageId::anonymous(),
        ModulePath::from_qualified("api"),
        "pub struct Row {} struct Hidden {} pub const Mask: i64 = 1;",
    ));
    graph.resolve_imports();
    let providers =
        vela_hir::provider::discover_providers(&graph).expect("valid provider metadata");
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0].provider_type_name, "main::Worker");
    assert_eq!(providers[0].key.provider().as_str(), "worker");
    assert_eq!(
        providers[0]
            .methods
            .iter()
            .map(|method| method.name.as_str())
            .collect::<Vec<_>>(),
        ["echo"]
    );
    let mut schema = RegistryFacts::default();
    for name in [
        "host::Row",
        "api::Row",
        "api::Hidden",
        "api::Mask",
        "Alias",
        "Row",
    ] {
        schema.insert_type(name, TypeFact::Host { name: name.into() });
    }
    let facts = AnalysisFacts::from_module_graph_and_schema(&graph, &schema);
    let declarations = graph.module(module).expect("module");
    for name in ["CONSTANT", "STATE"] {
        assert_eq!(
            facts.declaration(declarations.get(name).expect("declaration")),
            Some(&expected),
            "{imports} {hint} {name}"
        );
    }
    let body = graph
        .function_body(declarations.get("inspect").expect("inspect"))
        .expect("body");
    let mut checked = 0;
    for expression in body.expressions.values() {
        if matches!(expression.kind, HirExprKind::Call(_))
            || matches!(&expression.kind, HirExprKind::Field(field) if field.name == "value")
        {
            assert_eq!(
                facts.expression(expression.id),
                Some(&expected),
                "{imports} {hint}: {:?}",
                expression.kind
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 3);
}
