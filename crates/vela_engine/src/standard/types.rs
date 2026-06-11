use vela_def::{FieldId, TypeId, VariantId};
use vela_reflect::modules::DeclOrigin;
use vela_reflect::registry::{FieldDesc, SchemaHash, TypeDesc, TypeKey, TypeKind, VariantDesc};

use super::methods::{
    array_method_descs, map_method_descs, option_method_descs, range_method_descs,
    result_method_descs, set_method_descs, string_method_descs,
};

pub(crate) fn standard_type_descs() -> Vec<TypeDesc> {
    let mut descs = vec![
        builtin_type(
            "null",
            required_std_type_id("Null"),
            TypeKind::Null,
            "Null value type.",
        ),
        builtin_type(
            "bool",
            required_std_type_id("Bool"),
            TypeKind::Bool,
            "Boolean value type.",
        ),
        builtin_type(
            "i64",
            required_std_type_id("I64"),
            TypeKind::Int,
            "Default integer scalar value type.",
        ),
        builtin_type(
            "f64",
            required_std_type_id("F64"),
            TypeKind::Float,
            "Default floating-point scalar value type.",
        ),
        string_type_desc(),
        array_type_desc(),
        map_type_desc(),
        set_type_desc(),
        range_type_desc(),
        builtin_type(
            "function",
            required_std_type_id("Function"),
            TypeKind::Function,
            "Callable function value type.",
        ),
        builtin_type(
            "closure",
            required_std_type_id("Closure"),
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
        .schema_hash(SchemaHash::new(vela_common::stable_id(
            "std_schema",
            "",
            name,
        )))
        .origin(DeclOrigin::Host)
        .docs(docs)
        .attr("stdlib", "builtin")
}

fn string_type_desc() -> TypeDesc {
    let mut desc = builtin_type(
        "string",
        required_std_type_id("String"),
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
        required_std_type_id("Array"),
        TypeKind::Array,
        "Array collection type.",
    );
    for method in array_method_descs() {
        desc = desc.method(method);
    }
    desc
}

fn map_type_desc() -> TypeDesc {
    let mut desc = builtin_type(
        "map",
        required_std_type_id("Map"),
        TypeKind::Map,
        "Map collection type.",
    );
    for method in map_method_descs() {
        desc = desc.method(method);
    }
    desc
}

fn set_type_desc() -> TypeDesc {
    let mut desc = builtin_type(
        "set",
        required_std_type_id("Set"),
        TypeKind::Set,
        "Set collection type.",
    );
    for method in set_method_descs() {
        desc = desc.method(method);
    }
    desc
}

fn range_type_desc() -> TypeDesc {
    let mut desc = builtin_type(
        "range",
        required_std_type_id("Range"),
        TypeKind::Range,
        "Range value type.",
    );
    for method in range_method_descs() {
        desc = desc.method(method);
    }
    desc
}

fn option_type_desc() -> TypeDesc {
    let mut desc = TypeDesc::new(TypeKey::new(required_std_type_id("Option"), "Option"))
        .kind(TypeKind::ScriptEnum)
        .schema_hash(SchemaHash::new(vela_common::stable_id(
            "std_schema",
            "",
            "Option",
        )))
        .origin(DeclOrigin::Host)
        .docs("Dynamic standard Option enum without script-language generics.")
        .attr("stdlib", "option")
        .variant(
            VariantDesc::new(required_std_variant_id("Option", "Some"), "Some")
                .origin(DeclOrigin::Host)
                .docs("Carries a present Option payload.")
                .attr("stdlib", "option")
                .field(
                    FieldDesc::new(required_std_field_id("Option::Some", "0"), "0")
                        .type_hint("any")
                        .docs("Dynamic Option::Some payload value.")
                        .attr("stdlib", "option"),
                ),
        )
        .variant(
            VariantDesc::new(required_std_variant_id("Option", "None"), "None")
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
    let mut desc = TypeDesc::new(TypeKey::new(required_std_type_id("Result"), "Result"))
        .kind(TypeKind::ScriptEnum)
        .schema_hash(SchemaHash::new(vela_common::stable_id(
            "std_schema",
            "",
            "Result",
        )))
        .origin(DeclOrigin::Host)
        .docs("Dynamic standard Result enum without script-language generics.")
        .attr("stdlib", "result")
        .variant(
            VariantDesc::new(required_std_variant_id("Result", "Ok"), "Ok")
                .origin(DeclOrigin::Host)
                .docs("Carries a successful Result payload.")
                .attr("stdlib", "result")
                .field(
                    FieldDesc::new(required_std_field_id("Result::Ok", "0"), "0")
                        .type_hint("any")
                        .docs("Dynamic Result::Ok payload value.")
                        .attr("stdlib", "result"),
                ),
        )
        .variant(
            VariantDesc::new(required_std_variant_id("Result", "Err"), "Err")
                .origin(DeclOrigin::Host)
                .docs("Carries a recoverable Result error payload.")
                .attr("stdlib", "result")
                .field(
                    FieldDesc::new(required_std_field_id("Result::Err", "0"), "0")
                        .type_hint("any")
                        .docs("Dynamic Result::Err payload value.")
                        .attr("stdlib", "result"),
                ),
        );
    for method in result_method_descs() {
        desc = desc.method(method);
    }
    desc
}

fn required_std_type_id(name: &str) -> TypeId {
    let Some(id) = vela_stdlib::std_type_id(name) else {
        panic!("missing standard type identity for {name}");
    };
    id
}

fn required_std_variant_id(owner: &str, name: &str) -> VariantId {
    let Some(id) = vela_stdlib::std_variant_id(owner, name) else {
        panic!("missing standard variant identity for {owner}::{name}");
    };
    id
}

fn required_std_field_id(owner: &str, name: &str) -> FieldId {
    let Some(id) = vela_stdlib::std_field_id(owner, name) else {
        panic!("missing standard field identity for {owner}::{name}");
    };
    id
}
