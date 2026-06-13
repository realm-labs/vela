use vela_reflect::registry::MethodDesc;

use super::{MethodSpec, descs};

pub(crate) fn char_method_descs() -> Vec<MethodDesc> {
    descs("Char", CHAR_METHODS, "char")
}

const CHAR_METHODS: &[MethodSpec] = &[
    MethodSpec::new(
        "to_string",
        &[],
        "string",
        "Returns the character as a string.",
    ),
    MethodSpec::new(
        "is_whitespace",
        &[],
        "bool",
        "Returns true when the character is whitespace.",
    ),
    MethodSpec::new(
        "is_ascii",
        &[],
        "bool",
        "Returns true when the character is ASCII.",
    ),
    MethodSpec::new(
        "is_ascii_digit",
        &[],
        "bool",
        "Returns true when the character is an ASCII decimal digit.",
    ),
];
