use vela_bytecode::Linker;
use vela_common::SourceId;
use vela_def::{script_field_id, script_function_id, script_type_id, script_variant_id};
use vela_hir::module_graph::{ModulePath, ModuleSource};
use vela_reflect::registry::TypeRegistry;

use crate::engine::Engine;

#[test]
fn linked_and_reflected_script_schema_ids_share_canonical_identity() {
    let engine = Engine::builder().build().expect("engine");
    let program = engine
        .compile_module_sources(&[ModuleSource::new(
            SourceId::new(81),
            ModulePath::from_qualified("game::reward"),
            r#"
#[id(101)]
struct Reward {
    #[id(102)]
    count: i64,
}

enum Outcome {
    #[id(201)]
    Granted {
        #[id(202)]
        value: i64,
    },
}

fn main() {
    let reward = Reward { count: 1 };
    return Outcome::Granted { value: reward.count };
}
"#,
        )])
        .expect("script schemas should compile");
    let linked = Linker::new()
        .link_test_program(&program)
        .expect("script schemas should link");
    let graph = program.script_metadata().expect("script metadata graph");
    let mut reflected = TypeRegistry::new();
    reflected.register_script_types(graph);
    reflected.register_script_modules(graph);

    let main_id = script_function_id("game::reward::main");
    let reflected_main = reflected
        .function_by_name("game::reward::main")
        .expect("reflected main");
    let program_main = program
        .function_by_id(main_id)
        .expect("program main by canonical identity");
    let linked_main = linked
        .entry_point_by_name("game::reward::main")
        .and_then(|handle| linked.function(handle))
        .expect("linked main by canonical name");

    assert_eq!(reflected_main.id, main_id);
    assert_eq!(program_main.name, "game::reward::main");
    assert_eq!(linked.debug_name(linked_main.debug_name), program_main.name);

    let reward = reflected
        .type_by_name("game::reward::Reward")
        .expect("reflected Reward");
    let outcome = reflected
        .type_by_name("game::reward::Outcome")
        .expect("reflected Outcome");
    let granted = outcome
        .variants
        .iter()
        .find(|variant| variant.name == "Granted")
        .expect("reflected Granted");
    let linked_reward = linked
        .types()
        .find_map(|(_, ty)| {
            (linked.debug_name(ty.debug_name) == "game::reward::Reward").then_some(ty)
        })
        .expect("linked Reward");
    let linked_outcome = linked
        .types()
        .find_map(|(_, ty)| {
            (linked.debug_name(ty.debug_name) == "game::reward::Outcome").then_some(ty)
        })
        .expect("linked Outcome");
    let linked_granted = linked
        .variants()
        .find_map(|(_, variant)| {
            (linked.debug_name(variant.debug_name) == "game::reward::Outcome::Granted")
                .then_some(variant)
        })
        .expect("linked Granted");

    assert_eq!(
        reward.key.id,
        script_type_id("game::reward::Reward", Some(101))
    );
    assert_eq!(
        outcome.key.id,
        script_type_id("game::reward::Outcome", None)
    );
    assert_eq!(linked_reward.id, reward.key.id);
    assert_eq!(linked_outcome.id, outcome.key.id);
    assert_eq!(
        reward.fields[0].id,
        script_field_id("game::reward::Reward", None, "count", Some(102))
    );
    assert_eq!(
        granted.id,
        script_variant_id("game::reward::Outcome", "Granted", Some(201))
    );
    assert_eq!(linked_granted.id, granted.id);
    assert_eq!(
        granted.fields[0].id,
        script_field_id("game::reward::Outcome", Some("Granted"), "value", Some(202),)
    );
}
