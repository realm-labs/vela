use vela_bytecode::compiler::{compile_program_source, compile_program_source_with_options};
use vela_common::{FieldId, HostMethodId, HostObjectId, HostTypeId, SourceId, Span, TypeId};
use vela_host::{HostPath, HostRef, HostValue, MockStateAdapter, PatchOp, PatchTx};
use vela_hot_reload::{AccessAbi, FunctionAbi, MethodAbi};
use vela_reflect::{
    FieldAccess, FieldDesc, MethodDesc, ModuleDesc, ReflectErrorKind, ReflectPermission,
    ReflectPermissionSet, TypeDesc, TypeKey,
};
use vela_vm::{HostExecution, Value, VmErrorKind};

use crate::{
    EffectSet, Engine, FunctionAccess, NativeFunctionDesc, NativeFunctionId, NativeMethodDesc,
    ScriptHostMethodMetadata, ScriptReflectSchema, TypeHint,
};

use super::player_type;

struct ReflectOnlyPlayer;

impl ScriptReflectSchema for ReflectOnlyPlayer {
    fn script_reflect_type_desc() -> TypeDesc {
        TypeDesc::new(TypeKey::new(TypeId::new(9901), "ReflectOnlyPlayer"))
            .kind(vela_reflect::TypeKind::Host)
            .host_type(HostTypeId::new(9901))
            .field(FieldDesc::new(FieldId::new(1), "level"))
    }
}

struct MetadataOnlyPlayerMethods;

impl ScriptHostMethodMetadata for MetadataOnlyPlayerMethods {
    fn script_host_method_descs() -> Vec<NativeMethodDesc> {
        vec![
            NativeMethodDesc::new(
                TypeKey::new(TypeId::new(1), "Player"),
                HostMethodId::new(44),
                "metadata_bonus",
            )
            .param("amount", TypeHint::Int)
            .returns(TypeHint::Int)
            .effects(EffectSet::host_read()),
        ]
    }
}

#[test]
fn engine_builder_registers_reflect_schema_metadata() {
    let engine = Engine::builder()
        .register_reflect_schema::<ReflectOnlyPlayer>()
        .build()
        .expect("engine should build with reflect schema");

    let registry = engine.registry();
    let reflected = registry
        .type_by_name("ReflectOnlyPlayer")
        .expect("reflect schema should be registered");
    assert_eq!(reflected.key.id, TypeId::new(9901));
    assert_eq!(reflected.host_type_id, Some(HostTypeId::new(9901)));
    assert_eq!(reflected.fields[0].name, "level");
}

#[test]
fn engine_builder_registers_host_methods_from_metadata_trait() {
    let engine = Engine::builder()
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_host_methods::<MetadataOnlyPlayerMethods>()
        .build()
        .expect("engine should build with host method metadata");

    let registry = engine.registry();
    let player = registry
        .type_by_name("Player")
        .expect("player type should be registered");
    assert_eq!(player.methods.len(), 1);
    assert_eq!(player.methods[0].id, HostMethodId::new(44));
    assert_eq!(player.methods[0].name, "metadata_bonus");
    assert_eq!(
        player.methods[0].params[0].type_hint.as_deref(),
        Some("int")
    );
    assert!(player.methods[0].effects.reads_host);
}

#[test]
fn engine_registers_native_function_reflection_metadata() {
    let source_span = Span::new(SourceId::new(7), 12, 24);
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game.add", NativeFunctionId::new(21))
                .param("lhs", TypeHint::Int)
                .param("rhs", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::host_read())
                .access(
                    FunctionAccess::public()
                        .reflect_callable(true)
                        .require_permission("game.add"),
                )
                .docs("Adds two integers.")
                .attr("domain", "gameplay")
                .attr("stable", "true")
                .source_span(source_span),
            |_| Ok(Value::Int(0)),
        )
        .build()
        .expect("engine should build");

    let registry = engine.registry();
    let module = registry
        .module_by_name("game")
        .expect("native module metadata");
    assert_eq!(module.exports.len(), 1);
    assert_eq!(module.exports[0].name, "game.add");

    let function = registry
        .function_by_name("game.add")
        .expect("native function metadata");
    assert_eq!(function.name, "game.add");
    assert_eq!(function.module.as_deref(), Some("game"));
    assert!(function.public);
    assert_eq!(function.params.len(), 2);
    assert_eq!(function.params[0].name, "lhs");
    assert_eq!(function.params[0].type_hint.as_deref(), Some("int"));
    assert_eq!(function.params[1].name, "rhs");
    assert_eq!(function.params[1].type_hint.as_deref(), Some("int"));
    assert_eq!(function.return_type.as_deref(), Some("int"));
    assert!(function.effects.reads_host);
    assert!(!function.effects.writes_host);
    assert!(function.access.reflect_visible);
    assert!(function.access.reflect_callable);
    assert_eq!(
        function.access.required_permissions(),
        &["game.add".to_owned()]
    );
    assert_eq!(function.docs.as_deref(), Some("Adds two integers."));
    assert_eq!(function.attrs.get("domain"), Some("gameplay"));
    assert_eq!(function.attrs.get("stable"), Some("true"));
    assert_eq!(function.source_span, Some(source_span));

    let function_abi = FunctionAbi::from_function(function);
    assert_eq!(
        function_abi.access,
        AccessAbi::function(true, true, true, vec!["game.add".to_owned()])
    );
    assert_eq!(function_abi.source_span, Some(source_span));
}

