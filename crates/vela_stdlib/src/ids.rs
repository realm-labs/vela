use vela_def::{FieldId, FunctionId, MethodId, TypeId, VariantId};

use crate::{STD_FIELDS, STD_FUNCTIONS, STD_METHODS, STD_TYPES, STD_VARIANTS};

#[must_use]
pub fn std_function_id(module: &str, name: &str) -> Option<FunctionId> {
    STD_FUNCTIONS
        .iter()
        .find(|spec| spec.module == module && spec.name == name)
        .map(|spec| spec.id())
}

#[must_use]
pub fn std_method_id(owner: &str, name: &str) -> Option<MethodId> {
    STD_METHODS
        .iter()
        .find(|spec| spec.owner == owner && spec.name == name)
        .map(|spec| spec.id())
}

#[must_use]
pub fn std_type_id(name: &str) -> Option<TypeId> {
    STD_TYPES
        .iter()
        .find(|spec| spec.name == name || spec.source_name() == name)
        .map(|spec| spec.id())
}

#[must_use]
pub fn std_variant_id(owner: &str, name: &str) -> Option<VariantId> {
    STD_VARIANTS
        .iter()
        .find(|spec| spec.owner == owner && spec.name == name)
        .map(|spec| spec.id())
}

#[must_use]
pub fn std_field_id(owner: &str, name: &str) -> Option<FieldId> {
    STD_FIELDS
        .iter()
        .find(|spec| spec.owner == owner && spec.name == name)
        .map(|spec| spec.id())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{STD_FIELDS, STD_FUNCTIONS, STD_METHODS, STD_TYPES, STD_VARIANTS};

    #[rstest::rstest]
    #[case::math_max("math", "max", Some(STD_FUNCTIONS[0].id()))]
    #[case::missing_module("missing", "max", None)]
    fn function_id_lookup(
        #[case] module: &str,
        #[case] name: &str,
        #[case] expected: Option<FunctionId>,
    ) {
        assert_eq!(std_function_id(module, name), expected);
    }

    #[rstest::rstest]
    #[case::string_len("String", "len", Some(STD_METHODS[0].id()))]
    #[case::missing_method("String", "missing", None)]
    fn method_id_lookup(
        #[case] owner: &str,
        #[case] name: &str,
        #[case] expected: Option<MethodId>,
    ) {
        assert_eq!(std_method_id(owner, name), expected);
    }

    #[rstest::rstest]
    #[case::unit("Unit", Some(STD_TYPES[0].id()))]
    #[case::unit_source_name("()", Some(STD_TYPES[0].id()))]
    #[case::i64("i64", Some(STD_TYPES[6].id()))]
    #[case::f64("f64", Some(STD_TYPES[12].id()))]
    #[case::bytes("Bytes", Some(STD_TYPES[14].id()))]
    #[case::lowercase_bytes("bytes", None)]
    #[case::missing_type("Missing", None)]
    #[case::removed_int("int", None)]
    #[case::removed_float("float", None)]
    fn type_id_lookup(#[case] name: &str, #[case] expected: Option<TypeId>) {
        assert_eq!(std_type_id(name), expected);
    }

    #[rstest::rstest]
    #[case::option_some("Option", "Some", Some(STD_VARIANTS[0].id()))]
    #[case::missing_variant("Option", "Missing", None)]
    fn variant_id_lookup(
        #[case] owner: &str,
        #[case] name: &str,
        #[case] expected: Option<VariantId>,
    ) {
        assert_eq!(std_variant_id(owner, name), expected);
    }

    #[rstest::rstest]
    #[case::option_some_payload("Option::Some", "0", Some(STD_FIELDS[0].id()))]
    #[case::missing_field("Option::Some", "missing", None)]
    fn field_id_lookup(#[case] owner: &str, #[case] name: &str, #[case] expected: Option<FieldId>) {
        assert_eq!(std_field_id(owner, name), expected);
    }
}
