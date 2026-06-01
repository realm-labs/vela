use vela_common::{FieldId, FunctionId, TypeId, VariantId};
use vela_reflect::{DeclOrigin, FieldDesc, SchemaHash, TypeDesc, TypeKey, TypeKind, VariantDesc};

use crate::{EffectSet, FunctionAccess, NativeFunctionDesc, NativeFunctionId, TypeHint};

pub const MATH_MAX_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0100);
pub const MATH_MIN_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0101);
pub const MATH_CLAMP_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0102);
pub const MATH_LERP_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0103);
pub const MATH_MOVE_TOWARDS_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0104);
pub const MATH_DISTANCE2D_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0105);
pub const MATH_DISTANCE3D_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0106);
pub const MATH_POW_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0107);
pub const MATH_SQRT_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0108);
pub const MATH_SIGN_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0109);
pub const MATH_FLOOR_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_010a);
pub const MATH_CEIL_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_010b);
pub const MATH_ROUND_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_010c);
pub const MATH_ABS_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_010d);

pub const OPTION_SOME_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0200);
pub const OPTION_NONE_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0201);
pub const OPTION_IS_SOME_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0202);
pub const OPTION_IS_NONE_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0203);
pub const OPTION_UNWRAP_OR_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0204);
pub const OPTION_OK_OR_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0205);
pub const OPTION_FLATTEN_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0206);

pub const RESULT_OK_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0300);
pub const RESULT_ERR_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0301);
pub const RESULT_IS_OK_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0302);
pub const RESULT_IS_ERR_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0303);
pub const RESULT_UNWRAP_OR_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0304);
pub const RESULT_TO_OPTION_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0305);
pub const RESULT_TO_ERROR_OPTION_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0306);
pub const RESULT_FLATTEN_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0307);

pub const SET_FROM_ARRAY_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0400);

pub const NULL_TYPE_ID: TypeId = TypeId::new(0xff00_0500);
pub const BOOL_TYPE_ID: TypeId = TypeId::new(0xff00_0501);
pub const INT_TYPE_ID: TypeId = TypeId::new(0xff00_0502);
pub const FLOAT_TYPE_ID: TypeId = TypeId::new(0xff00_0503);
pub const STRING_TYPE_ID: TypeId = TypeId::new(0xff00_0504);
pub const ARRAY_TYPE_ID: TypeId = TypeId::new(0xff00_0505);
pub const MAP_TYPE_ID: TypeId = TypeId::new(0xff00_0506);
pub const SET_TYPE_ID: TypeId = TypeId::new(0xff00_0507);
pub const FUNCTION_TYPE_ID: TypeId = TypeId::new(0xff00_0508);
pub const CLOSURE_TYPE_ID: TypeId = TypeId::new(0xff00_0509);
pub const OPTION_TYPE_ID: TypeId = TypeId::new(0xff00_0600);
pub const RESULT_TYPE_ID: TypeId = TypeId::new(0xff00_0601);

const OPTION_SOME_VARIANT_ID: VariantId = VariantId::new(0xff00_0602);
const OPTION_NONE_VARIANT_ID: VariantId = VariantId::new(0xff00_0603);
const RESULT_OK_VARIANT_ID: VariantId = VariantId::new(0xff00_0604);
const RESULT_ERR_VARIANT_ID: VariantId = VariantId::new(0xff00_0605);
const OPTION_SOME_FIELD_ID: FieldId = FieldId::new(0xff00_0606);
const RESULT_OK_FIELD_ID: FieldId = FieldId::new(0xff00_0607);
const RESULT_ERR_FIELD_ID: FieldId = FieldId::new(0xff00_0608);

pub(crate) fn standard_native_function_descs() -> Vec<NativeFunctionDesc> {
    let mut descs = Vec::new();
    descs.extend(math_descs());
    descs.extend(option_descs());
    descs.extend(result_descs());
    descs.push(set_from_array_desc());
    descs
}

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
        builtin_type(
            "string",
            STRING_TYPE_ID,
            TypeKind::String,
            "String value type.",
        ),
        builtin_type(
            "array",
            ARRAY_TYPE_ID,
            TypeKind::Array,
            "Array collection type.",
        ),
        builtin_type("map", MAP_TYPE_ID, TypeKind::Map, "Map collection type."),
        builtin_type("set", SET_TYPE_ID, TypeKind::Set, "Set collection type."),
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

