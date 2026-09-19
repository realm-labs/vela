use super::*;
use crate::body::HirBodyOwner;

#[test]
fn required_signature_defaults_keep_metadata_indices_after_recovery() {
    use vela_syntax::ast::{AstNode, SyntaxParam, SyntaxTraitMethod};
    for text in [
        "trait Reader { fn (self); fn read(self, value = 1); }",
        "trait Reader { fn read(self, : i64 = 0, value = 1); }",
    ] {
        let mut graph = ModuleGraph::new();
        let module = graph.add_source(source(1, "game", text));
        let declaration = graph
            .module(module)
            .expect("fixture module")
            .get("Reader")
            .expect("Reader declaration");
        let shape = graph.trait_shape(declaration).expect("Reader shape");
        let method = shape
            .methods
            .iter()
            .find(|method| method.name == "read")
            .expect("surviving read method");
        let value = method
            .signature
            .params
            .iter()
            .find(|param| param.name == "value")
            .expect("surviving value parameter");
        let body = graph
            .body(value.default_body.expect("owned surviving default"))
            .expect("surviving default root");
        let span = body.origin.span;
        assert_eq!(&text[span.start as usize..span.end as usize], "1");
        let parsed = vela_syntax::parse::parse_source(text);
        assert!(
            parsed.tree().syntax().descendants().any(|node| {
                SyntaxTraitMethod::cast(node.clone())
                    .is_some_and(|method| method.name_token().is_none())
                    || SyntaxParam::cast(node).is_some_and(|param| param.name_token().is_none())
            }),
            "fixture must retain a nameless recovery node: {text}"
        );
    }
}

#[test]
fn required_signature_defaults_own_canonical_nested_scopes() {
    let text = "trait Reader { fn run(self, value: i64, callback = |value: bool| value); }";
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(1, "game", text));
    let declaration = graph
        .module(module)
        .expect("fixture module")
        .get("Reader")
        .expect("Reader declaration");
    let method = &graph
        .trait_shape(declaration)
        .expect("Reader shape")
        .methods[0];
    assert!(!method.has_default);
    assert!(method.default_body_node.is_none());
    let root = method
        .signature
        .params
        .last()
        .expect("callback parameter")
        .default_body
        .expect("callback default root");
    assert_eq!(
        graph.body(root).expect("default body").owner,
        HirBodyOwner::TraitSignatureDefault(declaration)
    );
    let offset = text.rfind("value").expect("value reference marker") as u32;
    let inner = graph
        .body_containing_offset(SourceId::new(1), offset)
        .expect("innermost body");
    assert!(matches!(inner.owner, HirBodyOwner::Lambda { parent, .. } if parent == root));
    assert_eq!(
        inner.origin.span,
        graph.body(root).expect("default body").origin.span
    );
    assert_eq!(
        graph
            .body_and_ancestors(inner.id)
            .map(|body| body.id)
            .collect::<Vec<_>>(),
        [inner.id, root]
    );
    let bindings = graph
        .bindings_for_body(inner.id)
        .expect("canonical binding map");
    assert_eq!(bindings.body(), root);
    let locals = bindings
        .locals_named("value")
        .iter()
        .map(|id| graph.local_binding(*id).expect("canonical local identity"))
        .collect::<Vec<_>>();
    assert_eq!(locals.len(), 2);
    let outer = locals
        .iter()
        .find(|local| local.kind == LocalBindingKind::Parameter)
        .expect("outer parameter");
    let nested = locals
        .iter()
        .find(|local| local.kind == LocalBindingKind::LambdaParameter)
        .expect("inner lambda parameter");
    assert_eq!(
        outer.type_hint.as_ref().expect("outer type hint").display(),
        "i64"
    );
    assert_eq!(
        nested
            .type_hint
            .as_ref()
            .expect("inner type hint")
            .display(),
        "bool"
    );
    assert!(
        bindings
            .resolutions()
            .any(|(_, resolution)| resolution == &BindingResolution::Local(nested.id))
    );
    assert!(
        !bindings
            .resolutions()
            .any(|(_, resolution)| resolution == &BindingResolution::Local(outer.id))
    );
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
}

#[test]
fn required_signature_defaults_refresh_import_and_qualified_owners() {
    for order in [[0, 1], [1, 0]] {
        let mut graph = ModuleGraph::new();
        let sources = [
            source(
                1,
                "game",
                "use api::value as imported; trait Reader { fn run(self, first = imported, second = api::value); }",
            ),
            source(2, "api", "pub const value: i64 = 1;"),
        ];
        for index in order {
            graph.add_source(sources[index].clone());
        }
        graph.resolve_imports();
        let declaration = graph
            .declarations()
            .find(|decl| decl.name == "Reader")
            .expect("Reader declaration")
            .id;
        let expected = graph
            .declarations()
            .find(|decl| decl.name == "value")
            .expect("value declaration")
            .id;
        let method = &graph
            .trait_shape(declaration)
            .expect("Reader shape")
            .methods[0];
        let roots = method
            .signature
            .params
            .iter()
            .filter_map(|param| param.default_body)
            .collect::<Vec<_>>();
        assert_eq!(roots.len(), 2);
        assert_ne!(roots[0], roots[1]);
        for root in roots {
            let bindings = graph.bindings_for_body(root).expect("default bindings");
            assert!(
                bindings
                    .resolutions()
                    .any(|(_, resolution)| resolution == &BindingResolution::Declaration(expected)),
                "{order:?}: {bindings:?}"
            );
        }
        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    }
}
