use crate::{DefPath, FieldId, FunctionId, GlobalId, MethodId, TypeId, VariantId, stable_id};

const SCRIPT_PACKAGE: &str = "script";

/// Returns the canonical definition path for a script function symbol.
///
/// `symbol` omits the package and includes any source module qualification,
/// for example `game::combat::grant_reward`.
#[must_use]
pub fn script_function_path(symbol: &str) -> DefPath {
    let (module, name) = split_source_symbol(symbol);
    DefPath::function(SCRIPT_PACKAGE, module, name)
}

/// Returns the stable semantic identity for a script function symbol.
#[must_use]
pub fn script_function_id(symbol: &str) -> FunctionId {
    FunctionId::from_def_id(script_function_path(symbol).id())
}

/// Returns the stable identity for an inherent script method.
///
/// `owner` is the source-qualified script type name selected by the compiler,
/// such as `main::Player` or `game::combat::Player`.
#[must_use]
pub fn script_inherent_method_id(owner: &str, method: &str) -> MethodId {
    MethodId::new(u128::from(stable_id("inherent_method", owner, method)))
}

/// Returns the stable identity shared by every implementation of a script
/// trait method.
///
/// `trait_name` is the source-qualified trait name selected by the compiler,
/// or the builtin comparison-trait name for builtin operator traits.
#[must_use]
pub fn script_trait_method_id(trait_name: &str, method: &str) -> MethodId {
    MethodId::new(u128::from(stable_id("trait_method", trait_name, method)))
}

/// Returns the canonical definition path for a script type symbol.
#[must_use]
pub fn script_type_path(symbol: &str) -> DefPath {
    let (module, name) = split_source_symbol(symbol);
    DefPath::ty(SCRIPT_PACKAGE, module, name)
}

/// Returns the stable semantic identity for a script type symbol.
///
/// An explicit schema ID wins over path-derived identity so an intentional
/// `#[id(...)]` alias can preserve identity across a source rename.
#[must_use]
pub fn script_type_id(symbol: &str, explicit: Option<u128>) -> TypeId {
    explicit.map_or_else(
        || TypeId::from_def_id(script_type_path(symbol).id()),
        TypeId::new,
    )
}

/// Returns the canonical definition path for a script global symbol.
#[must_use]
pub fn script_global_path(symbol: &str) -> DefPath {
    let (module, name) = split_source_symbol(symbol);
    DefPath::global(SCRIPT_PACKAGE, module, name)
}

/// Returns the stable semantic identity for a script global symbol.
#[must_use]
pub fn script_global_id(symbol: &str) -> GlobalId {
    GlobalId::from_def_id(script_global_path(symbol).id())
}

/// Returns the canonical definition path for a variant of a script enum.
#[must_use]
pub fn script_variant_path(enum_symbol: &str, variant: &str) -> DefPath {
    let (module, owner) = split_source_symbol(enum_symbol);
    DefPath::variant(SCRIPT_PACKAGE, module, owner, variant)
}

/// Returns the stable semantic identity for a variant of a script enum.
///
/// `explicit` is the parsed numeric value of a schema `#[id(...)]`
/// attribute, when present.
#[must_use]
pub fn script_variant_id(enum_symbol: &str, variant: &str, explicit: Option<u128>) -> VariantId {
    explicit.map_or_else(
        || VariantId::from_def_id(script_variant_path(enum_symbol, variant).id()),
        VariantId::new,
    )
}

/// Returns the canonical definition path for a script record or variant
/// field.
///
/// `type_symbol` names the owning record or enum. `variant` is supplied only
/// for an enum payload field, keeping the module/type boundary unambiguous.
#[must_use]
pub fn script_field_path(type_symbol: &str, variant: Option<&str>, field: &str) -> DefPath {
    let (module, type_name) = split_source_symbol(type_symbol);
    let owner = variant.map_or_else(
        || type_name.to_owned(),
        |variant| format!("{type_name}::{variant}"),
    );
    DefPath::field(SCRIPT_PACKAGE, module, owner, field)
}

