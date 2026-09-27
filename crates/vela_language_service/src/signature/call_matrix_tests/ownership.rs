use crate::{LanguageServiceDatabases, QueryContext};
use serde_json::Value;
use vela_hir::{binding::BindingResolution, module_graph::DeclarationKind};

pub(super) fn assert_target(db: &LanguageServiceDatabases, query: &QueryContext<'_>, case: &Value) {
    let graph = db.hir_db().graph();
    if let Some(expected) = case["localKind"].as_str() {
        let callee = query
            .call_argument_facts()
            .expect("call")
            .callee_expression()
            .expect("callee");
        let body = graph
            .bodies()
            .find(|body| body.expressions.contains_key(&callee))
            .expect("callee body");
        let bindings = graph.bindings_for_body(body.id).expect("bindings");
        let Some(BindingResolution::Local(local)) = bindings.resolution(callee) else {
            panic!(
                "{} must resolve to a local, not an unavailable global",
                case["id"]
            );
        };
        let binding = graph
            .body_and_ancestors(body.id)
            .find_map(|body| graph.bindings_for_body(body.id)?.local(*local))
            .expect("local owner including captures");
        assert_eq!(
            format!("{:?}", binding.kind),
            expected,
            "{} lexical owner",
            case["id"]
        );
    }
    if let Some(target) = case["nonCallable"].as_object() {
        let path: Vec<String> =
            serde_json::from_value(target["path"].clone()).expect("authored path");
        let module = graph
            .module_id(query.module_key().expect("current module"))
            .expect("module");
        let expected = target["kind"].as_str().expect("kind");
        let kind = match expected {
            "Const" => DeclarationKind::Const,
            "State" => DeclarationKind::State,
            "Struct" => DeclarationKind::Struct,
            "Trait" => DeclarationKind::Trait,
            "Enum" => DeclarationKind::Enum,
            other => panic!("unreviewed noncallable owner {other}"),
        };
        assert!(
            graph
                .resolve_visible_declaration_path(module, &path, kind)
                .is_some(),
            "{} has an available {expected} declaration",
            case["id"]
        );
    }
}
