use std::collections::{BTreeMap, BTreeSet};

use vela_common::SourceId;
use vela_hir::{
    body::{HirBody, HirBodyOwner},
    module_graph::{ModuleGraph, ModuleSource},
};
use vela_package::{ModulePath, PackageId};

use super::{AnalysisFacts, ExecutableReceiverSeed};
use crate::{registry::RegistryFacts, type_fact::TypeFact};

fn fixture() -> (ModuleGraph, RegistryFacts) {
    let mut graph = ModuleGraph::new();
    for (source, module, text) in [
        (60, "shapes", "pub struct Row { value: i64 }"),
        (
            61,
            "game",
            r#"
use shapes::Row as Alias;
struct Row { value: String }
trait Reader {
 fn required(self) -> i64;
 fn default_read(self) -> i64 {
  let out = self.required();
  let capture = || self.required();
  return out;
 }
}
impl Alias {
 fn read(self) -> i64 {
  let direct = self.value;
  let capture = || self.value;
  return direct;
 }
 fn erased(self: Any) { return self.value; }
}
impl Reader for Alias {
 fn required(self) -> i64 { return self.value; }
}
impl host::Row {
 fn host_read(self) -> i64 { return self.score; }
}
fn plain(value: Any) { return value.value; }
"#,
        ),
    ] {
        graph.add_source(ModuleSource::new(
            SourceId::new(source),
            PackageId::anonymous(),
            ModulePath::from_qualified(module),
            text,
        ));
    }
    graph.resolve_imports();
    // The source graph deliberately has no registry. Its host impl stays
    // unresolved until schema-backed analysis; every source owner resolves.
    let diagnostics = graph.diagnostics();
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code.as_deref(), Some("hir::unknown_schema"));
    assert!(diagnostics[0].message.contains("host::Row"));
    let mut schema = RegistryFacts::default();
    schema.insert_type("shapes::Row", TypeFact::host("registry_decoy"));
    schema.insert_field("registry_decoy", "value", TypeFact::STRING);
    schema.insert_type("host::Row", TypeFact::host("host::Row"));
    schema.insert_field("host::Row", "score", TypeFact::I64);
    (graph, schema)
}

fn method_name<'a>(graph: &'a ModuleGraph, body: &HirBody) -> Option<&'a str> {
    let bindings = graph.bindings_for_body(body.id)?;
    match body.owner {
        HirBodyOwner::ImplMethod(node) => graph
            .impl_metadata(bindings.declaration)?
            .methods
            .iter()
            .find(|method| method.node == node)
            .map(|method| method.name.as_str()),
        HirBodyOwner::TraitDefaultMethod(node) => graph
            .trait_shape(bindings.declaration)?
            .methods
            .iter()
            .find(|method| method.default_body_node == Some(node))
            .map(|method| method.name.as_str()),
        _ => None,
    }
}

#[test]
fn whole_module_implicit_receivers_preserve_scoped_source_and_schema_owners() {
    let (graph, schema) = fixture();
    let facts = AnalysisFacts::from_module_graph_and_schema(&graph, &schema);
    let missing = AnalysisFacts::from_module_graph(&graph);
    let mut methods = 0;
    let mut captured_fields = 0;
    for body in graph.bodies() {
        if let Some(name) = method_name(&graph, body) {
            methods += 1;
            let local = body.self_binding.expect("self binding");
            let expected = match name {
                "read" | "required" => TypeFact::record("shapes::Row"),
                "default_read" => TypeFact::trait_type("game::Reader"),
                "host_read" => TypeFact::host("host::Row"),
                "erased" => TypeFact::Any,
                other => panic!("unexpected method {other}"),
            };
            assert_eq!(facts.local(local), Some(&expected), "{name}");
            if name == "host_read" {
                assert_eq!(missing.local(local), Some(&TypeFact::Unknown));
            } else {
                assert_eq!(
                    missing.local(local),
                    Some(&expected),
                    "source without schema {name}"
                );
            }
        }
        for (expression, field) in body.fields() {
            let field_name = field.name.as_str();
            let owner = graph
                .body_and_ancestors(body.id)
                .find_map(|ancestor| method_name(&graph, ancestor));
            let expected = match owner {
                Some("read" | "required") if field_name == "value" => TypeFact::I64,
                Some("host_read") if field_name == "score" => TypeFact::I64,
                Some("erased") | None if field_name == "value" => TypeFact::Any,
                _ => continue,
            };
            assert_eq!(
                facts.expression(expression),
                Some(&expected),
                "{owner:?}.{field_name}"
            );
            if matches!(body.owner, HirBodyOwner::Lambda { .. }) {
                captured_fields += 1;
            }
        }
        if graph
            .body_and_ancestors(body.id)
            .any(|ancestor| method_name(&graph, ancestor) == Some("default_read"))
        {
            for (expression, _) in body.calls() {
                assert_eq!(facts.expression(expression), Some(&TypeFact::I64));
            }
        }
    }
    assert_eq!(methods, 5);
    assert_eq!(captured_fields, 1);
}

#[test]
fn implicit_receivers_keep_external_facts_and_executable_seed_boundaries() {
    let (graph, schema) = fixture();
    let body = graph
        .bodies()
        .find(|body| method_name(&graph, body) == Some("read"))
        .expect("read body");
    let local = body.self_binding.expect("self");
    let external = AnalysisFacts::from_module_graph_and_schema_with_local_facts(
        &graph,
        &schema,
        [(local, TypeFact::Any)],
    );
    assert_eq!(external.local(local), Some(&TypeFact::Any));
    for (expression, _) in body.fields() {
        assert_eq!(external.expression(expression), Some(&TypeFact::Any));
    }
    let bodies = graph
        .bodies()
        .filter(|candidate| {
            graph
                .body_and_ancestors(candidate.id)
                .any(|ancestor| ancestor.id == body.id)
        })
        .map(|body| body.id)
        .collect::<BTreeSet<_>>();
    let literals = BTreeMap::new();
    let unseeded =
        AnalysisFacts::from_executable_scope(&graph, Some(&schema), &bodies, None, &literals);
    assert_eq!(
        unseeded.base_local(local),
        None,
        "tooling must not manufacture an executable receiver"
    );
    let receiver = TypeFact::Any;
    let seeded = AnalysisFacts::from_executable_scope(
        &graph,
        Some(&schema),
        &bodies,
        Some(ExecutableReceiverSeed {
            local,
            fact: &receiver,
            script_type: None,
        }),
        &literals,
    );
    assert_eq!(seeded.local(local), Some(&TypeFact::Any));
    for candidate in graph
        .bodies()
        .filter(|candidate| bodies.contains(&candidate.id))
    {
        for (expression, _) in candidate.fields() {
            assert_eq!(seeded.expression(expression), Some(&TypeFact::Any));
        }
    }
}
