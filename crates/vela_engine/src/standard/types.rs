use vela_common::TypeId;
use vela_reflect::modules::DeclOrigin;
use vela_reflect::registry::{FieldDesc, SchemaHash, TypeDesc, TypeKey, TypeKind, VariantDesc};

use super::ids::{
    ARRAY_TYPE_ID, BOOL_TYPE_ID, CLOSURE_TYPE_ID, FLOAT_TYPE_ID, FUNCTION_TYPE_ID, INT_TYPE_ID,
    MAP_TYPE_ID, NULL_TYPE_ID, OPTION_NONE_VARIANT_ID, OPTION_SOME_FIELD_ID,
    OPTION_SOME_VARIANT_ID, OPTION_TYPE_ID, RANGE_TYPE_ID, RESULT_ERR_FIELD_ID,
    RESULT_ERR_VARIANT_ID, RESULT_OK_FIELD_ID, RESULT_OK_VARIANT_ID, RESULT_TYPE_ID, SET_TYPE_ID,
    STRING_TYPE_ID,
};
use super::methods::{
    array_method_descs, map_method_descs, option_method_descs, range_method_descs,
    result_method_descs, set_method_descs, string_method_descs,
};

pub(crate) fn standard_type_descs() -> Vec<TypeDesc> {
    let mut descs = vec![
        builtin_type("null", NULL_TYPE_ID, TypeKind::Null, "Null value type."),
        builtin_type("bool", BOOL_TYPE_ID, TypeKind::Bool, "Boolean value type."),
        builtin_type("int", INT_TYPE_ID, TypeKind::Int, "Integer value type."),
        builtin_type(
            "float",
            FLOAT_TYPE_ID,
            TypeKind::Float,
            "Floating-point value type.",
        ),
        string_type_desc(),
        array_type_desc(),
        map_type_desc(),
        set_type_desc(),
        range_type_desc(),
        builtin_type(
            "function",
            FUNCTION_TYPE_ID,
            TypeKind::Function,
            "Callable function value type.",
        ),
        builtin_type(
            "closure",
            CLOSURE_TYPE_ID,
            TypeKind::Closure,
            "Callable closure value type.",
        ),
    ];
    descs.push(option_type_desc());
    descs.push(result_type_desc());
    descs
}

fn builtin_type(name: &'static str, id: TypeId, kind: TypeKind, docs: &'static str) -> TypeDesc {
    TypeDesc::new(TypeKey::new(id, name))
        .kind(kind)
        .schema_hash(SchemaHash::new(u64::from(id.get())))
        .origin(DeclOrigin::Host)
        .docs(docs)
        .attr("stdlib", "builtin")
}

fn string_type_desc() -> TypeDesc {
    let mut desc = builtin_type(
        "string",
        STRING_TYPE_ID,
        TypeKind::String,
        "String value type.",
    );
    for method in string_method_descs() {
        desc = desc.method(method);
    }
    desc
}

fn array_type_desc() -> TypeDesc {
    let mut desc = builtin_type(
        "array",
        ARRAY_TYPE_ID,
        TypeKind::Array,
        "Array collection type.",
    );
    for method in array_method_descs() {
        desc = desc.method(method);
    }
    desc
}

fn map_type_desc() -> TypeDesc {
    let mut desc = builtin_type("map", MAP_TYPE_ID, TypeKind::Map, "Map collection type.");
    for method in map_method_descs() {
        desc = desc.method(method);
    }
    desc
}

fn set_type_desc() -> TypeDesc {
    let mut desc = builtin_type("set", SET_TYPE_ID, TypeKind::Set, "Set collection type.");
    for method in set_method_descs() {
        desc = desc.method(method);
    }
    desc
}

fn range_type_desc() -> TypeDesc {
    let mut desc = builtin_type("range", RANGE_TYPE_ID, TypeKind::Range, "Range value type.");
    for method in range_method_descs() {
        desc = desc.method(method);
    }
    desc
}

fn option_type_desc() -> TypeDesc {
    let mut desc = TypeDesc::new(TypeKey::new(OPTION_TYPE_ID, "Option"))
        .kind(TypeKind::ScriptEnum)
        .schema_hash(SchemaHash::new(0xff00_0600_0000_0001))
        .origin(DeclOrigin::Host)
        .docs("Dynamic standard Option enum without script-language generics.")
        .attr("stdlib", "option")
        .variant(
            VariantDesc::new(OPTION_SOME_VARIANT_ID, "Some")
                .origin(DeclOrigin::Host)
                .docs("Carries a present Option payload.")
                .attr("stdlib", "option")
                .field(
                    FieldDesc::new(OPTION_SOME_FIELD_ID, "0")
                        .type_hint("any")
                        .docs("Dynamic Option.Some payload value.")
                        .attr("stdlib", "option"),
                ),
        )
        .variant(
            VariantDesc::new(OPTION_NONE_VARIANT_ID, "None")
                .origin(DeclOrigin::Host)
                .docs("Represents expected absence without a payload.")
                .attr("stdlib", "option"),
        );
    for method in option_method_descs() {
        desc = desc.method(method);
    }
    desc
}

fn result_type_desc() -> TypeDesc {
    let mut desc = TypeDesc::new(TypeKey::new(RESULT_TYPE_ID, "Result"))
        .kind(TypeKind::ScriptEnum)
        .schema_hash(SchemaHash::new(0xff00_0601_0000_0001))
        .origin(DeclOrigin::Host)
        .docs("Dynamic standard Result enum without script-language generics.")
        .attr("stdlib", "result")
        .variant(
            VariantDesc::new(RESULT_OK_VARIANT_ID, "Ok")
                .origin(DeclOrigin::Host)
                .docs("Carries a successful Result payload.")
                .attr("stdlib", "result")
                .field(
                    FieldDesc::new(RESULT_OK_FIELD_ID, "0")
                        .type_hint("any")
                        .docs("Dynamic Result.Ok payload value.")
                        .attr("stdlib", "result"),
                ),
        )
        .variant(
            VariantDesc::new(RESULT_ERR_VARIANT_ID, "Err")
                .origin(DeclOrigin::Host)
                .docs("Carries a recoverable Result error payload.")
                .attr("stdlib", "result")
                .field(
                    FieldDesc::new(RESULT_ERR_FIELD_ID, "0")
                        .type_hint("any")
                        .docs("Dynamic Result.Err payload value.")
                        .attr("stdlib", "result"),
                ),
        );
    for method in result_method_descs() {
        desc = desc.method(method);
    }
    desc
}
