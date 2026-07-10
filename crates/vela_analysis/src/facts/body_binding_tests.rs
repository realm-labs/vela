use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingResolution, LocalBindingKind};
use vela_hir::body::HirBodyOwner;
use vela_hir::module_graph::{ModuleGraph, ModulePath, ModuleSource};

use super::AnalysisFacts;
use crate::type_fact::TypeFact;

#[test]
fn analysis_ingests_impl_trait_lambda_and_default_binding_generations() {
    let source = SourceId::new(44);
    let text = r#"
trait Score {
    fn score(self, bonus: i64 = 1) -> i64 {
        return (|value: i64| value + bonus)(self.count);
    }
}

struct Reward { count: i64 }
impl Score for Reward {}

impl Reward {
    fn doubled(self, factor: i64 = 2) -> i64 {
        return (|value: i64| value * factor)(self.count);
    }
}
"#;
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        source,
        ModulePath::from_qualified("game"),
        text,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);
    let facts = AnalysisFacts::from_module_graph(&graph);

    let mut saw_impl = false;
    let mut saw_trait = false;
    let mut saw_lambda = false;
    let mut saw_default = false;
    for body in graph.bodies() {
        saw_impl |= matches!(body.owner, HirBodyOwner::ImplMethod(_));
        saw_trait |= matches!(body.owner, HirBodyOwner::TraitDefaultMethod(_));
        saw_lambda |= matches!(body.owner, HirBodyOwner::Lambda { .. });
        saw_default |= matches!(body.owner, HirBodyOwner::ParameterDefault { .. });

        let Some(bindings) = graph.bindings_for_body(body.id) else {
            continue;
        };
        for local in bindings.locals().filter(|local| {
            matches!(
                local.kind,
                LocalBindingKind::Parameter | LocalBindingKind::LambdaParameter
            ) && local.type_hint.is_some()
        }) {
            assert_eq!(facts.local(local.id), Some(&TypeFact::I64));
        }
        for (expression, resolution) in bindings.resolutions() {
            if let BindingResolution::Local(local) = resolution
                && facts.local(*local) == Some(&TypeFact::I64)
            {
                assert_eq!(facts.expression(expression), Some(&TypeFact::I64));
            }
        }
    }

    assert!(saw_impl && saw_trait && saw_lambda && saw_default);
    let factor_use = text.rfind("factor").expect("factor use");
    let expression = graph
        .expression_at_span(Span::new(
            source,
            factor_use as u32,
            (factor_use + "factor".len()) as u32,
        ))
        .expect("factor expression");
    assert_eq!(facts.expression(expression), Some(&TypeFact::I64));
}