fn option_type_desc() -> TypeDesc {
    TypeDesc::new(TypeKey::new(OPTION_TYPE_ID, "Option"))
        .kind(TypeKind::ScriptEnum)
        .schema_hash(SchemaHash::new(0xff00_0600_0000_0001))
        .origin(DeclOrigin::Host)
        .docs("Dynamic standard Option enum without script-language generics.")
        .attr("stdlib", "option")
        .variant(
            VariantDesc::new(OPTION_SOME_VARIANT_ID, "Some")
                .origin(DeclOrigin::Host)
                .field(FieldDesc::new(OPTION_SOME_FIELD_ID, "0").type_hint("any")),
        )
        .variant(VariantDesc::new(OPTION_NONE_VARIANT_ID, "None").origin(DeclOrigin::Host))
}

fn result_type_desc() -> TypeDesc {
    TypeDesc::new(TypeKey::new(RESULT_TYPE_ID, "Result"))
        .kind(TypeKind::ScriptEnum)
        .schema_hash(SchemaHash::new(0xff00_0601_0000_0001))
        .origin(DeclOrigin::Host)
        .docs("Dynamic standard Result enum without script-language generics.")
        .attr("stdlib", "result")
        .variant(
            VariantDesc::new(RESULT_OK_VARIANT_ID, "Ok")
                .origin(DeclOrigin::Host)
                .field(FieldDesc::new(RESULT_OK_FIELD_ID, "0").type_hint("any")),
        )
        .variant(
            VariantDesc::new(RESULT_ERR_VARIANT_ID, "Err")
                .origin(DeclOrigin::Host)
                .field(FieldDesc::new(RESULT_ERR_FIELD_ID, "0").type_hint("any")),
        )
}

fn math_descs() -> [NativeFunctionDesc; 14] {
    [
        math_binary(
            "math.max",
            MATH_MAX_FUNCTION_ID,
            "left",
            "right",
            TypeHint::Any,
        ),
        math_binary(
            "math.min",
            MATH_MIN_FUNCTION_ID,
            "left",
            "right",
            TypeHint::Any,
        ),
        math_ternary(
            "math.clamp",
            MATH_CLAMP_FUNCTION_ID,
            ["value", "min", "max"],
            TypeHint::Any,
        ),
        math_ternary(
            "math.lerp",
            MATH_LERP_FUNCTION_ID,
            ["start", "end", "t"],
            TypeHint::Float,
        ),
        math_ternary(
            "math.move_towards",
            MATH_MOVE_TOWARDS_FUNCTION_ID,
            ["current", "target", "max_delta"],
            TypeHint::Any,
        ),
        math_distance2d(),
        math_distance3d(),
        math_binary(
            "math.pow",
            MATH_POW_FUNCTION_ID,
            "base",
            "exponent",
            TypeHint::Any,
        ),
        math_unary("math.sqrt", MATH_SQRT_FUNCTION_ID, TypeHint::Float),
        math_unary("math.sign", MATH_SIGN_FUNCTION_ID, TypeHint::Int),
        math_unary("math.floor", MATH_FLOOR_FUNCTION_ID, TypeHint::Int),
        math_unary("math.ceil", MATH_CEIL_FUNCTION_ID, TypeHint::Int),
        math_unary("math.round", MATH_ROUND_FUNCTION_ID, TypeHint::Int),
        math_unary("math.abs", MATH_ABS_FUNCTION_ID, TypeHint::Any),
    ]
}

fn option_descs() -> [NativeFunctionDesc; 7] {
    [
        option_desc("option.some", OPTION_SOME_FUNCTION_ID)
            .param("value", TypeHint::Any)
            .returns(TypeHint::Any),
        option_desc("option.none", OPTION_NONE_FUNCTION_ID).returns(TypeHint::Any),
        option_desc("option.is_some", OPTION_IS_SOME_FUNCTION_ID)
            .param("option", TypeHint::Any)
            .returns(TypeHint::Bool),
        option_desc("option.is_none", OPTION_IS_NONE_FUNCTION_ID)
            .param("option", TypeHint::Any)
            .returns(TypeHint::Bool),
        option_desc("option.unwrap_or", OPTION_UNWRAP_OR_FUNCTION_ID)
            .param("option", TypeHint::Any)
            .param("fallback", TypeHint::Any)
            .returns(TypeHint::Any),
        option_desc("option.ok_or", OPTION_OK_OR_FUNCTION_ID)
            .param("option", TypeHint::Any)
            .param("error", TypeHint::Any)
            .returns(TypeHint::Any),
        option_desc("option.flatten", OPTION_FLATTEN_FUNCTION_ID)
            .param("option", TypeHint::Any)
            .returns(TypeHint::Any),
    ]
}

