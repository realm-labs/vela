use crate::{
    DefPath, FieldId, FunctionId, MethodId, StateId, TraitId, TypeId, VariantId, stable_id,
};

/// Returns the canonical definition path for a script function symbol.
///
/// `symbol` omits the package and includes any source module qualification,
/// for example `game::combat::grant_reward`.
#[must_use]
pub fn script_function_path(package: &str, symbol: &str) -> DefPath {
    let (module, name) = split_source_symbol(symbol);
    DefPath::function(package, module, name)
}

/// Returns the stable semantic identity for a script function symbol.
#[must_use]
pub fn script_function_id(package: &str, symbol: &str) -> FunctionId {
    FunctionId::from_def_id(script_function_path(package, symbol).id())
}

/// Returns the stable identity for an inherent script method.
///
/// `owner` is the source-qualified script type name selected by the compiler,
/// such as `main::Player` or `game::combat::Player`.
#[must_use]
pub fn script_inherent_method_id(package: &str, owner: &str, method: &str) -> MethodId {
    MethodId::new(u128::from(stable_id(
        "inherent_method",
        package,
        &format!("{owner}::{method}"),
    )))
}

/// Returns the stable identity shared by every implementation of a script
/// trait method.
///
/// `trait_name` is the source-qualified trait name selected by the compiler,
/// or the builtin comparison-trait name for builtin operator traits.
#[must_use]
pub fn script_trait_method_id(package: &str, trait_name: &str, method: &str) -> MethodId {
    MethodId::new(u128::from(stable_id(
        "trait_method",
        package,
        &format!("{trait_name}::{method}"),
    )))
}

/// Returns the canonical definition path for a script type symbol.
#[must_use]
pub fn script_type_path(package: &str, symbol: &str) -> DefPath {
    let (module, name) = split_source_symbol(symbol);
    DefPath::ty(package, module, name)
}

/// Returns the stable semantic identity for a script type symbol.
///
/// An explicit schema ID wins over path-derived identity so an intentional
/// `#[id(...)]` alias can preserve identity across a source rename.
#[must_use]
pub fn script_type_id(package: &str, symbol: &str, explicit: Option<u128>) -> TypeId {
    explicit.map_or_else(
        || TypeId::from_def_id(script_type_path(package, symbol).id()),
        |explicit| {
            TypeId::new(u128::from(stable_id(
                "explicit_type",
                package,
                &explicit.to_string(),
            )))
        },
    )
}

/// Returns the stable semantic identity for a script trait symbol.
#[must_use]
pub fn script_trait_id(package: &str, symbol: &str) -> TraitId {
    let (module, name) = split_source_symbol(symbol);
    TraitId::from_def_id(DefPath::trait_def(package, module, name).id())
}

/// Returns the canonical definition path for a script state symbol.
#[must_use]
pub fn script_state_path(package: &str, symbol: &str) -> DefPath {
    let (module, name) = split_source_symbol(symbol);
    DefPath::state(package, module, name)
}

/// Returns the stable semantic identity for a script state symbol.
#[must_use]
pub fn script_state_id(package: &str, symbol: &str) -> StateId {
    StateId::from_def_id(script_state_path(package, symbol).id())
}

/// Returns the stable executable identity for a script state initializer.
///
/// The initializer is an implementation detail of the state declaration, so
/// it deliberately shares the declaration's canonical definition path while
/// remaining type-separated from [`StateId`].
#[must_use]
pub fn script_state_initializer_id(package: &str, symbol: &str) -> FunctionId {
    FunctionId::from_def_id(script_state_path(package, symbol).id())
}

/// Returns the canonical definition path for a variant of a script enum.
#[must_use]
pub fn script_variant_path(package: &str, enum_symbol: &str, variant: &str) -> DefPath {
    let (module, owner) = split_source_symbol(enum_symbol);
    DefPath::variant(package, module, owner, variant)
}

/// Returns the stable semantic identity for a variant of a script enum.
///
/// `explicit` is the parsed numeric value of a schema `#[id(...)]`
/// attribute, when present.
#[must_use]
pub fn script_variant_id(
    package: &str,
    enum_symbol: &str,
    variant: &str,
    explicit: Option<u128>,
) -> VariantId {
    explicit.map_or_else(
        || VariantId::from_def_id(script_variant_path(package, enum_symbol, variant).id()),
        |explicit| {
            VariantId::new(u128::from(stable_id(
                "explicit_variant",
                package,
                &explicit.to_string(),
            )))
        },
    )
}

/// Returns the canonical definition path for a script record or variant
/// field.
///
/// `type_symbol` names the owning record or enum. `variant` is supplied only
/// for an enum payload field, keeping the module/type boundary unambiguous.
#[must_use]
pub fn script_field_path(
    package: &str,
    type_symbol: &str,
    variant: Option<&str>,
    field: &str,
) -> DefPath {
    let (module, type_name) = split_source_symbol(type_symbol);
    let owner = variant.map_or_else(
        || type_name.to_owned(),
        |variant| format!("{type_name}::{variant}"),
    );
    DefPath::field(package, module, owner, field)
}

