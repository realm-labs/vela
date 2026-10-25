use crate::{facts::AnalysisFacts, registry::RegistryFacts, type_fact::TypeFact};
use vela_common::SourceId;
use vela_hir::{
    body::HirExprKind,
    module_graph::{ModuleGraph, ModuleSource},
    script_methods::{ScriptMethodCatalog, ScriptMethodCatalogMode},
};
use vela_package::{ModulePath, PackageId};

#[test]
fn nested_registered_hints_preserve_structural_facts_through_calls_fields_and_globals() {
    let row = TypeFact::Record {
        name: "host::Row".into(),
    };
    for (hint, expected) in [
        ("Range", TypeFact::Range),
        ("Array<Range>", TypeFact::array(TypeFact::Range)),
        ("Alias", row.clone()),
        ("Array<Alias>", TypeFact::array(row.clone())),
        (
            "Map<String, Alias>",
            TypeFact::map(TypeFact::STRING, row.clone()),
        ),
        ("Set<Alias>", TypeFact::set(row.clone())),
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
        trait Echo {{ fn echo(self, value: {hint}) -> {hint} {{ value }} }}
        pub struct Worker {{}}
        impl Echo for Worker {{}}
        struct Holder {{ value: {hint} }}
        const CONSTANT: {hint} = [];
        state STATE: {hint} = [];
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
    let methods = ScriptMethodCatalog::from_graph(&graph, ScriptMethodCatalogMode::ModuleGraph)
        .expect("valid ordinary trait implementation");
    assert_eq!(methods.len(), 1);
    assert_eq!(
        methods
            .methods()
            .map(|method| method.name())
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
        schema.insert_type(name, TypeFact::Record { name: name.into() });
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