/// Returns the stable semantic identity for a script record or variant field.
///
/// `explicit` is the parsed numeric value of a schema `#[id(...)]`
/// attribute, when present.
#[must_use]
pub fn script_field_id(
    type_symbol: &str,
    variant: Option<&str>,
    field: &str,
    explicit: Option<u128>,
) -> FieldId {
    explicit.map_or_else(
        || FieldId::from_def_id(script_field_path(type_symbol, variant, field).id()),
        FieldId::new,
    )
}

fn split_source_symbol(symbol: &str) -> (Vec<&str>, &str) {
    let mut parts = symbol.split("::").collect::<Vec<_>>();
    let name = parts.pop().unwrap_or(symbol);
    (parts, name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_identity_uses_typed_canonical_paths() {
        assert_eq!(
            script_function_id("game::combat::grant_reward"),
            FunctionId::from_def_id(
                DefPath::function("script", ["game", "combat"], "grant_reward").id()
            )
        );
        assert_eq!(
            script_type_id("game::combat::Reward", None),
            TypeId::from_def_id(DefPath::ty("script", ["game", "combat"], "Reward").id())
        );
        assert_eq!(
            script_global_id("game::combat::current_reward"),
            GlobalId::from_def_id(
                DefPath::global("script", ["game", "combat"], "current_reward").id()
            )
        );
        assert_eq!(
            script_variant_id("game::combat::Outcome", "Granted", None),
            VariantId::from_def_id(
                DefPath::variant("script", ["game", "combat"], "Outcome", "Granted").id()
            )
        );
        assert_eq!(
            script_field_id("game::combat::Outcome", Some("Granted"), "value", None,),
            FieldId::from_def_id(
                DefPath::field("script", ["game", "combat"], "Outcome::Granted", "value",).id()
            )
        );
    }

    #[test]
    fn script_identity_keeps_unqualified_symbols_in_the_package_root() {
        assert_eq!(
            script_type_path("Reward"),
            DefPath::ty("script", std::iter::empty::<&str>(), "Reward")
        );
        assert_eq!(
            script_field_path("Reward", None, "count"),
            DefPath::field("script", std::iter::empty::<&str>(), "Reward", "count",)
        );
    }

    #[test]
    fn explicit_schema_ids_preserve_identity_across_renames() {
        assert_eq!(
            script_type_id("game::OldType", Some(101)),
            script_type_id("game::NewType", Some(101))
        );
        assert_eq!(
            script_variant_id("game::Outcome", "Active", Some(201)),
            script_variant_id("game::Outcome", "Started", Some(201))
        );
        assert_eq!(
            script_field_id("game::Reward", None, "count", Some(301)),
            script_field_id("game::Reward", None, "quantity", Some(301))
        );
    }

    #[test]
    fn path_derived_member_ids_do_not_depend_on_declaration_order() {
        let first = [
            script_field_id("game::Reward", None, "count", None),
            script_field_id("game::Reward", None, "item", None),
        ];
        let reordered = [
            script_field_id("game::Reward", None, "item", None),
            script_field_id("game::Reward", None, "count", None),
        ];

        assert_eq!(first[0], reordered[1]);
        assert_eq!(first[1], reordered[0]);
    }

    #[test]
    fn script_method_helpers_preserve_existing_fnv_identities() {
        assert_eq!(
            script_inherent_method_id("main::Player", "bonus"),
            MethodId::new(0xe0dc_50cc_b2ea_1381)
        );
        assert_eq!(
            script_trait_method_id("main::BonusSource", "bonus"),
            MethodId::new(0xbc3f_86dc_30f1_b48f)
        );
        assert_eq!(
            script_trait_method_id("PartialEq", "eq"),
            MethodId::new(0xafff_db83_17bd_1f5c)
        );
        assert_eq!(
            script_inherent_method_id("game::combat::Player", "bonus"),
            MethodId::new(u128::from(stable_id(
                "inherent_method",
                "game::combat::Player",
                "bonus"
            )))
        );
    }
}