#[test]
fn engine_standard_natives_register_reflection_metadata() {
    let engine = Engine::builder()
        .with_standard_natives()
        .reflection_permissions(ReflectPermissionSet::new().with(ReflectPermission::ReadTypeInfo))
        .build()
        .expect("engine should build with standard natives");
    let registry = engine.registry();

    let string_type = registry.type_by_name("string").expect("string type");
    assert_eq!(string_type.kind, vela_reflect::TypeKind::String);
    assert_eq!(string_type.attrs.get("stdlib"), Some("builtin"));
    let trim = string_type
        .methods
        .iter()
        .find(|method| method.name == "trim")
        .expect("string.trim method metadata");
    assert_eq!(trim.return_type.as_deref(), Some("string"));
    assert_eq!(trim.attrs.get("stdlib"), Some("string"));
    let split_once = string_type
        .methods
        .iter()
        .find(|method| method.name == "split_once")
        .expect("string.split_once method metadata");
    assert_eq!(split_once.params[0].name, "separator");
    assert_eq!(split_once.params[0].type_hint.as_deref(), Some("string"));
    assert_eq!(split_once.return_type.as_deref(), Some("Option"));
    let parse_int = string_type
        .methods
        .iter()
        .find(|method| method.name == "parse_int")
        .expect("string.parse_int method metadata");
    assert_eq!(parse_int.return_type.as_deref(), Some("Option"));

    let array_type = registry.type_by_name("array").expect("array type");
    assert_eq!(array_type.kind, vela_reflect::TypeKind::Array);
    assert_eq!(array_type.attrs.get("stdlib"), Some("builtin"));
    let array_push = array_type
        .methods
        .iter()
        .find(|method| method.name == "push")
        .expect("array.push method metadata");
    assert_eq!(array_push.params[0].type_hint.as_deref(), Some("any"));
    assert_eq!(array_push.return_type.as_deref(), Some("null"));
    let array_map = array_type
        .methods
        .iter()
        .find(|method| method.name == "map")
        .expect("array.map method metadata");
    assert_eq!(array_map.params[0].type_hint.as_deref(), Some("function"));
    assert_eq!(array_map.return_type.as_deref(), Some("array"));

    let map_type = registry.type_by_name("map").expect("map type");
    assert_eq!(map_type.kind, vela_reflect::TypeKind::Map);
    let map_get = map_type
        .methods
        .iter()
        .find(|method| method.name == "get")
        .expect("map.get method metadata");
    assert_eq!(map_get.params[0].name, "key");
    assert_eq!(map_get.return_type.as_deref(), Some("Option"));

    let set_type = registry.type_by_name("set").expect("set type");
    assert_eq!(set_type.kind, vela_reflect::TypeKind::Set);
    let set_union = set_type
        .methods
        .iter()
        .find(|method| method.name == "union")
        .expect("set.union method metadata");
    assert_eq!(set_union.params[0].type_hint.as_deref(), Some("set"));
    assert_eq!(set_union.return_type.as_deref(), Some("set"));

    let range_type = registry.type_by_name("range").expect("range type");
    assert_eq!(range_type.kind, vela_reflect::TypeKind::Range);
    assert_eq!(range_type.attrs.get("stdlib"), Some("builtin"));
    let range_len = range_type
        .methods
        .iter()
        .find(|method| method.name == "len")
        .expect("range.len method metadata");
    assert!(range_len.params.is_empty());
    assert_eq!(range_len.return_type.as_deref(), Some("int"));
    let range_is_empty = range_type
        .methods
        .iter()
        .find(|method| method.name == "is_empty")
        .expect("range.is_empty method metadata");
    assert_eq!(range_is_empty.return_type.as_deref(), Some("bool"));

    let option_type = registry.type_by_name("Option").expect("Option type");
    assert_eq!(option_type.kind, vela_reflect::TypeKind::ScriptEnum);
    assert_eq!(option_type.variants.len(), 2);
    assert_eq!(option_type.variants[0].name, "Some");
    assert_eq!(option_type.variants[0].fields[0].name, "0");
    assert_eq!(
        option_type.variants[0].fields[0].type_hint.as_deref(),
        Some("any")
    );
    assert_eq!(option_type.variants[1].name, "None");
    assert_eq!(option_type.attrs.get("stdlib"), Some("option"));
    let option_map = option_type
        .methods
        .iter()
        .find(|method| method.name == "map")
        .expect("Option.map method metadata");
    assert_eq!(option_map.params[0].name, "callback");
    assert_eq!(option_map.params[0].type_hint.as_deref(), Some("function"));
    assert_eq!(option_map.return_type.as_deref(), Some("Option"));
    assert_eq!(option_map.attrs.get("stdlib"), Some("option"));
    let option_ok_or = option_type
        .methods
        .iter()
        .find(|method| method.name == "ok_or")
        .expect("Option.ok_or method metadata");
    assert_eq!(option_ok_or.params[0].name, "error");
    assert_eq!(option_ok_or.return_type.as_deref(), Some("Result"));

    let result_type = registry.type_by_name("Result").expect("Result type");
    assert_eq!(result_type.kind, vela_reflect::TypeKind::ScriptEnum);
    assert_eq!(result_type.variants.len(), 2);
    assert_eq!(result_type.variants[0].name, "Ok");
    assert_eq!(result_type.variants[0].fields[0].name, "0");
    assert_eq!(
        result_type.variants[0].fields[0].type_hint.as_deref(),
        Some("any")
    );
    assert_eq!(result_type.variants[1].name, "Err");
    assert_eq!(result_type.variants[1].fields[0].name, "0");
    assert_eq!(
        result_type.variants[1].fields[0].type_hint.as_deref(),
        Some("any")
    );
    assert_eq!(result_type.attrs.get("stdlib"), Some("result"));
    let result_map_err = result_type
        .methods
        .iter()
        .find(|method| method.name == "map_err")
        .expect("Result.map_err method metadata");
    assert_eq!(result_map_err.params[0].name, "callback");
    assert_eq!(
        result_map_err.params[0].type_hint.as_deref(),
        Some("function")
    );
    assert_eq!(result_map_err.return_type.as_deref(), Some("Result"));
    assert_eq!(result_map_err.attrs.get("stdlib"), Some("result"));
    let result_to_error = result_type
        .methods
        .iter()
        .find(|method| method.name == "to_error_option")
        .expect("Result.to_error_option method metadata");
    assert_eq!(result_to_error.return_type.as_deref(), Some("Option"));

    let math = registry.module_by_name("math").expect("math module");
    assert_eq!(
        math.docs.as_deref(),
        Some("Deterministic math standard-library helpers.")
    );
    assert_eq!(math.attrs.get("stdlib"), Some("math"));
    assert_eq!(math.exports.len(), 14);
    assert!(math.exports.iter().any(|export| export.name == "math.max"));
    assert!(math.exports.iter().any(|export| export.name == "math.sqrt"));

    let max = registry.function_by_name("math.max").expect("math.max");
    assert_eq!(max.module.as_deref(), Some("math"));
    assert_eq!(max.params.len(), 2);
    assert_eq!(max.params[0].name, "left");
    assert_eq!(max.params[1].name, "right");
    assert_eq!(max.return_type.as_deref(), Some("any"));
    assert_eq!(max.attrs.get("stdlib"), Some("math"));
    assert!(max.access.reflect_visible);
    assert!(max.access.reflect_callable);

    let sqrt = registry.function_by_name("math.sqrt").expect("math.sqrt");
    assert_eq!(sqrt.return_type.as_deref(), Some("float"));

    let option = registry.module_by_name("option").expect("option module");
    assert_eq!(
        option.docs.as_deref(),
        Some("Option standard-library propagation helpers.")
    );
    assert_eq!(option.attrs.get("stdlib"), Some("option"));
    assert_eq!(option.exports.len(), 7);
    assert!(
        option
            .exports
            .iter()
            .any(|export| export.name == "option.some")
    );
    assert!(
        option
            .exports
            .iter()
            .any(|export| export.name == "option.unwrap_or")
    );

    let result = registry.module_by_name("result").expect("result module");
    assert_eq!(
        result.docs.as_deref(),
        Some("Result standard-library propagation helpers.")
    );
    assert_eq!(result.attrs.get("stdlib"), Some("result"));
    assert_eq!(result.exports.len(), 8);
    assert!(
        result
            .exports
            .iter()
            .any(|export| export.name == "result.ok")
    );
    assert!(
        result
            .exports
            .iter()
            .any(|export| export.name == "result.to_option")
    );

    let set = registry.module_by_name("set").expect("set module");
    assert_eq!(
        set.docs.as_deref(),
        Some("Set standard-library construction helpers.")
    );
    assert_eq!(set.attrs.get("stdlib"), Some("set"));
    assert_eq!(set.exports.len(), 1);
    assert_eq!(set.exports[0].name, "set.from_array");

    let option_some = registry
        .function_by_name("option.some")
        .expect("option.some");
    assert_eq!(option_some.module.as_deref(), Some("option"));
    assert_eq!(option_some.params[0].name, "value");
    assert_eq!(option_some.return_type.as_deref(), Some("any"));
    assert_eq!(option_some.attrs.get("stdlib"), Some("option"));

    let result_ok = registry.function_by_name("result.ok").expect("result.ok");
    assert_eq!(result_ok.module.as_deref(), Some("result"));
    assert_eq!(result_ok.params[0].name, "value");
    assert_eq!(result_ok.return_type.as_deref(), Some("any"));
    assert_eq!(result_ok.attrs.get("stdlib"), Some("result"));

    let set_from_array = registry
        .function_by_name("set.from_array")
        .expect("set.from_array");
    assert_eq!(set_from_array.module.as_deref(), Some("set"));
    assert_eq!(set_from_array.params[0].name, "values");
    assert_eq!(set_from_array.params[0].type_hint.as_deref(), Some("array"));
    assert_eq!(set_from_array.return_type.as_deref(), Some("set"));
    assert_eq!(set_from_array.attrs.get("stdlib"), Some("set"));

    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let math = reflect.module("math");
    let option_module = reflect.module("option");
    let result_module = reflect.module("result");
    let set_module = reflect.module("set");
    let string_type = reflect.type_info("string");
    let array_type = reflect.type_info("array");
    let option_type = reflect.type_info("Option");
    let result_type = reflect.type_info("Result");
    let null_value_type = reflect.type_of(null);
    let bool_value_type = reflect.type_of(true);
    let int_value_type = reflect.type_of(42);
    let float_value_type = reflect.type_of(1.5);
    let string_value_type = reflect.type_of("quest");
    let array_value_type = reflect.type_of(["quest"]);
    let map_value_type = reflect.type_of({"quest": 1});
    let set_value_type = reflect.type_of(set.from_array(["quest"]));
    let option_value_type = reflect.type_of(option.some(1));
    let result_value_type = reflect.type_of(result.ok(1));
    let closure_value_type = reflect.type_of(|value| value);
    let range_value_type = reflect.type_of(1..3);
    let option_variants = reflect.variants(option_type);
    let result_variants = reflect.variants(result_type);
    let string_methods = reflect.methods(string_type);
    let array_methods = reflect.methods(array_type);
    let option_methods = reflect.methods(option_type);
    let result_methods = reflect.methods(result_type);
    let map_type = reflect.type_info("map");
    let set_type = reflect.type_info("set");
    let range_type = reflect.type_info("range");
    let map_methods = reflect.methods(map_type);
    let set_methods = reflect.methods(set_type);
    let range_methods = reflect.methods(range_type);
    let trim = reflect.method(string_type, "trim");
    let split_once = reflect.method(string_type, "split_once");
    let parse_int = reflect.method(string_type, "parse_int");
    let array_push = reflect.method(array_type, "push");
    let array_map = reflect.method(array_type, "map");
    let map_get = reflect.method(map_type, "get");
    let set_union = reflect.method(set_type, "union");
    let range_len = reflect.method(range_type, "len");
    let range_is_empty = reflect.method(range_type, "is_empty");
    let option_map = reflect.method(option_type, "map");
    let option_ok_or = reflect.method(option_type, "ok_or");
    let result_map_err = reflect.method(result_type, "map_err");
    let result_to_error = reflect.method(result_type, "to_error_option");
    let max = reflect.function("math.max");
    let sqrt = reflect.function("math.sqrt");
    let some = reflect.function("option.some");
    let ok = reflect.function("result.ok");
    let set_from_array = reflect.function("set.from_array");
    let params = reflect.params(max);
    let some_params = reflect.params(some);
    let ok_params = reflect.params(ok);
    let set_params = reflect.params(set_from_array);
    let math_exports = reflect.exports(math);
    let option_exports = reflect.exports(option_module);
    let result_exports = reflect.exports(result_module);
    let set_exports = reflect.exports(set_module);
    let type_of_checks = reflect.name(null_value_type) == "null"
        && reflect.kind(null_value_type) == "null"
        && reflect.name(bool_value_type) == "bool"
        && reflect.kind(bool_value_type) == "bool"
        && reflect.name(int_value_type) == "int"
        && reflect.kind(int_value_type) == "int"
        && reflect.name(float_value_type) == "float"
        && reflect.kind(float_value_type) == "float"
        && reflect.name(string_value_type) == "string"
        && reflect.kind(string_value_type) == "string"
        && reflect.name(array_value_type) == "array"
        && reflect.kind(array_value_type) == "array"
        && reflect.name(map_value_type) == "map"
        && reflect.kind(map_value_type) == "map"
        && reflect.name(set_value_type) == "set"
        && reflect.kind(set_value_type) == "set"
        && reflect.name(option_value_type) == "Option"
        && reflect.kind(option_value_type) == "script_enum"
        && reflect.name(result_value_type) == "Result"
        && reflect.kind(result_value_type) == "script_enum"
        && reflect.name(closure_value_type) == "closure"
        && reflect.kind(closure_value_type) == "closure"
        && reflect.name(range_value_type) == "range"
        && reflect.kind(range_value_type) == "range";
    return reflect.has_function("math.max")
        && reflect.has_function("math.sqrt")
        && reflect.has_function("option.some")
        && reflect.has_function("result.ok")
        && reflect.has_function("set.from_array")
        && reflect.has_type("string")
        && reflect.has_type("array")
        && reflect.has_type("map")
        && reflect.has_type("set")
        && reflect.has_type("range")
        && reflect.has_type("Option")
        && reflect.has_type("Result")
        && reflect.kind(string_type) == "string"
        && reflect.kind(array_type) == "array"
        && reflect.kind(map_type) == "map"
        && reflect.kind(set_type) == "set"
        && reflect.kind(range_type) == "range"
        && reflect.kind(option_type) == "script_enum"
        && reflect.kind(result_type) == "script_enum"
        && type_of_checks
        && reflect.docs(math) == "Deterministic math standard-library helpers."
        && reflect.docs(option_module) == "Option standard-library propagation helpers."
        && reflect.docs(result_module) == "Result standard-library propagation helpers."
        && reflect.docs(set_module) == "Set standard-library construction helpers."
        && reflect.attr(math, "stdlib") == "math"
        && reflect.attr(option_module, "stdlib") == "option"
        && reflect.attr(result_module, "stdlib") == "result"
        && reflect.attr(set_module, "stdlib") == "set"
        && reflect.attr(string_type, "stdlib") == "builtin"
        && reflect.attr(option_type, "stdlib") == "option"
        && reflect.attr(result_type, "stdlib") == "result"
        && string_methods.len() >= 22
        && reflect.has_method(string_type, "trim")
        && reflect.has_method(string_type, "split_once")
        && reflect.has_method(string_type, "parse_int")
        && trim.owner == "string"
        && reflect.returns(trim) == "string"
        && reflect.attr(trim, "stdlib") == "string"
        && split_once.params.len() == 1
        && split_once.params[0].name == "separator"
        && split_once.params[0].type == "string"
        && reflect.returns(split_once) == "Option"
        && reflect.returns(parse_int) == "Option"
        && array_methods.len() >= 28
        && map_methods.len() >= 19
        && set_methods.len() >= 21
        && range_methods.len() == 2
        && option_methods.len() >= 9
        && result_methods.len() >= 10
        && reflect.has_method(array_type, "push")
        && reflect.has_method(array_type, "map")
        && reflect.has_method(map_type, "get")
        && reflect.has_method(map_type, "map_values")
        && reflect.has_method(set_type, "union")
        && reflect.has_method(set_type, "is_subset")
        && reflect.has_method(range_type, "len")
        && reflect.has_method(range_type, "is_empty")
        && reflect.has_method(option_type, "map")
        && reflect.has_method(option_type, "ok_or")
        && reflect.has_method(result_type, "map_err")
        && reflect.has_method(result_type, "to_error_option")
        && array_push.params[0].name == "value"
        && array_push.params[0].type == "any"
        && reflect.returns(array_push) == "null"
        && array_map.params[0].type == "function"
        && reflect.returns(array_map) == "array"
        && map_get.params[0].name == "key"
        && reflect.returns(map_get) == "Option"
        && set_union.params[0].type == "set"
        && reflect.returns(set_union) == "set"
        && range_len.params.is_empty()
        && reflect.returns(range_len) == "int"
        && reflect.attr(range_len, "stdlib") == "range"
        && range_is_empty.params.is_empty()
        && reflect.returns(range_is_empty) == "bool"
        && option_map.params[0].name == "callback"
        && option_map.params[0].type == "function"
        && reflect.returns(option_map) == "Option"
        && reflect.attr(option_map, "stdlib") == "option"
        && option_ok_or.params[0].name == "error"
        && reflect.returns(option_ok_or) == "Result"
        && result_map_err.params[0].name == "callback"
        && result_map_err.params[0].type == "function"
        && reflect.returns(result_map_err) == "Result"
        && reflect.attr(result_map_err, "stdlib") == "result"
        && reflect.returns(result_to_error) == "Option"
        && option_variants.len() == 2
        && option_variants[0].name == "Some"
        && option_variants[0].fields[0].name == "0"
        && option_variants[0].fields[0].type == "any"
        && option_variants[1].name == "None"
        && result_variants.len() == 2
        && result_variants[0].name == "Ok"
        && result_variants[0].fields[0].type == "any"
        && result_variants[1].name == "Err"
        && result_variants[1].fields[0].type == "any"
        && !reflect.has_function("math.random")
        && math_exports.len() == 14
        && math_exports.contains("math.max")
        && math_exports.contains("math.sqrt")
        && option_exports.len() == 7
        && option_exports.contains("option.some")
        && option_exports.contains("option.unwrap_or")
        && result_exports.len() == 8
        && result_exports.contains("result.ok")
        && result_exports.contains("result.to_option")
        && set_exports.len() == 1
        && set_exports.contains("set.from_array")
        && reflect.attr(max, "stdlib") == "math"
        && reflect.attr(some, "stdlib") == "option"
        && reflect.attr(ok, "stdlib") == "result"
        && reflect.attr(set_from_array, "stdlib") == "set"
        && reflect.returns(max) == "any"
        && reflect.returns(sqrt) == "float"
        && reflect.returns(some) == "any"
        && reflect.returns(ok) == "any"
        && reflect.returns(set_from_array) == "set"
        && params.len() == 2
        && params[0].name == "left"
        && params[1].name == "right"
        && some_params.len() == 1
        && some_params[0].name == "value"
        && ok_params.len() == 1
        && ok_params[0].name == "value"
        && set_params.len() == 1
        && set_params[0].name == "values"
        && set_params[0].type == "array";
}
"#,
    )
    .expect("program should compile");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine
            .into_vm()
            .run_program_with_host(&program, "main", &[], &mut host),
        Ok(Value::Bool(true))
    );
}

