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

    #[test]
    fn id_lookups_return_manifest_ids() {
        assert_eq!(std_function_id("math", "max"), Some(STD_FUNCTIONS[0].id()));
        assert_eq!(std_method_id("String", "len"), Some(STD_METHODS[0].id()));
        assert_eq!(std_type_id("Null"), Some(STD_TYPES[0].id()));
        assert_eq!(std_type_id("null"), Some(STD_TYPES[0].id()));
        assert_eq!(std_type_id("i64"), Some(STD_TYPES[5].id()));
        assert_eq!(std_type_id("f64"), Some(STD_TYPES[11].id()));
        assert_eq!(std_type_id("bytes"), Some(STD_TYPES[13].id()));
        assert_eq!(std_variant_id("Option", "Some"), Some(STD_VARIANTS[0].id()));
        assert_eq!(std_field_id("Option::Some", "0"), Some(STD_FIELDS[0].id()));
    }

    #[test]
    fn id_lookups_return_none_for_missing_defs() {
        assert_eq!(std_function_id("missing", "max"), None);
        assert_eq!(std_method_id("String", "missing"), None);
        assert_eq!(std_type_id("Missing"), None);
        assert_eq!(std_type_id("int"), None);
        assert_eq!(std_type_id("float"), None);
        assert_eq!(std_variant_id("Option", "Missing"), None);
        assert_eq!(std_field_id("Option::Some", "missing"), None);
    }
}