/// Returns the stable semantic identity for a script record or variant field.
///
/// `explicit` is the parsed numeric value of a schema `#[id(...)]`
/// attribute, when present.
#[must_use]
pub fn script_field_id(
    package: &str,
    type_symbol: &str,
    variant: Option<&str>,
    field: &str,
    explicit: Option<u128>,
) -> FieldId {
    explicit.map_or_else(
        || FieldId::from_def_id(script_field_path(package, type_symbol, variant, field).id()),
        |explicit| {
            FieldId::new(u128::from(stable_id(
                "explicit_field",
                package,
                &explicit.to_string(),
            )))
        },
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

    const PACKAGE: &str = "com.example.game";

    #[test]
    fn same_symbol_path_in_two_packages_has_distinct_stable_ids() {
        let first = super::script_function_id("com.example.first", "game::main::run");
        let second = super::script_function_id("com.example.second", "game::main::run");

        assert_ne!(first, second);
        assert_ne!(
            super::script_type_id("com.example.first", "game::main::State", None),
            super::script_type_id("com.example.second", "game::main::State", None)
        );
    }

    fn script_function_id(symbol: &str) -> FunctionId {
        super::script_function_id(PACKAGE, symbol)
    }

    fn script_type_id(symbol: &str, explicit: Option<u128>) -> TypeId {
        super::script_type_id(PACKAGE, symbol, explicit)
    }

    fn script_state_id(symbol: &str) -> StateId {
        super::script_state_id(PACKAGE, symbol)
    }

    fn script_variant_id(symbol: &str, variant: &str, explicit: Option<u128>) -> VariantId {
        super::script_variant_id(PACKAGE, symbol, variant, explicit)
    }

    fn script_field_id(
        symbol: &str,
        variant: Option<&str>,
        field: &str,
        explicit: Option<u128>,
    ) -> FieldId {
        super::script_field_id(PACKAGE, symbol, variant, field, explicit)
    }

    fn script_type_path(symbol: &str) -> DefPath {
        super::script_type_path(PACKAGE, symbol)
    }

    fn script_field_path(symbol: &str, variant: Option<&str>, field: &str) -> DefPath {
        super::script_field_path(PACKAGE, symbol, variant, field)
    }

    fn script_inherent_method_id(owner: &str, method: &str) -> MethodId {
        super::script_inherent_method_id(PACKAGE, owner, method)
    }

    fn script_trait_method_id(owner: &str, method: &str) -> MethodId {
        super::script_trait_method_id(PACKAGE, owner, method)
    }

    #[test]
    fn script_identity_uses_typed_canonical_paths() {
        assert_eq!(
            script_function_id("game::combat::grant_reward"),
            FunctionId::from_def_id(
                DefPath::function(PACKAGE, ["game", "combat"], "grant_reward").id()
            )
        );
        assert_eq!(
            script_type_id("game::combat::Reward", None),
            TypeId::from_def_id(DefPath::ty(PACKAGE, ["game", "combat"], "Reward").id())
        );
        assert_eq!(
            script_state_id("game::combat::current_reward"),
            StateId::from_def_id(
                DefPath::state(PACKAGE, ["game", "combat"], "current_reward").id()
            )
        );
        assert_eq!(
            script_variant_id("game::combat::Outcome", "Granted", None),
            VariantId::from_def_id(
                DefPath::variant(PACKAGE, ["game", "combat"], "Outcome", "Granted").id()
            )
        );
        assert_eq!(
            script_field_id("game::combat::Outcome", Some("Granted"), "value", None,),
            FieldId::from_def_id(
                DefPath::field(PACKAGE, ["game", "combat"], "Outcome::Granted", "value",).id()
            )
        );
    }

    #[test]
    fn script_identity_keeps_unqualified_symbols_in_the_package_root() {
        assert_eq!(
            script_type_path("Reward"),
            DefPath::ty(PACKAGE, std::iter::empty::<&str>(), "Reward")
        );
        assert_eq!(
            script_field_path("Reward", None, "count"),
            DefPath::field(PACKAGE, std::iter::empty::<&str>(), "Reward", "count",)
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
            MethodId::new(u128::from(stable_id(
                "inherent_method",
                PACKAGE,
                "main::Player::bonus"
            )))
        );
        assert_eq!(
            script_trait_method_id("main::BonusSource", "bonus"),
            MethodId::new(u128::from(stable_id(
                "trait_method",
                PACKAGE,
                "main::BonusSource::bonus"
            )))
        );
        assert_eq!(
            script_trait_method_id("PartialEq", "eq"),
            MethodId::new(u128::from(stable_id(
                "trait_method",
                PACKAGE,
                "PartialEq::eq"
            )))
        );
        assert_eq!(
            script_inherent_method_id("game::combat::Player", "bonus"),
            MethodId::new(u128::from(stable_id(
                "inherent_method",
                PACKAGE,
                "game::combat::Player::bonus"
            )))
        );
    }
}