#[test]
fn engine_reflect_call_invokes_reflect_callable_native_functions() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game.add", NativeFunctionId::new(91))
                .param("lhs", TypeHint::Int)
                .param("rhs", TypeHint::Int)
                .returns(TypeHint::Int)
                .access(FunctionAccess::public().reflect_callable(true)),
            |args| {
                let [Value::Int(lhs), Value::Int(rhs)] = args else {
                    return Ok(Value::Null);
                };
                Ok(Value::Int(lhs + rhs))
            },
        )
        .reflection_permissions(ReflectPermissionSet::all())
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let add = reflect.function("game.add");
    return reflect.call(add, 2, 3);
}
"#,
    )
    .expect("program should compile");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine
            .into_vm()
            .run_program_with_host(&program, "main", &[], &mut host),
        Ok(Value::Int(5))
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn engine_reflect_call_rejects_non_callable_native_functions() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game.add", NativeFunctionId::new(92))
                .param("lhs", TypeHint::Int)
                .param("rhs", TypeHint::Int)
                .returns(TypeHint::Int),
            |_| Ok(Value::Int(0)),
        )
        .reflection_permissions(ReflectPermissionSet::all())
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let add = reflect.function("game.add");
    return reflect.call(add, 2, 3);
}
"#,
    )
    .expect("program should compile");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        engine
            .into_vm()
            .run_program_with_host(&program, "main", &[], &mut host),
        Err(error) if error.kind == VmErrorKind::Reflect(
            ReflectErrorKind::FunctionNotReflectCallable {
                function: "game.add".to_owned()
            }
        )
    ));
    assert!(tx.patches().is_empty());
}

