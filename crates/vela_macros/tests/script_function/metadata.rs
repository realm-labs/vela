use super::*;

#[test]
fn script_function_generates_native_function_metadata() {
    assert_eq!(
        vela_native_function_desc_grant_bonus(),
        NativeFunctionDesc::new("game::grant_bonus", function_id("game::grant_bonus"))
            .param("amount", TypeHint::Int)
            .param("multiplier", TypeHint::Int)
            .returns(TypeHint::Int)
            .effects(EffectSet::pure())
            .access(
                FunctionAccess::public()
                    .reflect_callable(true)
                    .require_permission("bonus.read"),
            )
            .attr("domain", "gameplay")
            .attr("stable", "true")
            .docs("Grants a copied bonus amount."),
    );
}

#[test]
fn script_function_alias_preserves_native_function_id_across_renames() {
    assert_eq!(
        vela_native_function_desc_grant_bonus_v2(),
        NativeFunctionDesc::new("game::grant_bonus_v2", function_id("game::grant_bonus"))
            .param("amount", TypeHint::Int)
            .returns(TypeHint::Int)
            .effects(EffectSet::pure())
            .access(FunctionAccess::public().reflect_callable(true))
            .docs("Grants a renamed copied bonus amount."),
    );
}

#[test]
fn script_function_generates_set_signature_metadata() {
    assert_eq!(
        vela_native_function_desc_count_labels(),
        NativeFunctionDesc::new("game::count_labels", function_id("game::count_labels"))
            .param("labels", TypeHint::Set)
            .returns(TypeHint::Int)
            .effects(EffectSet::pure())
            .access(FunctionAccess::public().reflect_callable(true))
            .docs("Counts copied unique labels from a script set::"),
    );
}

#[test]
fn script_function_generates_hash_set_signature_metadata() {
    assert_eq!(
        vela_native_function_desc_count_unordered_labels(),
        NativeFunctionDesc::new(
            "game::count_unordered_labels",
            function_id("game::count_unordered_labels"),
        )
        .param("labels", TypeHint::Set)
        .returns(TypeHint::Int)
        .effects(EffectSet::pure())
        .access(FunctionAccess::public().reflect_callable(true))
        .docs("Counts copied unordered labels from a script set::"),
    );
}

#[test]
fn script_function_generates_fixed_array_signature_metadata() {
    assert_eq!(
        vela_native_function_desc_sum_weights(),
        NativeFunctionDesc::new("game::sum_weights", function_id("game::sum_weights"))
            .param("weights", TypeHint::Array)
            .returns(TypeHint::Int)
            .effects(EffectSet::pure())
            .access(FunctionAccess::public().reflect_callable(true))
            .docs("Sums a copied fixed weight array."),
    );
    assert_eq!(
        vela_native_function_desc_default_weights(),
        NativeFunctionDesc::new(
            "game::default_weights",
            function_id("game::default_weights")
        )
        .returns(TypeHint::Array)
        .effects(EffectSet::pure())
        .access(FunctionAccess::public().reflect_callable(true))
        .docs("Returns a copied fixed weight array."),
    );
}

#[test]
fn script_function_generates_hash_map_signature_metadata() {
    assert_eq!(
        vela_native_function_desc_score_total(),
        NativeFunctionDesc::new("game::score_total", function_id("game::score_total"))
            .param("scores", TypeHint::Map)
            .returns(TypeHint::Int)
            .effects(EffectSet::pure())
            .access(FunctionAccess::public().reflect_callable(true))
            .docs("Sums copied score values from a script map."),
    );
}

#[test]
fn script_function_generates_btree_map_signature_metadata() {
    assert_eq!(
        vela_native_function_desc_ordered_score_summary(),
        NativeFunctionDesc::new(
            "game::ordered_score_summary",
            function_id("game::ordered_score_summary"),
        )
        .param("scores", TypeHint::Map)
        .returns(TypeHint::Map)
        .effects(EffectSet::pure())
        .access(FunctionAccess::public().reflect_callable(true))
        .docs("Adds a copied total entry to an ordered script map."),
    );
}

#[test]
fn script_function_generates_f32_signature_metadata() {
    assert_eq!(
        vela_native_function_desc_scale_weight(),
        NativeFunctionDesc::new("game::scale_weight", function_id("game::scale_weight"))
            .param("weight", TypeHint::Float)
            .returns(TypeHint::Float)
            .effects(EffectSet::pure())
            .access(FunctionAccess::public().reflect_callable(true))
            .docs("Scales a copied encounter weight."),
    );
}

#[test]
fn script_function_generates_option_signature_metadata() {
    assert_eq!(
        vela_native_function_desc_optional_bonus(),
        NativeFunctionDesc::new("game::optional_bonus", function_id("game::optional_bonus"))
            .param("bonus", TypeHint::Int)
            .returns(TypeHint::Int)
            .effects(EffectSet::pure())
            .access(FunctionAccess::public().reflect_callable(true))
            .docs("Applies an optional copied bonus."),
    );
}

#[test]
fn script_function_generates_five_arg_signature_metadata() {
    assert_eq!(
        vela_native_function_desc_sum5(),
        NativeFunctionDesc::new("game::sum5", function_id("game::sum5"))
            .param("a", TypeHint::Int)
            .param("b", TypeHint::Int)
            .param("c", TypeHint::Int)
            .param("d", TypeHint::Int)
            .param("e", TypeHint::Int)
            .returns(TypeHint::Int)
            .effects(EffectSet::pure())
            .access(FunctionAccess::public().reflect_callable(true))
            .docs("Sums five copied script integers."),
    );
}

