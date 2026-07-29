use quote::quote;

use super::expand_result;

#[test]
fn service_domain_generates_one_whole_generation_controller() {
    let output = expand_result(
        quote! { context = RequestContext },
        quote! {
            pub struct GameLogic {
                pub reward: Service<dyn RewardService>,
                pub inventory: Service<dyn InventoryService>,
            }
        },
    )
    .expect("service domain should expand")
    .to_string();

    assert!(output.contains("__GameLogicGeneration"));
    assert!(output.contains("ServiceController < __GameLogicGeneration >"));
    assert!(output.contains("GameLogicBuilder"));
    assert!(output.contains("GameLogicApp"));
    assert!(output.contains("GameLogicPatches"));
    assert!(output.contains("PatchRevision"));
    assert!(output.contains("ServicePatchState"));
    assert!(output.contains("pub fn apply"));
    assert!(output.contains("stage_snapshot"));
    assert!(output.contains("stage_delta"));
    assert!(!output.contains("stage_snapshot_source"));
    assert!(output.contains("__vela_compose_service_RewardService"));
    assert!(output.contains("register_service_set_schema"));
    assert_eq!(output.matches("ServiceController <").count(), 1);
    assert!(output.contains("MissingDefault"));
    assert!(!output.contains("RustRewardService"));
    assert!(!output.contains("HostRef"));
    assert!(!output.contains("runtime : :: vela_engine :: runtime :: Runtime"));
}

#[test]
fn service_domain_requires_marker_fields() {
    let error = expand_result(
        quote! { context = RequestContext },
        quote! {
            pub struct GameLogic {
                pub reward: dyn RewardService,
            }
        },
    )
    .expect_err("bare trait field must fail");

    assert!(error.to_string().contains("Service<dyn ServiceTrait>"));
}