#[test]
fn engine_reflect_call_invokes_host_native_functions_through_patch_tx() {
    let engine = Engine::builder()
        .grant_permission("player.write")
        .register_host_native_fn(
            NativeFunctionDesc::new("game.set_level", NativeFunctionId::new(93))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("level", TypeHint::Int)
                .returns(TypeHint::Null)
                .effects(EffectSet::host_write())
                .access(
                    FunctionAccess::public()
                        .reflect_callable(true)
                        .require_permission("player.write"),
                ),
            |args, host| {
                let [Value::HostRef(player), Value::Int(level)] = args else {
                    return Ok(Value::Null);
                };
                host.tx.set_path(
                    HostPath::new(*player).field(FieldId::new(1)),
                    HostValue::Int(*level),
                    None,
                )?;
                Ok(Value::Null)
            },
        )
        .reflection_permissions(ReflectPermissionSet::all())
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    let set_level = reflect.function("game.set_level");
    reflect.call(set_level, player, 12);
    return 1;
}
"#,
    )
    .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.into_vm().run_program_with_host(
            &program,
            "main",
            &[Value::HostRef(host_ref)],
            &mut host
        ),
        Ok(Value::Int(1))
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].path,
        HostPath::new(host_ref).field(FieldId::new(1))
    );
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(12)));
}