#[test]
fn script_function_generates_result_signature_metadata() {
    assert_eq!(
        vela_native_function_desc_checked_bonus(),
        NativeFunctionDesc::new("game::checked_bonus", function_id("game::checked_bonus"))
            .param("ok", TypeHint::Bool)
            .returns(TypeHint::Any)
            .effects(EffectSet::pure())
            .access(FunctionAccess::public().reflect_callable(true))
            .docs("Returns a dynamic copied Result bonus."),
    );
}

#[test]
fn script_function_generates_host_result_signature_metadata() {
    assert_eq!(
        vela_native_function_desc_checked_host_bonus(),
        NativeFunctionDesc::new(
            "game::checked_host_bonus",
            function_id("game::checked_host_bonus"),
        )
        .param("ok", TypeHint::Bool)
        .returns(TypeHint::Int)
        .effects(EffectSet::pure())
        .access(FunctionAccess::public().reflect_callable(true))
        .docs("Returns a fallible copied host bonus."),
    );
}

#[test]
fn script_function_generates_path_proxy_signature_metadata() {
    assert_eq!(
        vela_native_function_desc_path_depth(),
        NativeFunctionDesc::new("game::path_depth", function_id("game::path_depth"))
            .param("path", TypeHint::PathProxy)
            .returns(TypeHint::Int)
            .effects(EffectSet::pure())
            .access(FunctionAccess::public().reflect_callable(true))
            .docs("Measures a copied host path proxy."),
    );
}

#[test]
fn script_function_generates_private_reflect_visible_metadata() {
    assert_eq!(
        vela_native_function_desc_debug_probe(),
        NativeFunctionDesc::new("game::debug_probe", function_id("game::debug_probe"))
            .returns(TypeHint::Bool)
            .effects(EffectSet::pure())
            .access(FunctionAccess::private().reflect_visible(true))
            .docs("Private reflection-only debug probe."),
    );
}

#[test]
fn script_context_function_generates_native_function_metadata() {
    assert_eq!(
        vela_native_function_desc_set_level(),
        NativeFunctionDesc::new("game::set_level", function_id("game::set_level"))
            .param("player", TypeHint::Any)
            .param("level", TypeHint::Int)
            .returns(TypeHint::Bool)
            .effects(EffectSet::host_write())
            .access(
                FunctionAccess::public()
                    .reflect_callable(true)
                    .require_permission("player.write"),
            )
            .docs("Sets a copied player level through PatchTx."),
    );
}

#[test]
fn script_context_function_alias_preserves_native_function_id_across_renames() {
    assert_eq!(
        vela_native_function_desc_set_level_v2(),
        NativeFunctionDesc::new("game::set_level_v2", function_id("game::set_level"))
            .param("player", TypeHint::Any)
            .param("level", TypeHint::Int)
            .returns(TypeHint::Int)
            .effects(EffectSet::host_write())
            .access(
                FunctionAccess::public()
                    .reflect_callable(true)
                    .require_permission("player.write"),
            )
            .docs("Sets a renamed copied player level through PatchTx."),
    );
}

#[test]
fn script_context_function_generates_host_result_signature_metadata() {
    assert_eq!(
        vela_native_function_desc_checked_level(),
        NativeFunctionDesc::new("game::checked_level", function_id("game::checked_level"))
            .param("player", TypeHint::Any)
            .param("level", TypeHint::Int)
            .param("ok", TypeHint::Bool)
            .returns(TypeHint::Int)
            .effects(EffectSet::host_write())
            .access(
                FunctionAccess::public()
                    .reflect_callable(true)
                    .require_permission("player.write"),
            )
            .docs("Returns a fallible copied player level through PatchTx."),
    );
}

#[test]
fn script_host_function_generates_native_function_metadata() {
    assert_eq!(
        vela_native_function_desc_set_score(),
        NativeFunctionDesc::new("game::set_score", function_id("game::set_score"))
            .param("player", TypeHint::Any)
            .param("score", TypeHint::Int)
            .returns(TypeHint::Int)
            .effects(EffectSet::host_write())
            .access(
                FunctionAccess::public()
                    .reflect_callable(true)
                    .require_permission("player.write"),
            )
            .docs("Sets a copied player score through host execution."),
    );
}

#[test]
fn script_host_function_alias_preserves_native_function_id_across_renames() {
    assert_eq!(
        vela_native_function_desc_set_score_v2(),
        NativeFunctionDesc::new("game::set_score_v2", function_id("game::set_score"))
            .param("player", TypeHint::Any)
            .param("score", TypeHint::Int)
            .returns(TypeHint::Int)
            .effects(EffectSet::host_write())
            .access(
                FunctionAccess::public()
                    .reflect_callable(true)
                    .require_permission("player.write"),
            )
            .docs("Sets a renamed copied player score through host execution."),
    );
}

#[test]
fn script_host_function_generates_host_result_signature_metadata() {
    assert_eq!(
        vela_native_function_desc_checked_score(),
        NativeFunctionDesc::new("game::checked_score", function_id("game::checked_score"))
            .param("player", TypeHint::Any)
            .param("score", TypeHint::Int)
            .param("ok", TypeHint::Bool)
            .returns(TypeHint::Int)
            .effects(EffectSet::host_write())
            .access(
                FunctionAccess::public()
                    .reflect_callable(true)
                    .require_permission("player.write"),
            )
            .docs("Returns a fallible copied player score through host execution."),
    );
}
