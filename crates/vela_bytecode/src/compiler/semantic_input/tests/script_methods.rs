use vela_common::SourceId;
use vela_def::{script_function_id, script_inherent_method_id, script_trait_method_id};
use vela_hir::module_graph::{ModulePath, ModuleSource};
use vela_mir::{CompileFunctionIdentity, CompileMethodClass, CompileParameterDefault};

use super::{FixtureRoots, prepare_source};
use crate::compiler::options::CompilerOptions;
use crate::compiler::semantic::parse_semantic_modules;
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
    let semantic = parse_semantic_modules(sources).expect("module semantic graph");
    let script_function_symbols = semantic.script_function_symbols();
    let type_symbols = semantic.type_symbols();
    let global_symbols = semantic.global_symbols();
    let constants = semantic.const_values().expect("module constants");
    let schema_defaults = semantic
        .schema_defaults(&type_symbols, &constants)
        .expect("module schema defaults");
    let options = CompilerOptions::default();
    prepare_semantic_input(SemanticInputRequest {
        graph: semantic.script_metadata_graph(),
        roots: SemanticRoots::Program,
        script_function_symbols: &script_function_symbols,
        script_methods: semantic.script_method_catalog(),
        type_symbols: &type_symbols,
        global_symbols: &global_symbols,
        constants: &constants,
        schema_defaults: &schema_defaults,
        options: &options,
        registry: None,
    })
    .expect("module semantic input")
}