#[test]
fn engine_reflect_call_denies_effectful_native_functions_without_effect_permission() {
    let engine = Engine::builder()
        .grant_permission("player.write")
        .register_host_native_fn(
            NativeFunctionDesc::new("game.set_level", NativeFunctionId::new(94))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("level", TypeHint::Int)
                .returns(TypeHint::Null)
                .effects(EffectSet {
                    reads_host: false,
                    writes_host: true,
                    emits_events: false,
                })
                .access(
                    FunctionAccess::public()
                        .reflect_callable(true)
                        .require_permission("player.write"),
                ),
            |args, host| {
                let [Value::HostRef(player), Value::Int(level)] = args else {
                    return Ok(Value::Null);
                };
                host.tx.set_path(
                    HostPath::new(*player).field(FieldId::new(1)),
                    HostValue::Int(*level),
                    None,
                )?;
                Ok(Value::Null)
            },
        )
        .reflection_permissions(
            ReflectPermissionSet::new()
                .with(ReflectPermission::ReadTypeInfo)
                .with(ReflectPermission::CallMethods),
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    let set_level = reflect.function("game.set_level");
    reflect.call(set_level, player, 12);
    return 1;
}
"#,
    )
    .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        engine
            .into_vm()
            .run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
        Err(error) if error.kind == VmErrorKind::Reflect(
            ReflectErrorKind::FunctionEffectPermissionDenied {
                function: "game.set_level".to_owned(),
                permission: ReflectPermission::CallHostWriteMethods,
            }
        )
    ));
    assert!(tx.patches().is_empty());
}

