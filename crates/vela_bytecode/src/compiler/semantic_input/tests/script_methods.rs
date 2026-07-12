use vela_common::SourceId;
use vela_def::{script_function_id, script_inherent_method_id, script_trait_method_id};
use vela_hir::body::HirPatternKind;
use vela_hir::module_graph::{ModulePath, ModuleSource};
use vela_hir::source_ingestion::build_module_source_set;
use vela_mir::{
    CompileFunctionIdentity, CompileMethodClass, CompileParameterDefault,
    CompilePatternConstructorTarget,
};

use super::{FixtureRoots, prepare_source};
use crate::compiler::options::CompilerOptions;
use crate::compiler::semantic::SemanticCompilation;
use crate::compiler::semantic_input::{
    PreparedSemanticInput, SemanticInputRequest, SemanticRoots, prepare_semantic_input,
};

#[test]
fn single_source_catalog_keeps_main_method_identity_and_root_symbol_seed() {
    let fixture = prepare_source(
        r#"
struct Player { level: i64 }
impl Player {
    fn bonus(self, amount: i64 = 2) -> i64 { return self.level + amount; }
}
"#,
        FixtureRoots::Program,
    )
    .expect("single-source method semantic input");
    let targets = fixture.input.targets();
    let owner = targets
        .type_by_name("script::Player")
        .expect("single-source Player descriptor");
    assert_eq!(owner.canonical_name, "script::Player");
    assert_eq!(owner.runtime_name, "Player");
    assert!(targets.type_by_name("Player").is_none());
    let method_id = script_inherent_method_id("main::Player", "bonus");
    let descriptor = targets
        .method_descriptor(owner.id, method_id)
        .expect("single-source method descriptor");
    let CompileMethodClass::Script {
        executable,
        owner_name,
        code_symbol,
    } = &descriptor.class
    else {
        panic!("script method class");
    };

    assert_eq!(owner_name, "Player");
    assert_eq!(code_symbol, "__impl.Player.bonus");
    assert_eq!(
        executable.function,
        script_function_id("__impl.Player.bonus")
    );
    assert_eq!(descriptor.signature.parameters.len(), 1);
    assert!(matches!(
        descriptor.signature.parameters[0].default,
        CompileParameterDefault::HirBody(_)
    ));
}

#[test]
fn module_catalog_keeps_qualified_method_identity_and_source_symbol_seed() {
    let input = prepare_modules(&[ModuleSource::new(
        SourceId::new(911),
        ModulePath::from_qualified("game::combat"),
        r#"
struct Player { level: i64 }
impl Player {
    fn bonus(self) -> i64 { return self.level; }
}
"#,
    )]);
    let targets = input.targets();
    let owner = targets
        .type_by_name("script::game::combat::Player")
        .expect("qualified Player descriptor");
    assert_eq!(owner.canonical_name, "script::game::combat::Player");
    assert_eq!(owner.runtime_name, "game::combat::Player");
    assert!(targets.type_by_name("game::combat::Player").is_none());
    let method_id = script_inherent_method_id("game::combat::Player", "bonus");
    let descriptor = targets
        .method_descriptor(owner.id, method_id)
        .expect("qualified method descriptor");
    let CompileMethodClass::Script {
        executable,
        owner_name,
        code_symbol,
    } = &descriptor.class
    else {
        panic!("script method class");
    };

    assert_eq!(owner_name, "game::combat::Player");
    assert_eq!(
        code_symbol,
        "game::combat.__impl.game::combat::Player.bonus"
    );
    assert_eq!(executable.function, script_function_id(code_symbol));
}

