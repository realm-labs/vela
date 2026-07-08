use vela_common::PrimitiveTag;
use vela_hir::type_hint::HirTypeHint;

use crate::compiler::patterns::PatternBindingFacts;
use crate::compiler::record_shapes::ValueShape;
use crate::compiler::script_types::ScriptTypeFact;
use crate::compiler::value_types::RuntimeTypeFact;

pub(super) fn iterable_item_shape(shape: ValueShape) -> Option<ValueShape> {
    match shape {
        ValueShape::Array(element) | ValueShape::Set(element) => Some(*element),
        ValueShape::Map { key, value } => Some(ValueShape::map_entry(*key, *value)),
        _ => None,
    }
}

pub(super) fn i64_pattern_facts() -> PatternBindingFacts {
    PatternBindingFacts::value(Some(RuntimeTypeFact::primitive(PrimitiveTag::I64)))
}

pub(super) fn merge_type_hint_and_value_fact(
    hinted: Option<ScriptTypeFact>,
    value: Option<ScriptTypeFact>,
) -> Option<ScriptTypeFact> {
    match (hinted, value) {
        (Some(hinted), Some(value)) if hinted.type_name == value.type_name => {
            Some(ScriptTypeFact {
                type_name: hinted.type_name,
                enum_variant: value.enum_variant,
            })
        }
        (Some(hinted), _) => Some(hinted),
        (None, value) => value,
    }
}

pub(super) fn is_map_or_set_type_hint(hint: &HirTypeHint) -> bool {
    matches!(hint.path.as_slice(), [name] if matches!(name.as_str(), "Map" | "Set"))
}