#[test]
fn engine_builder_registers_module_reflection_metadata() {
    let engine = Engine::builder()
        .register_module(
            ModuleDesc::new("game.reward")
                .docs("Reward module.")
                .attr("domain", "gameplay"),
        )
        .register_native_fn(
            NativeFunctionDesc::new("game.reward.grant", NativeFunctionId::new(221))
                .returns(TypeHint::Bool),
            |_| Ok(Value::Bool(true)),
        )
        .build()
        .expect("engine should build");

    let registry = engine.registry();
    let module = registry
        .module_by_name("game.reward")
        .expect("registered module metadata");
    assert_eq!(module.docs.as_deref(), Some("Reward module."));
    assert_eq!(module.attrs.get("domain"), Some("gameplay"));
    assert_eq!(module.exports.len(), 1);
    assert_eq!(module.exports[0].name, "game.reward.grant");
}

#[test]
fn engine_registers_native_method_source_span_metadata() {
    let source_span = Span::new(SourceId::new(8), 30, 42);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_native_method_fn(
            NativeMethodDesc::new(owner, HostMethodId::new(51), "grant_exp")
                .param("amount", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::host_write())
                .source_span(source_span),
            |_, _, _| Ok(Value::Int(0)),
        )
        .build()
        .expect("engine should build");
    let registry = engine.registry();
    let method = registry
        .type_by_name("Player")
        .and_then(|desc| {
            desc.methods
                .iter()
                .find(|method| method.name == "grant_exp")
        })
        .expect("native method metadata");

    assert_eq!(method.source_span, Some(source_span));

    let method_abi = MethodAbi::from_method("Player", method);
    assert_eq!(method_abi.source_span, Some(source_span));
}