#[test]
fn qualified_record_patterns_are_placed_as_explicit_never_matches() {
    let sources = [
        ModuleSource::new(
            SourceId::new(912),
            ModulePath::from_qualified("game::main"),
            r#"
fn main() {
    let reward = game::reward::Reward { amount: 7 };
    return match reward { game::reward::Reward { amount } => amount, _ => 0 };
}
"#,
        ),
        ModuleSource::new(
            SourceId::new(913),
            ModulePath::from_qualified("game::reward"),
            "pub struct Reward { amount: i64 }",
        ),
    ];
    let built = build_module_source_set(&sources).expect("module semantic graph");
    let semantic = SemanticCompilation::new(&built).expect("semantic compilation");
    let (body, pattern) = semantic
        .graph()
        .bodies()
        .find_map(|body| {
            body.patterns.values().find_map(|pattern| {
                matches!(pattern.kind, HirPatternKind::RecordVariant { .. })
                    .then_some((body.id, pattern.id))
            })
        })
        .expect("qualified record pattern");
    let script_function_symbols = semantic.function_symbols();
    let type_symbols = semantic.type_symbols();
    let global_symbols = semantic.global_symbols();
    let evaluated_constants = semantic.evaluated_constants().expect("module constants");
    let schema_defaults = semantic
        .schema_defaults(&type_symbols, &evaluated_constants)
        .expect("module schema defaults");
    let options = CompilerOptions::default();
    let input = prepare_semantic_input(SemanticInputRequest {
        graph: semantic.graph(),
        roots: SemanticRoots::Program,
        script_function_symbols: &script_function_symbols,
        script_methods: semantic.script_method_catalog(),
        type_symbols: &type_symbols,
        global_symbols: &global_symbols,
        evaluated_constants: &evaluated_constants,
        schema_defaults: &schema_defaults,
        options: &options,
        registry: None,
    })
    .expect("qualified record semantic input");
    let targets = input.targets();
    let function = targets
        .compilation_roots()
        .find_map(|(function, root)| (root.body == body).then_some(function))
        .expect("owning function root");
    let target = targets
        .function_targets(function)
        .and_then(|targets| targets.pattern_constructor(pattern))
        .expect("qualified record pattern target");
    let CompilePatternConstructorTarget::NeverMatchesRecord { type_id, fields } = target else {
        panic!("expected explicit never-match record placement, got {target:?}")
    };
    let descriptor = targets
        .type_descriptor(*type_id)
        .expect("record type descriptor");
    assert_eq!(descriptor.runtime_name, "game::reward::Reward");
    assert_eq!(fields.len(), 1);
    assert_eq!(
        targets
            .field_descriptor(fields[0])
            .expect("record field descriptor")
            .name,
        "amount"
    );
}

#[test]
fn shared_trait_default_catalog_specializes_owners_without_changing_body_or_method_id() {
    let fixture = prepare_source(
        r#"
trait BonusSource { fn bonus(self) -> i64 { return self.value; } }
struct Player { value: i64 }
struct Monster { value: i64 }
impl BonusSource for Player {}
impl BonusSource for Monster {}
"#,
        FixtureRoots::Program,
    )
    .expect("shared trait-default semantic input");
    let targets = fixture.input.targets();
    let method_id = script_trait_method_id("main::BonusSource", "bonus");
    let roots = targets
        .compilation_roots()
        .filter_map(|(_, root)| match root.identity {
            CompileFunctionIdentity::Method(target) if target.method == method_id => {
                Some((target.owner, target.function, root.body))
            }
            CompileFunctionIdentity::Function(_) | CompileFunctionIdentity::Method(_) => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(roots.len(), 2);
    assert_ne!(roots[0].0, roots[1].0);
    assert_ne!(roots[0].1, roots[1].1);
    assert_eq!(roots[0].2, roots[1].2);
    assert_eq!(
        roots
            .iter()
            .map(|(_, function, _)| *function)
            .collect::<std::collections::BTreeSet<_>>(),
        [
            script_function_id("__impl.BonusSource.for.Player.bonus"),
            script_function_id("__impl.BonusSource.for.Monster.bonus"),
        ]
        .into_iter()
        .collect()
    );
}

fn prepare_modules(sources: &[ModuleSource]) -> PreparedSemanticInput {
    let built = build_module_source_set(sources).expect("module semantic graph");
    let semantic = SemanticCompilation::new(&built).expect("semantic compilation");
    let script_function_symbols = semantic.function_symbols();
    let type_symbols = semantic.type_symbols();
    let global_symbols = semantic.global_symbols();
    let evaluated_constants = semantic.evaluated_constants().expect("module constants");
    let schema_defaults = semantic
        .schema_defaults(&type_symbols, &evaluated_constants)
        .expect("module schema defaults");
    let options = CompilerOptions::default();
    prepare_semantic_input(SemanticInputRequest {
        graph: semantic.graph(),
        roots: SemanticRoots::Program,
        script_function_symbols: &script_function_symbols,
        script_methods: semantic.script_method_catalog(),
        type_symbols: &type_symbols,
        global_symbols: &global_symbols,
        evaluated_constants: &evaluated_constants,
        schema_defaults: &schema_defaults,
        options: &options,
        registry: None,
    })
    .expect("module semantic input")
}
