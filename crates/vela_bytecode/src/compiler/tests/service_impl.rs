use super::*;

#[test]
fn compiler_emits_service_impl_methods_as_hidden_stable_functions() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
#[service_impl(game::inventory::InventoryService)]
impl InventoryHotfix {
    fn grant(value: i64) -> i64 {
        return value * 3;
    }
}
"#,
    )
    .expect("service method should compile");
    let symbol = "__service_impl.game.inventory.InventoryService.grant";
    let function =
        vela_def::script_function_id(vela_package::PackageId::anonymous().as_str(), symbol);
    let code = program
        .function_by_id(function)
        .expect("stable hidden service function");

    assert_eq!(code.name, symbol);
    assert_eq!(code.params, ["value"]);
    assert!(
        program.function("grant").is_none(),
        "service method must not become a public function"
    );
}