#[test]
fn engine_installs_permissioned_reflection_natives() {
    let engine = Engine::builder()
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .reflection_permissions(
            ReflectPermissionSet::read_only().with(ReflectPermission::InspectHostPath),
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    if reflect.name(player) == "Player" && reflect.get(player, "level") == 7 {
        reflect.set(player, "level", 8);
    }
    return 0;
}
"#,
    )
    .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(
        HostPath::new(host_ref).field(FieldId::new(1)),
        HostValue::Int(7),
    );
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        engine
            .into_vm()
            .run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
        Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::PermissionDenied {
            permission: ReflectPermission::WriteValueFields
        })
    ));
    assert!(tx.patches().is_empty());
}

#[test]
fn engine_compiler_keeps_reflect_module_calls_off_host_method_lowering() {
    let engine = Engine::builder()
        .register_type(
            player_type(TypeId::new(1), HostTypeId::new(1))
                .method(MethodDesc::new(HostMethodId::new(9), "set")),
        )
        .reflection_policy(vela_reflect::ReflectPolicy::all())
        .build()
        .expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    reflect.set(player, "level", 12);
    return reflect.get(player, "level");
}
"#,
        &engine.compiler_options(),
    )
    .expect("reflect.set should compile as a native module call");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(
        HostPath::new(host_ref).field(FieldId::new(1)),
        HostValue::Int(7),
    );
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.into_vm().run_program_with_host(
            &program,
            "main",
            &[Value::HostRef(host_ref)],
            &mut host
        ),
        Ok(Value::Int(12))
    );
    assert_eq!(tx.patches().len(), 1);
}

