use vela_common::PrimitiveTag;
use vela_def::{DefPath, FieldId, FunctionId, MethodId, TypeId, VariantId};
use vela_registry::{
    FieldDef, FunctionDef, FunctionSignature, MethodDef, ParamDef, TypeDef, VariantDef,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StdParamSpec {
    pub name: &'static str,
    pub type_hint: &'static str,
    pub defaulted: bool,
}

impl StdParamSpec {
    #[must_use]
    pub const fn new(name: &'static str, type_hint: &'static str) -> Self {
        Self {
            name,
            type_hint,
            defaulted: false,
        }
    }

    #[must_use]
    pub const fn optional(name: &'static str, type_hint: &'static str) -> Self {
        Self {
            name,
            type_hint,
            defaulted: true,
        }
    }

    #[must_use]
    pub fn def(self) -> ParamDef {
        ParamDef::new(self.name, Some(self.type_hint)).defaulted(self.defaulted)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StdFunctionSpec {
    pub module: &'static str,
    pub name: &'static str,
    pub params: &'static [StdParamSpec],
    pub return_type: &'static str,
    pub docs: &'static str,
}

impl StdFunctionSpec {
    #[must_use]
    pub const fn new(
        module: &'static str,
        name: &'static str,
        params: &'static [StdParamSpec],
        return_type: &'static str,
        docs: &'static str,
    ) -> Self {
        Self {
            module,
            name,
            params,
            return_type,
            docs,
        }
    }

    #[must_use]
    pub fn path(self) -> DefPath {
        DefPath::function("std", [self.module], self.name)
    }

    #[must_use]
    pub fn id(self) -> FunctionId {
        FunctionId::from_def_id(self.path().id())
    }

    #[must_use]
    pub fn signature(self) -> FunctionSignature {
        FunctionSignature::new(
            self.params.iter().map(|param| param.def()),
            Some(self.return_type.to_owned()),
        )
    }

    #[must_use]
    pub fn def(self) -> FunctionDef {
        FunctionDef::new(self.path(), self.signature())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StdMethodSpec {
    pub owner: &'static str,
    pub name: &'static str,
    pub params: &'static [StdParamSpec],
    pub return_type: &'static str,
    pub docs: &'static str,
}

impl StdMethodSpec {
    #[must_use]
    pub const fn new(
        owner: &'static str,
        name: &'static str,
        params: &'static [StdParamSpec],
        return_type: &'static str,
        docs: &'static str,
    ) -> Self {
        Self {
            owner,
            name,
            params,
            return_type,
            docs,
        }
    }

    #[must_use]
    pub fn owner_type_id(self) -> TypeId {
        TypeId::from_def_id(DefPath::ty("std", std::iter::empty::<&str>(), self.owner).id())
    }

    #[must_use]
    pub fn path(self) -> DefPath {
        DefPath::method("std", std::iter::empty::<&str>(), self.owner, self.name)
    }

    #[must_use]
    pub fn id(self) -> MethodId {
        MethodId::from_def_id(self.path().id())
    }

    #[must_use]
    pub fn signature(self) -> FunctionSignature {
        FunctionSignature::new(
            self.params.iter().map(|param| param.def()),
            Some(self.return_type.to_owned()),
        )
    }

    #[must_use]
    pub fn def(self) -> MethodDef {
        MethodDef::new(self.path(), self.owner_type_id(), self.signature())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StdTypeSpec {
    pub name: &'static str,
    pub primitive: Option<PrimitiveTag>,
}

impl StdTypeSpec {
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            primitive: None,
        }
    }

    #[must_use]
    pub const fn primitive(name: &'static str, primitive: PrimitiveTag) -> Self {
        Self {
            name,
            primitive: Some(primitive),
        }
    }

    #[must_use]
    pub fn path(self) -> DefPath {
        DefPath::ty("std", std::iter::empty::<&str>(), self.name)
    }

    #[must_use]
    pub fn id(self) -> TypeId {
        TypeId::from_def_id(self.path().id())
    }

    #[must_use]
    pub fn def(self) -> TypeDef {
        let def = TypeDef::new(self.path());
        if let Some(primitive) = self.primitive {
            def.primitive_tag(primitive)
        } else {
            def
        }
    }

    #[must_use]
    pub const fn source_name(self) -> &'static str {
        match self.primitive {
            Some(primitive) => primitive.name(),
            None => self.name,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StdVariantSpec {
    pub owner: &'static str,
    pub name: &'static str,
}

impl StdVariantSpec {
    #[must_use]
    pub const fn new(owner: &'static str, name: &'static str) -> Self {
        Self { owner, name }
    }

    #[must_use]
    pub fn owner_type_id(self) -> TypeId {
        TypeId::from_def_id(DefPath::ty("std", std::iter::empty::<&str>(), self.owner).id())
    }

    #[must_use]
    pub fn path(self) -> DefPath {
        DefPath::variant("std", std::iter::empty::<&str>(), self.owner, self.name)
    }

    #[must_use]
    pub fn id(self) -> VariantId {
        VariantId::from_def_id(self.path().id())
    }

    #[must_use]
    pub fn def(self) -> VariantDef {
        VariantDef::new(self.path(), self.owner_type_id())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StdFieldSpec {
    pub owner: &'static str,
    pub name: &'static str,
}

impl StdFieldSpec {
    #[must_use]
    pub const fn new(owner: &'static str, name: &'static str) -> Self {
        Self { owner, name }
    }

    #[must_use]
    pub fn owner_type_id(self) -> TypeId {
        TypeId::from_def_id(DefPath::ty("std", std::iter::empty::<&str>(), self.owner).id())
    }

    #[must_use]
    pub fn path(self) -> DefPath {
        DefPath::field("std", std::iter::empty::<&str>(), self.owner, self.name)
    }

    #[must_use]
    pub fn id(self) -> FieldId {
        FieldId::from_def_id(self.path().id())
    }

    #[must_use]
    pub fn def(self) -> FieldDef {
        FieldDef::new(self.path(), self.owner_type_id())
    }
}

pub const STD_TYPES: &[StdTypeSpec] = &[
    StdTypeSpec::primitive("Null", PrimitiveTag::Null),
    StdTypeSpec::primitive("Bool", PrimitiveTag::Bool),
    StdTypeSpec::primitive("Char", PrimitiveTag::Char),
    StdTypeSpec::primitive("I8", PrimitiveTag::I8),
    StdTypeSpec::primitive("I16", PrimitiveTag::I16),
    StdTypeSpec::primitive("I32", PrimitiveTag::I32),
    StdTypeSpec::primitive("I64", PrimitiveTag::I64),
    StdTypeSpec::primitive("U8", PrimitiveTag::U8),
    StdTypeSpec::primitive("U16", PrimitiveTag::U16),
    StdTypeSpec::primitive("U32", PrimitiveTag::U32),
    StdTypeSpec::primitive("U64", PrimitiveTag::U64),
    StdTypeSpec::primitive("F32", PrimitiveTag::F32),
    StdTypeSpec::primitive("F64", PrimitiveTag::F64),
    StdTypeSpec::primitive("String", PrimitiveTag::String),
    StdTypeSpec::primitive("Bytes", PrimitiveTag::Bytes),
    StdTypeSpec::new("Array"),
    StdTypeSpec::new("Map"),
    StdTypeSpec::new("Set"),
    StdTypeSpec::new("Function"),
    StdTypeSpec::new("Closure"),
    StdTypeSpec::new("Iterator"),
    StdTypeSpec::new("Range"),
    StdTypeSpec::new("Option"),
    StdTypeSpec::new("Result"),
];

pub const STD_VARIANTS: &[StdVariantSpec] = &[
    StdVariantSpec::new("Option", "Some"),
    StdVariantSpec::new("Option", "None"),
    StdVariantSpec::new("Result", "Ok"),
    StdVariantSpec::new("Result", "Err"),
];

pub const STD_FIELDS: &[StdFieldSpec] = &[
    StdFieldSpec::new("Option::Some", "0"),
    StdFieldSpec::new("Result::Ok", "0"),
    StdFieldSpec::new("Result::Err", "0"),
];

pub const STD_FUNCTIONS: &[StdFunctionSpec] = &[
    StdFunctionSpec::new(
        "math",
        "max",
        &[
            StdParamSpec::new("left", "any"),
            StdParamSpec::new("right", "any"),
        ],
        "any",
        "Returns the larger numeric value.",
    ),
    StdFunctionSpec::new(
        "math",
        "min",
        &[
            StdParamSpec::new("left", "any"),
            StdParamSpec::new("right", "any"),
        ],
        "any",
        "Returns the smaller numeric value.",
    ),
    StdFunctionSpec::new(
        "math",
        "clamp",
        &[
            StdParamSpec::new("value", "any"),
            StdParamSpec::new("min", "any"),
            StdParamSpec::new("max", "any"),
        ],
        "any",
        "Clamps a numeric value between inclusive bounds.",
    ),
    StdFunctionSpec::new(
        "math",
        "lerp",
        &[
            StdParamSpec::new("start", "any"),
            StdParamSpec::new("end", "any"),
            StdParamSpec::new("t", "any"),
        ],
        "f64",
        "Linearly interpolates between two numeric values.",
    ),
    StdFunctionSpec::new(
        "math",
        "move_towards",
        &[
            StdParamSpec::new("current", "any"),
            StdParamSpec::new("target", "any"),
            StdParamSpec::new("max_delta", "any"),
        ],
        "any",
        "Moves a numeric value toward a target by at most max_delta.",
    ),
    StdFunctionSpec::new(
        "math",
        "distance2d",
        &[
            StdParamSpec::new("x1", "any"),
            StdParamSpec::new("y1", "any"),
            StdParamSpec::new("x2", "any"),
            StdParamSpec::new("y2", "any"),
        ],
        "f64",
        "Returns the 2D distance between two points.",
    ),
    StdFunctionSpec::new(
        "math",
        "distance3d",
        &[
            StdParamSpec::new("x1", "any"),
            StdParamSpec::new("y1", "any"),
            StdParamSpec::new("z1", "any"),
            StdParamSpec::new("x2", "any"),
            StdParamSpec::new("y2", "any"),
            StdParamSpec::new("z2", "any"),
        ],
        "f64",
        "Returns the 3D distance between two points.",
    ),
    StdFunctionSpec::new(
        "math",
        "pow",
        &[
            StdParamSpec::new("base", "any"),
            StdParamSpec::new("exponent", "any"),
        ],
        "any",
        "Raises a numeric base to a numeric exponent.",
    ),
    StdFunctionSpec::new(
        "math",
        "sqrt",
        &[StdParamSpec::new("value", "any")],
        "f64",
        "Returns the square root as a float.",
    ),
    StdFunctionSpec::new(
        "math",
        "sign",
        &[StdParamSpec::new("value", "any")],
        "i64",
        "Returns -1, 0, or 1 for the numeric sign.",
    ),
    StdFunctionSpec::new(
        "math",
        "floor",
        &[StdParamSpec::new("value", "any")],
        "i64",
        "Rounds a numeric value down to an integer.",
    ),
    StdFunctionSpec::new(
        "math",
        "ceil",
        &[StdParamSpec::new("value", "any")],
        "i64",
        "Rounds a numeric value up to an integer.",
    ),
    StdFunctionSpec::new(
        "math",
        "round",
        &[StdParamSpec::new("value", "any")],
        "i64",
        "Rounds a numeric value to the nearest integer.",
    ),
    StdFunctionSpec::new(
        "math",
        "abs",
        &[StdParamSpec::new("value", "any")],
        "any",
        "Returns the absolute numeric value.",
    ),
    StdFunctionSpec::new(
        "option",
        "some",
        &[StdParamSpec::new("value", "any")],
        "any",
        "Wraps a value in Option::Some.",
    ),
    StdFunctionSpec::new("option", "none", &[], "any", "Creates Option::None."),
    StdFunctionSpec::new(
        "option",
        "is_some",
        &[StdParamSpec::new("option", "any")],
        "bool",
        "Returns true when the value is Option::Some.",
    ),
    StdFunctionSpec::new(
        "option",
        "is_none",
        &[StdParamSpec::new("option", "any")],
        "bool",
        "Returns true when the value is Option::None.",
    ),
    StdFunctionSpec::new(
        "option",
        "unwrap_or",
        &[
            StdParamSpec::new("option", "any"),
            StdParamSpec::new("fallback", "any"),
        ],
        "any",
        "Returns the Option::Some payload or a fallback value.",
    ),
    StdFunctionSpec::new(
        "option",
        "ok_or",
        &[
            StdParamSpec::new("option", "any"),
            StdParamSpec::new("error", "any"),
        ],
        "any",
        "Converts Option::Some to Result::Ok or Option::None to Result::Err.",
    ),
    StdFunctionSpec::new(
        "option",
        "flatten",
        &[StdParamSpec::new("option", "any")],
        "any",
        "Flattens a nested Option value by one nesting layer.",
    ),
    StdFunctionSpec::new(
        "result",
        "ok",
        &[StdParamSpec::new("value", "any")],
        "any",
        "Wraps a success value in Result::Ok.",
    ),
    StdFunctionSpec::new(
        "result",
        "err",
        &[StdParamSpec::new("error", "any")],
        "any",
        "Wraps an error value in Result::Err.",
    ),
    StdFunctionSpec::new(
        "result",
        "is_ok",
        &[StdParamSpec::new("result", "any")],
        "bool",
        "Returns true when the value is Result::Ok.",
    ),
    StdFunctionSpec::new(
        "result",
        "is_err",
        &[StdParamSpec::new("result", "any")],
        "bool",
        "Returns true when the value is Result::Err.",
    ),
    StdFunctionSpec::new(
        "result",
        "unwrap_or",
        &[
            StdParamSpec::new("result", "any"),
            StdParamSpec::new("fallback", "any"),
        ],
        "any",
        "Returns the Result::Ok payload or a fallback value.",
    ),
    StdFunctionSpec::new(
        "result",
        "to_option",
        &[StdParamSpec::new("result", "any")],
        "any",
        "Converts Result::Ok to Option::Some and Result::Err to Option::None.",
    ),
    StdFunctionSpec::new(
        "result",
        "to_error_option",
        &[StdParamSpec::new("result", "any")],
        "any",
        "Converts Result::Err to Option::Some and Result::Ok to Option::None.",
    ),
    StdFunctionSpec::new(
        "result",
        "flatten",
        &[StdParamSpec::new("result", "any")],
        "any",
        "Flattens a nested Result value by one nesting layer.",
    ),
    StdFunctionSpec::new(
        "set",
        "from_array",
        &[StdParamSpec::new("values", "array")],
        "set",
        "Builds a set from array values.",
    ),
    StdFunctionSpec::new(
        "bytes",
        "from_hex",
        &[StdParamSpec::new("text", "string")],
        "Result",
        "Decodes hexadecimal text to bytes or returns an error string.",
    ),
    StdFunctionSpec::new(
        "i32",
        "from_i16",
        &[StdParamSpec::new("value", "i16")],
        "i32",
        "Widens an i16 value to i32.",
    ),
    StdFunctionSpec::new(
        "i64",
        "from_i32",
        &[StdParamSpec::new("value", "i32")],
        "i64",
        "Widens an i32 value to i64.",
    ),
    StdFunctionSpec::new(
        "u32",
        "from_u16",
        &[StdParamSpec::new("value", "u16")],
        "u32",
        "Widens a u16 value to u32.",
    ),
    StdFunctionSpec::new(
        "u64",
        "from_u32",
        &[StdParamSpec::new("value", "u32")],
        "u64",
        "Widens a u32 value to u64.",
    ),
    StdFunctionSpec::new(
        "f64",
        "from_f32",
        &[StdParamSpec::new("value", "f32")],
        "f64",
        "Widens an f32 value to f64.",
    ),
    StdFunctionSpec::new(
        "i16",
        "try_from_i64",
        &[StdParamSpec::new("value", "i64")],
        "Result",
        "Narrows an i64 value to i16 or returns an error string.",
    ),
    StdFunctionSpec::new(
        "i8",
        "try_from_i64",
        &[StdParamSpec::new("value", "i64")],
        "Result",
        "Narrows an i64 value to i8 or returns an error string.",
    ),
    StdFunctionSpec::new(
        "u16",
        "try_from_u64",
        &[StdParamSpec::new("value", "u64")],
        "Result",
        "Narrows a u64 value to u16 or returns an error string.",
    ),
    StdFunctionSpec::new(
        "u8",
        "try_from_u64",
        &[StdParamSpec::new("value", "u64")],
        "Result",
        "Narrows a u64 value to u8 or returns an error string.",
    ),
    StdFunctionSpec::new(
        "f32",
        "try_from_f64",
        &[StdParamSpec::new("value", "f64")],
        "Result",
        "Narrows a finite f64 value to f32 or returns an error string.",
    ),
    StdFunctionSpec::new(
        "u8",
        "wrapping_add",
        &[
            StdParamSpec::new("lhs", "u8"),
            StdParamSpec::new("rhs", "u8"),
        ],
        "u8",
        "Adds two u8 values with wrapping overflow semantics.",
    ),
    StdFunctionSpec::new(
        "u32",
        "wrapping_mul",
        &[
            StdParamSpec::new("lhs", "u32"),
            StdParamSpec::new("rhs", "u32"),
        ],
        "u32",
        "Multiplies two u32 values with wrapping overflow semantics.",
    ),
    StdFunctionSpec::new(
        "i8",
        "wrapping_add",
        &[
            StdParamSpec::new("lhs", "i8"),
            StdParamSpec::new("rhs", "i8"),
        ],
        "i8",
        "Adds two i8 values with wrapping overflow semantics.",
    ),
    StdFunctionSpec::new(
        "u8",
        "bit_and",
        &[
            StdParamSpec::new("lhs", "u8"),
            StdParamSpec::new("rhs", "u8"),
        ],
        "u8",
        "Applies bitwise AND to two u8 values.",
    ),
    StdFunctionSpec::new(
        "u8",
        "bit_or",
        &[
            StdParamSpec::new("lhs", "u8"),
            StdParamSpec::new("rhs", "u8"),
        ],
        "u8",
        "Applies bitwise OR to two u8 values.",
    ),
    StdFunctionSpec::new(
        "u8",
        "bit_xor",
        &[
            StdParamSpec::new("lhs", "u8"),
            StdParamSpec::new("rhs", "u8"),
        ],
        "u8",
        "Applies bitwise XOR to two u8 values.",
    ),
    StdFunctionSpec::new(
        "u8",
        "shift_left",
        &[
            StdParamSpec::new("value", "u8"),
            StdParamSpec::new("bits", "u32"),
        ],
        "u8",
        "Shifts a u8 value left; shifts at or beyond the width return zero.",
    ),
    StdFunctionSpec::new(
        "u8",
        "shift_right",
        &[
            StdParamSpec::new("value", "u8"),
            StdParamSpec::new("bits", "u32"),
        ],
        "u8",
        "Shifts a u8 value right; shifts at or beyond the width return zero.",
    ),
    StdFunctionSpec::new(
        "u8",
        "rotate_left",
        &[
            StdParamSpec::new("value", "u8"),
            StdParamSpec::new("bits", "u32"),
        ],
        "u8",
        "Rotates a u8 value left.",
    ),
    StdFunctionSpec::new(
        "u8",
        "rotate_right",
        &[
            StdParamSpec::new("value", "u8"),
            StdParamSpec::new("bits", "u32"),
        ],
        "u8",
        "Rotates a u8 value right.",
    ),
];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use vela_def::DefPath;
    use vela_registry::RegistryError;

    use super::*;
    use crate::{STD_METHODS, standard_registry};

    #[test]
    fn manifest_declares_current_stdlib_surface() {
        assert_eq!(STD_TYPES.len(), 24);
        assert_eq!(STD_VARIANTS.len(), 4);
        assert_eq!(STD_FIELDS.len(), 3);
        assert_eq!(STD_FUNCTIONS.len(), 51);
        assert_eq!(STD_METHODS.len(), 153);
    }

    #[test]
    fn manifest_contains_representative_current_functions_and_methods() {
        assert!(
            STD_FUNCTIONS
                .iter()
                .any(|spec| spec.module == "math" && spec.name == "distance3d")
        );
        assert!(
            STD_FUNCTIONS
                .iter()
                .any(|spec| spec.module == "set" && spec.name == "from_array")
        );
        assert!(
            STD_FUNCTIONS
                .iter()
                .any(|spec| spec.module == "bytes" && spec.name == "from_hex")
        );
        assert!(
            STD_FUNCTIONS
                .iter()
                .any(|spec| spec.module == "i32" && spec.name == "from_i16")
        );
        assert!(
            STD_FUNCTIONS
                .iter()
                .any(|spec| spec.module == "i64" && spec.name == "from_i32")
        );
        assert!(
            STD_FUNCTIONS
                .iter()
                .any(|spec| spec.module == "i16" && spec.name == "try_from_i64")
        );
        assert!(
            STD_FUNCTIONS
                .iter()
                .any(|spec| spec.module == "i8" && spec.name == "try_from_i64")
        );
        assert!(
            STD_FUNCTIONS
                .iter()
                .any(|spec| spec.module == "u8" && spec.name == "wrapping_add")
        );
        assert!(
            STD_FUNCTIONS
                .iter()
                .any(|spec| spec.module == "u8" && spec.name == "bit_and")
        );
        assert!(
            STD_METHODS
                .iter()
                .any(|spec| spec.owner == "Array" && spec.name == "sort_by")
        );
        assert!(
            STD_METHODS
                .iter()
                .any(|spec| spec.owner == "Map" && spec.name == "entries")
        );
        assert!(
            STD_METHODS
                .iter()
                .any(|spec| spec.owner == "Set" && spec.name == "symmetric_difference")
        );
        assert!(
            STD_METHODS
                .iter()
                .any(|spec| spec.owner == "String" && spec.name == "parse_bool")
        );
        assert!(
            STD_METHODS
                .iter()
                .any(|spec| spec.owner == "String" && spec.name == "parse_i64")
        );
        assert!(
            STD_METHODS
                .iter()
                .any(|spec| spec.owner == "String" && spec.name == "parse_u8")
        );
        assert!(
            STD_METHODS
                .iter()
                .any(|spec| spec.owner == "String" && spec.name == "parse_f32")
        );
        assert!(
            STD_METHODS
                .iter()
                .any(|spec| spec.owner == "String" && spec.name == "parse_char")
        );
        assert!(
            STD_METHODS
                .iter()
                .any(|spec| spec.owner == "Bytes" && spec.name == "read_u32_le")
        );
        assert!(
            STD_METHODS
                .iter()
                .any(|spec| spec.owner == "Bytes" && spec.name == "to_hex")
        );
        assert!(
            STD_METHODS
                .iter()
                .any(|spec| spec.owner == "Bytes" && spec.name == "values")
        );
        assert!(
            STD_METHODS
                .iter()
                .any(|spec| spec.owner == "Char" && spec.name == "is_ascii_digit")
        );
        assert!(
            STD_METHODS
                .iter()
                .any(|spec| spec.owner == "Iterator" && spec.name == "collect_map")
        );
        assert!(
            STD_METHODS
                .iter()
                .any(|spec| spec.owner == "Option" && spec.name == "and_then")
        );
        assert!(
            STD_METHODS
                .iter()
                .any(|spec| spec.owner == "Result" && spec.name == "map_err")
        );
        assert!(
            STD_METHODS
                .iter()
                .any(|spec| spec.owner == "Range" && spec.name == "len")
        );
    }

    #[test]
    fn option_and_result_schema_is_declared() {
        let type_names = STD_TYPES
            .iter()
            .map(|spec| spec.name)
            .collect::<BTreeSet<_>>();
        let variants = STD_VARIANTS
            .iter()
            .map(|spec| (spec.owner, spec.name))
            .collect::<BTreeSet<_>>();
        let fields = STD_FIELDS
            .iter()
            .map(|spec| (spec.owner, spec.name))
            .collect::<BTreeSet<_>>();

        assert!(type_names.contains("Option"));
        assert!(type_names.contains("Result"));
        assert!(variants.contains(&("Option", "Some")));
        assert!(variants.contains(&("Option", "None")));
        assert!(variants.contains(&("Result", "Ok")));
        assert!(variants.contains(&("Result", "Err")));
        assert!(fields.contains(&("Option::Some", "0")));
        assert!(fields.contains(&("Result::Ok", "0")));
        assert!(fields.contains(&("Result::Err", "0")));
    }

    #[test]
    fn primitive_schema_types_are_declared_without_old_int_float_types() {
        let primitive_types = STD_TYPES
            .iter()
            .filter_map(|spec| spec.primitive.map(|primitive| (spec.name, primitive)))
            .collect::<BTreeSet<_>>();
        let type_names = STD_TYPES
            .iter()
            .map(|spec| spec.name)
            .collect::<BTreeSet<_>>();

        assert_eq!(
            primitive_types,
            BTreeSet::from([
                ("Null", PrimitiveTag::Null),
                ("Bool", PrimitiveTag::Bool),
                ("Char", PrimitiveTag::Char),
                ("I8", PrimitiveTag::I8),
                ("I16", PrimitiveTag::I16),
                ("I32", PrimitiveTag::I32),
                ("I64", PrimitiveTag::I64),
                ("U8", PrimitiveTag::U8),
                ("U16", PrimitiveTag::U16),
                ("U32", PrimitiveTag::U32),
                ("U64", PrimitiveTag::U64),
                ("F32", PrimitiveTag::F32),
                ("F64", PrimitiveTag::F64),
                ("String", PrimitiveTag::String),
                ("Bytes", PrimitiveTag::Bytes),
            ])
        );
        assert!(!type_names.contains("Int"));
        assert!(!type_names.contains("Float"));
    }

    #[test]
    fn standard_registry_resolves_manifest_definitions() {
        let registry = standard_registry().expect("standard registry should build");
        let view = registry.compile_view();
        let math_max = STD_FUNCTIONS[0];
        let array_sort_by = STD_METHODS
            .iter()
            .copied()
            .find(|spec| spec.owner == "Array" && spec.name == "sort_by")
            .expect("Array::sort_by should be declared");
        let option_type = STD_TYPES
            .iter()
            .copied()
            .find(|spec| spec.name == "Option")
            .expect("Option should be declared");
        let option_some_field = STD_FIELDS[0];

        assert_eq!(
            view.resolve_native_function_path(&math_max.path()),
            Some(math_max.id())
        );
        assert_eq!(
            view.resolve_value_method(array_sort_by.owner_type_id(), "sort_by"),
            Some(array_sort_by.id())
        );
        assert_eq!(
            view.resolve_type(&option_type.path()),
            Some(option_type.id())
        );
        assert_eq!(
            registry.id_for_path(&option_some_field.path()),
            Some(option_some_field.id().def_id())
        );
        for spec in STD_TYPES.iter().filter(|spec| spec.primitive.is_some()) {
            let primitive = spec.primitive.expect("primitive spec should carry tag");
            assert_eq!(registry.primitive_type_id(primitive), Some(spec.id()));
            assert_eq!(view.type_primitive_kind(spec.id()), Some(primitive));
        }
        assert_eq!(
            view.resolve_type(&DefPath::ty("std", std::iter::empty::<&str>(), "Int")),
            None
        );
        assert_eq!(
            view.resolve_type(&DefPath::ty("std", std::iter::empty::<&str>(), "Float")),
            None
        );
    }

    #[test]
    fn registry_validation_rejects_duplicate_stdlib_names() {
        let mut registry = standard_registry().expect("standard registry should build");
        let error = registry
            .register_function(STD_FUNCTIONS[0].def())
            .expect_err("duplicate stdlib function should be rejected");

        assert!(matches!(error, RegistryError::DuplicatePath { .. }));
    }

    #[test]
    fn manifest_ids_are_derived_from_def_paths() {
        let function = STD_FUNCTIONS[0];
        let method = STD_METHODS[0];
        let variant = STD_VARIANTS[0];
        let field = STD_FIELDS[0];

        assert_eq!(
            function.id().def_id(),
            DefPath::function("std", ["math"], "max").id()
        );
        assert_eq!(
            method.id().def_id(),
            DefPath::method("std", std::iter::empty::<&str>(), "String", "len").id()
        );
        assert_eq!(
            variant.id().def_id(),
            DefPath::variant("std", std::iter::empty::<&str>(), "Option", "Some").id()
        );
        assert_eq!(
            field.id().def_id(),
            DefPath::field("std", std::iter::empty::<&str>(), "Option::Some", "0").id()
        );
    }
}
