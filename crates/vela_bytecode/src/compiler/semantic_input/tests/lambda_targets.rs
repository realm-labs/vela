use vela_common::PrimitiveTag;
use vela_def::{DefPath, TypeId};
use vela_mir::{CompileTypeClass, MirTypeContract};
use vela_registry::{DefinitionRegistry, TypeDef, TypeKindDef};

use super::{FixtureRoots, prepare_source, prepare_source_with_registry};

#[test]
fn selected_root_owns_nested_script_and_primitive_lambda_contracts() {
    let fixture = prepare_source(
        r#"
struct Reward { amount: i64 }

fn selected() {
    let outer = |reward: Reward, count: i64| {
        return |flag: bool| reward;
    };
    return outer;
}

fn ignored() { return |text: String| text; }
"#,
        FixtureRoots::Function("selected"),
    )
    .expect("typed nested lambda targets should close for the selected root");
    let targets = fixture.input.targets();
    let function = targets
        .function_for_declaration(fixture.declarations["selected"])
        .expect("selected function identity");
    let root = targets.function(function).expect("selected root");
    let scoped = targets.function_targets(function).expect("scoped targets");
    let lambdas = scoped.lambdas().collect::<Vec<_>>();

    assert_eq!(
        lambdas.len(),
        2,
        "the ignored root must not leak its lambda"
    );
    let outer = lambdas
        .iter()
        .copied()
        .find(|lambda| lambda.parameters.iter().any(|param| param.name == "reward"))
        .expect("outer lambda target");
    let inner = lambdas
        .iter()
        .copied()
        .find(|lambda| lambda.parameters.iter().any(|param| param.name == "flag"))
        .expect("inner lambda target");
    assert_eq!(outer.parent, root.body);
    assert_eq!(inner.parent, outer.body);
    assert!(inner.code_symbol.starts_with(&outer.code_symbol));

    let reward = scoped
        .type_by_name("script::Reward")
        .expect("script type descriptor");
    assert_eq!(
        scoped.type_for_declaration(fixture.declarations["Reward"]),
        Some(reward.id)
    );
    let reward_param = outer
        .parameters
        .iter()
        .find(|parameter| parameter.name == "reward")
        .expect("reward parameter");
    assert_eq!(
        scoped.lambda_parameter(outer.body, reward_param.parameter),
        Some(reward_param)
    );
    assert_eq!(
        reward_param.contract,
        Some(MirTypeContract::Shape {
            type_id: reward.id,
            shape: reward.shape.expect("script record shape"),
        })
    );
    assert_eq!(
        outer.parameters[1].contract,
        Some(MirTypeContract::Primitive(PrimitiveTag::I64))
    );
    assert_eq!(
        inner.parameters[0].contract,
        Some(MirTypeContract::Primitive(PrimitiveTag::Bool))
    );
}

#[test]
fn registry_lambda_contract_closes_descriptor_without_a_stable_lambda_function() {
    let mut registry = DefinitionRegistry::new();
    let payload = TypeId::new(9_301);
    registry
        .register_type(
            TypeDef::new(DefPath::ty("host", ["game"], "Payload"))
                .with_id(payload)
                .kind(TypeKindDef::ScriptStruct),
        )
        .expect("registry payload type");
    let fixture = prepare_source_with_registry(
        "fn main() { return |payload: game::Payload| payload; }",
        FixtureRoots::Program,
        registry.compile_view(),
    )
    .expect("registry lambda contract should close its descriptor edge");
    let targets = fixture.input.targets();
    let (function, root) = targets.compilation_roots().next().expect("main root");
    let scoped = targets.function_targets(function).expect("main targets");
    let lambda = scoped.lambdas().next().expect("typed lambda target");

    assert_eq!(lambda.parent, root.body);
    assert_eq!(lambda.parameters.len(), 1);
    assert_eq!(
        lambda.parameters[0].contract,
        Some(MirTypeContract::Definition(payload))
    );
    assert_eq!(
        scoped
            .type_descriptor(payload)
            .map(|descriptor| descriptor.class),
        Some(CompileTypeClass::Registry)
    );
    assert!(targets.functions_for_body(lambda.body).is_empty());
    assert!(
        targets
            .function_descriptor(function)
            .expect("root descriptor")
            .signature
            .parameters
            .is_empty(),
        "lambda contracts must not be duplicated into the root signature"
    );
}

#[test]
fn lambda_in_parameter_default_targets_the_owning_runtime_prologue() {
    let fixture = prepare_source(
        "fn main(callback = (|value: u64| value)) { return callback; }",
        FixtureRoots::Program,
    )
    .expect("a lambda in a parameter default should remain a nested runtime function");
    let targets = fixture.input.targets();
    let (function, root) = targets.compilation_roots().next().expect("main root");
    let lambda = targets
        .function_targets(function)
        .expect("main targets")
        .lambdas()
        .next()
        .expect("default lambda target");

    assert_eq!(lambda.parent, root.body);
    assert_eq!(
        lambda.parameters[0].contract,
        Some(MirTypeContract::Primitive(PrimitiveTag::U64))
    );
}

#[test]
fn shared_trait_lambda_body_is_scoped_to_each_method_root() {
    let fixture = prepare_source(
        r#"
trait Mapper {
    fn mapper(self) { return |value: i32| value; }
}
struct Left {}
struct Right {}
impl Mapper for Left {}
impl Mapper for Right {}
"#,
        FixtureRoots::Program,
    )
    .expect("shared trait-default lambda targets should remain root-scoped");
    let targets = fixture.input.targets();
    let roots = targets.compilation_roots().collect::<Vec<_>>();
    assert_eq!(roots.len(), 2);

    let lambdas = roots
        .iter()
        .map(|(function, _)| {
            targets
                .function_targets(*function)
                .expect("method targets")
                .lambdas()
                .next()
                .expect("shared lambda target")
        })
        .collect::<Vec<_>>();
    assert_eq!(lambdas[0].body, lambdas[1].body);
    assert_ne!(lambdas[0].code_symbol, lambdas[1].code_symbol);
    assert_eq!(
        lambdas[0].parameters[0].contract,
        Some(MirTypeContract::Primitive(PrimitiveTag::I32))
    );
    assert!(targets.functions_for_body(lambdas[0].body).is_empty());
}