fn result_descs() -> [NativeFunctionDesc; 8] {
    [
        result_desc("result.ok", RESULT_OK_FUNCTION_ID)
            .param("value", TypeHint::Any)
            .returns(TypeHint::Any),
        result_desc("result.err", RESULT_ERR_FUNCTION_ID)
            .param("error", TypeHint::Any)
            .returns(TypeHint::Any),
        result_desc("result.is_ok", RESULT_IS_OK_FUNCTION_ID)
            .param("result", TypeHint::Any)
            .returns(TypeHint::Bool),
        result_desc("result.is_err", RESULT_IS_ERR_FUNCTION_ID)
            .param("result", TypeHint::Any)
            .returns(TypeHint::Bool),
        result_desc("result.unwrap_or", RESULT_UNWRAP_OR_FUNCTION_ID)
            .param("result", TypeHint::Any)
            .param("fallback", TypeHint::Any)
            .returns(TypeHint::Any),
        result_desc("result.to_option", RESULT_TO_OPTION_FUNCTION_ID)
            .param("result", TypeHint::Any)
            .returns(TypeHint::Any),
        result_desc("result.to_error_option", RESULT_TO_ERROR_OPTION_FUNCTION_ID)
            .param("result", TypeHint::Any)
            .returns(TypeHint::Any),
        result_desc("result.flatten", RESULT_FLATTEN_FUNCTION_ID)
            .param("result", TypeHint::Any)
            .returns(TypeHint::Any),
    ]
}

fn set_from_array_desc() -> NativeFunctionDesc {
    stdlib_desc("set.from_array", SET_FROM_ARRAY_FUNCTION_ID, "set")
        .param("values", TypeHint::Array)
        .returns(TypeHint::Set)
        .docs("Set standard-library construction helper.")
}

fn math_unary(name: &'static str, id: NativeFunctionId, returns: TypeHint) -> NativeFunctionDesc {
    math_desc(name, id)
        .param("value", TypeHint::Any)
        .returns(returns)
}

fn math_binary(
    name: &'static str,
    id: NativeFunctionId,
    first: &'static str,
    second: &'static str,
    returns: TypeHint,
) -> NativeFunctionDesc {
    math_desc(name, id)
        .param(first, TypeHint::Any)
        .param(second, TypeHint::Any)
        .returns(returns)
}

fn math_ternary(
    name: &'static str,
    id: NativeFunctionId,
    params: [&'static str; 3],
    returns: TypeHint,
) -> NativeFunctionDesc {
    math_desc(name, id)
        .param(params[0], TypeHint::Any)
        .param(params[1], TypeHint::Any)
        .param(params[2], TypeHint::Any)
        .returns(returns)
}

fn math_distance2d() -> NativeFunctionDesc {
    math_desc("math.distance2d", MATH_DISTANCE2D_FUNCTION_ID)
        .param("x1", TypeHint::Any)
        .param("y1", TypeHint::Any)
        .param("x2", TypeHint::Any)
        .param("y2", TypeHint::Any)
        .returns(TypeHint::Float)
}

fn math_distance3d() -> NativeFunctionDesc {
    math_desc("math.distance3d", MATH_DISTANCE3D_FUNCTION_ID)
        .param("x1", TypeHint::Any)
        .param("y1", TypeHint::Any)
        .param("z1", TypeHint::Any)
        .param("x2", TypeHint::Any)
        .param("y2", TypeHint::Any)
        .param("z2", TypeHint::Any)
        .returns(TypeHint::Float)
}

fn math_desc(name: &'static str, id: NativeFunctionId) -> NativeFunctionDesc {
    stdlib_desc(name, id, "math").docs("Deterministic math standard-library helper.")
}

fn option_desc(name: &'static str, id: NativeFunctionId) -> NativeFunctionDesc {
    stdlib_desc(name, id, "option").docs("Option standard-library propagation helper.")
}

fn result_desc(name: &'static str, id: NativeFunctionId) -> NativeFunctionDesc {
    stdlib_desc(name, id, "result").docs("Result standard-library propagation helper.")
}

fn stdlib_desc(
    name: &'static str,
    id: NativeFunctionId,
    namespace: &'static str,
) -> NativeFunctionDesc {
    NativeFunctionDesc::new(name, id)
        .effects(EffectSet::pure())
        .access(FunctionAccess::public().reflect_callable(true))
        .attr("stdlib", namespace)
}