#[test]
fn engine_granted_permissions_unlock_reflection_metadata_lists() {
    let engine = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .host_type(HostTypeId::new(1))
                .field(
                    FieldDesc::new(FieldId::new(1), "secret_level")
                        .access(FieldAccess::new().require_permission("player.inspect")),
                ),
        )
        .register_native_fn(
            NativeFunctionDesc::new("game.secret_bonus", NativeFunctionId::new(77))
                .returns(TypeHint::Int)
                .access(
                    FunctionAccess::public()
                        .reflect_callable(true)
                        .require_permission("game.inspect"),
                ),
            |_| Ok(Value::Int(5)),
        )
        .grant_permission("player.inspect")
        .grant_permission("game.inspect")
        .reflection_permissions(ReflectPermissionSet::new().with(ReflectPermission::ReadTypeInfo))
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let fields = reflect.fields();
    let functions = reflect.functions();
    if fields.len() == 1
        && fields[0].owner == "Player"
        && fields[0].name == "secret_level"
        && functions.len() == 1
        && functions[0].name == "game.secret_bonus" {
        return 1;
    }
    return 0;
}
"#,
    )
    .expect("program should compile");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine
            .into_vm()
            .run_program_with_host(&program, "main", &[], &mut host),
        Ok(Value::Int(1))
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn engine_missing_permissions_hide_reflection_metadata_lists() {
    let engine = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .host_type(HostTypeId::new(1))
                .field(
                    FieldDesc::new(FieldId::new(1), "secret_level")
                        .access(FieldAccess::new().require_permission("player.inspect")),
                ),
        )
        .register_native_fn(
            NativeFunctionDesc::new("game.secret_bonus", NativeFunctionId::new(77))
                .returns(TypeHint::Int)
                .access(
                    FunctionAccess::public()
                        .reflect_callable(true)
                        .require_permission("game.inspect"),
                ),
            |_| Ok(Value::Int(5)),
        )
        .reflection_permissions(ReflectPermissionSet::new().with(ReflectPermission::ReadTypeInfo))
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return reflect.fields().len() + reflect.functions().len();
}
"#,
    )
    .expect("program should compile");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine
            .into_vm()
            .run_program_with_host(&program, "main", &[], &mut host),
        Ok(Value::Int(0))
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn engine_installs_reflection_lookup_budget() {
    let engine = Engine::builder()
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .reflection_lookup_budget(1)
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    reflect.name(player);
    reflect.kind(player);
    return 1;
}
"#,
    )
    .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        engine
            .into_vm()
            .run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
        Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::LookupBudgetExceeded {
            limit: 1
        })
    ));
    assert!(tx.patches().is_empty());
}

#[test]
fn engine_reflect_call_denies_unapproved_native_methods() {
    let method = HostMethodId::new(6);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_native_method_fn(
            NativeMethodDesc::new(owner, method, "grant_exp")
                .effects(EffectSet::host_write())
                .access(
                    FunctionAccess::public()
                        .reflect_callable(true)
                        .require_permission("player.grant_exp"),
                ),
            |_, _, _| Ok(Value::Null),
        )
        .reflection_permissions(ReflectPermissionSet::all())
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    reflect.call(player, "grant_exp", 10);
    return 1;
}
"#,
    )
    .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        engine
            .into_vm()
            .run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
        Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::MethodPermissionDenied {
            method: "grant_exp".to_owned(),
            permission: "player.grant_exp".to_owned()
        })
    ));
    assert!(tx.patches().is_empty());
}

#[test]
fn engine_reflect_call_records_approved_native_methods() {
    let method = HostMethodId::new(6);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .grant_permission("player.grant_exp")
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_native_method_fn(
            NativeMethodDesc::new(owner, method, "grant_exp")
                .effects(EffectSet::host_write())
                .access(
                    FunctionAccess::public()
                        .reflect_callable(true)
                        .require_permission("player.grant_exp"),
                ),
            |_, _, _| Ok(Value::Null),
        )
        .reflection_permissions(ReflectPermissionSet::all())
        .build()
        .expect("engine should build");
    let registry = engine.registry();
    let reflected_method = registry
        .type_by_name("Player")
        .and_then(|desc| {
            desc.methods
                .iter()
                .find(|method| method.name == "grant_exp")
        })
        .expect("reflected method");
    assert!(reflected_method.access.reflect_callable);
    assert!(reflected_method.effects.writes_host);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    reflect.call(player, "grant_exp", 10);
    return 1;
}
"#,
    )
    .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.into_vm().run_program_with_host(
            &program,
            "main",
            &[Value::HostRef(host_ref)],
            &mut host
        ),
        Ok(Value::Int(1))
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::Int(10)]
        }
    );
}
