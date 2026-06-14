use super::*;
use vela_bytecode::UnlinkedProgram;
use vela_vm::budget::ExecutionBudget;
use vela_vm::error::VmResult;
use vela_vm::owned_value::OwnedValue;

fn run_linked_program_with_host(
    engine: &Engine,
    program: &UnlinkedProgram,
    args: &[OwnedValue],
    host: &mut HostExecution<'_>,
) -> VmResult<OwnedValue> {
    let linked = engine
        .link_program(program)
        .expect("engine reflection metadata test program should link");
    let mut budget = ExecutionBudget::unbounded();
    engine
        .into_vm_for_program(program)
        .run_linked_program_with_host_budget_and_caches(
            &linked,
            "main",
            args,
            host,
            &mut budget,
            None,
        )
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
        Some("i64")
    );
    assert!(player.methods[0].effects.reads_host);
}

#[test]
fn engine_registers_native_function_reflection_metadata() {
    let source_span = Span::new(SourceId::new(7), 12, 24);
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::add", NativeFunctionId::new(21))
                .param("lhs", TypeHint::i64())
                .param("rhs", TypeHint::i64())
                .returns(TypeHint::i64())
                .effects(EffectSet::host_read())
                .access(FunctionAccess::public().reflect_callable(true))
                .docs("Adds two integers.")
                .attr("domain", "gameplay")
                .attr("stable", "true")
                .source_span(source_span),
            |_| Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(0))),
        )
        .build()
        .expect("engine should build");

    let registry = engine.registry();
    let module = registry
        .module_by_name("game")
        .expect("native module metadata");
    assert_eq!(module.exports.len(), 1);
    assert_eq!(module.exports[0].name, "game::add");

    let function = registry
        .function_by_name("game::add")
        .expect("native function metadata");
    assert_eq!(function.name, "game::add");
    assert_eq!(function.module.as_deref(), Some("game"));
    assert!(function.public);
    assert_eq!(function.params.len(), 2);
    assert_eq!(function.params[0].name, "lhs");
    assert_eq!(function.params[0].type_hint.as_deref(), Some("i64"));
    assert_eq!(function.params[1].name, "rhs");
    assert_eq!(function.params[1].type_hint.as_deref(), Some("i64"));
    assert_eq!(function.return_type.as_deref(), Some("i64"));
    assert!(function.effects.reads_host);
    assert!(!function.effects.writes_host);
    assert!(function.access.reflect_visible);
    assert!(function.access.reflect_callable);
    assert!(function.access.required_permissions().is_empty());
    assert_eq!(function.docs.as_deref(), Some("Adds two integers."));
    assert_eq!(function.attrs.get("domain"), Some("gameplay"));
    assert_eq!(function.attrs.get("stable"), Some("true"));
    assert_eq!(function.source_span, Some(source_span));

    let function_abi = FunctionAbi::from_function(function);
    assert_eq!(function_abi.access, AccessAbi::function(true, true, true));
    assert_eq!(function_abi.source_span, Some(source_span));
}

#[test]
fn engine_native_private_functions_are_hidden_from_reflection() {
    let engine = Engine::builder()
        .with_standard_natives()
        .register_native_fn(
            NativeFunctionDesc::new("game::private_roll", NativeFunctionId::new(22))
                .returns(TypeHint::i64())
                .access(FunctionAccess::private()),
            |_| Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(4))),
        )
        .reflection_permissions(ReflectPermissionSet::all())
        .build()
        .expect("engine should build");

    let registry = engine.registry();
    let function = registry
        .function_by_name("game::private_roll")
        .expect("native function metadata");
    assert!(!function.public);
    assert!(!function.access.reflect_visible);

    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
fn main() {
    let game = reflect::module("game");
    let exports = reflect::exports(game);
    return !reflect::has_function("game::private_roll")
        && !exports.contains("game::private_roll");
}
"#,
        )
        .expect("program should compile");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        run_linked_program_with_host(&engine, &program, &[], &mut host),
        Ok(OwnedValue::Bool(true))
    );
}

#[test]
fn engine_native_private_functions_can_remain_reflect_visible() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::debug_probe", NativeFunctionId::new(23))
                .returns(TypeHint::boolean())
                .access(
                    FunctionAccess::private()
                        .reflect_visible(true)
                        .reflect_callable(false),
                ),
            |_| Ok(OwnedValue::Bool(true)),
        )
        .reflection_permissions(ReflectPermissionSet::all())
        .build()
        .expect("engine should build");

    let registry = engine.registry();
    let function = registry
        .function_by_name("game::debug_probe")
        .expect("native function metadata");
    assert!(!function.public);
    assert!(function.access.reflect_visible);
    assert!(!function.access.reflect_callable);

    let function_abi = FunctionAbi::from_function(function);
    assert_eq!(function_abi.access, AccessAbi::function(false, true, false));

    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
fn main() {
    let debug = reflect::function("game::debug_probe");
    return reflect::has_function("game::debug_probe")
        && !debug.public
        && debug.access.reflect_visible
        && !debug.access.reflect_callable;
}
"#,
        )
        .expect("program should compile");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        run_linked_program_with_host(&engine, &program, &[], &mut host),
        Ok(OwnedValue::Bool(true))
    );
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
    assert_eq!(string_type.kind, vela_reflect::registry::TypeKind::String);
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
    let parse_i64 = string_type
        .methods
        .iter()
        .find(|method| method.name == "parse_i64")
        .expect("string.parse_i64 method metadata");
    assert_eq!(parse_i64.return_type.as_deref(), Some("Option"));
    let parse_char = string_type
        .methods
        .iter()
        .find(|method| method.name == "parse_char")
        .expect("string.parse_char method metadata");
    assert_eq!(parse_char.return_type.as_deref(), Some("Option"));

    let array_type = registry.type_by_name("array").expect("array type");
    assert_eq!(array_type.kind, vela_reflect::registry::TypeKind::Array);
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
    assert_eq!(map_type.kind, vela_reflect::registry::TypeKind::Map);
    let map_get = map_type
        .methods
        .iter()
        .find(|method| method.name == "get")
        .expect("map.get method metadata");
    assert_eq!(map_get.params[0].name, "key");
    assert_eq!(map_get.return_type.as_deref(), Some("Option"));

    let set_type = registry.type_by_name("set").expect("set type");
    assert_eq!(set_type.kind, vela_reflect::registry::TypeKind::Set);
    let set_union = set_type
        .methods
        .iter()
        .find(|method| method.name == "union")
        .expect("set::union method metadata");
    assert_eq!(set_union.params[0].type_hint.as_deref(), Some("set"));
    assert_eq!(set_union.return_type.as_deref(), Some("set"));

    let range_type = registry.type_by_name("range").expect("range type");
    assert_eq!(range_type.kind, vela_reflect::registry::TypeKind::Range);
    assert_eq!(range_type.attrs.get("stdlib"), Some("builtin"));
    let range_len = range_type
        .methods
        .iter()
        .find(|method| method.name == "len")
        .expect("range.len method metadata");
    assert!(range_len.params.is_empty());
    assert_eq!(range_len.return_type.as_deref(), Some("i64"));
    let range_is_empty = range_type
        .methods
        .iter()
        .find(|method| method.name == "is_empty")
        .expect("range.is_empty method metadata");
    assert_eq!(range_is_empty.return_type.as_deref(), Some("bool"));

    let option_type = registry.type_by_name("Option").expect("Option type");
    assert_eq!(
        option_type.kind,
        vela_reflect::registry::TypeKind::ScriptEnum
    );
    assert_eq!(option_type.variants.len(), 2);
    assert_eq!(option_type.variants[0].name, "Some");
    assert_eq!(
        option_type.variants[0].docs.as_deref(),
        Some("Carries a present Option payload.")
    );
    assert_eq!(option_type.variants[0].attrs.get("stdlib"), Some("option"));
    assert_eq!(option_type.variants[0].fields[0].name, "0");
    assert_eq!(
        option_type.variants[0].fields[0].type_hint.as_deref(),
        Some("any")
    );
    assert_eq!(
        option_type.variants[0].fields[0].docs.as_deref(),
        Some("Dynamic Option::Some payload value.")
    );
    assert_eq!(
        option_type.variants[0].fields[0].attrs.get("stdlib"),
        Some("option")
    );
    assert_eq!(option_type.variants[1].name, "None");
    assert_eq!(
        option_type.variants[1].docs.as_deref(),
        Some("Represents expected absence without a payload.")
    );
    assert_eq!(option_type.variants[1].attrs.get("stdlib"), Some("option"));
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
    assert_eq!(
        result_type.kind,
        vela_reflect::registry::TypeKind::ScriptEnum
    );
    assert_eq!(result_type.variants.len(), 2);
    assert_eq!(result_type.variants[0].name, "Ok");
    assert_eq!(
        result_type.variants[0].docs.as_deref(),
        Some("Carries a successful Result payload.")
    );
    assert_eq!(result_type.variants[0].attrs.get("stdlib"), Some("result"));
    assert_eq!(result_type.variants[0].fields[0].name, "0");
    assert_eq!(
        result_type.variants[0].fields[0].type_hint.as_deref(),
        Some("any")
    );
    assert_eq!(
        result_type.variants[0].fields[0].docs.as_deref(),
        Some("Dynamic Result::Ok payload value.")
    );
    assert_eq!(
        result_type.variants[0].fields[0].attrs.get("stdlib"),
        Some("result")
    );
    assert_eq!(result_type.variants[1].name, "Err");
    assert_eq!(
        result_type.variants[1].docs.as_deref(),
        Some("Carries a recoverable Result error payload.")
    );
    assert_eq!(result_type.variants[1].attrs.get("stdlib"), Some("result"));
    assert_eq!(result_type.variants[1].fields[0].name, "0");
    assert_eq!(
        result_type.variants[1].fields[0].type_hint.as_deref(),
        Some("any")
    );
    assert_eq!(
        result_type.variants[1].fields[0].docs.as_deref(),
        Some("Dynamic Result::Err payload value.")
    );
    assert_eq!(
        result_type.variants[1].fields[0].attrs.get("stdlib"),
        Some("result")
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
    assert!(math.exports.iter().any(|export| export.name == "math::max"));
    assert!(
        math.exports
            .iter()
            .any(|export| export.name == "math::sqrt")
    );

    let max = registry.function_by_name("math::max").expect("math::max");
    assert_eq!(max.module.as_deref(), Some("math"));
    assert_eq!(max.params.len(), 2);
    assert_eq!(max.params[0].name, "left");
    assert_eq!(max.params[1].name, "right");
    assert_eq!(max.return_type.as_deref(), Some("any"));
    assert_eq!(max.attrs.get("stdlib"), Some("math"));
    assert_eq!(
        max.docs.as_deref(),
        Some("Returns the larger numeric value.")
    );
    assert!(max.access.reflect_visible);
    assert!(max.access.reflect_callable);

    let sqrt = registry.function_by_name("math::sqrt").expect("math::sqrt");
    assert_eq!(sqrt.return_type.as_deref(), Some("f64"));
    assert_eq!(
        sqrt.docs.as_deref(),
        Some("Returns the square root as a float.")
    );

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
            .any(|export| export.name == "option::some")
    );
    assert!(
        option
            .exports
            .iter()
            .any(|export| export.name == "option::unwrap_or")
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
            .any(|export| export.name == "result::ok")
    );
    assert!(
        result
            .exports
            .iter()
            .any(|export| export.name == "result::to_option")
    );

    let set = registry.module_by_name("set").expect("set module");
    assert_eq!(
        set.docs.as_deref(),
        Some("Set standard-library construction helpers.")
    );
    assert_eq!(set.attrs.get("stdlib"), Some("set"));
    assert_eq!(set.exports.len(), 1);
    assert_eq!(set.exports[0].name, "set::from_array");

    let option_some = registry
        .function_by_name("option::some")
        .expect("option::some");
    assert_eq!(option_some.module.as_deref(), Some("option"));
    assert_eq!(option_some.params[0].name, "value");
    assert_eq!(option_some.return_type.as_deref(), Some("any"));
    assert_eq!(option_some.attrs.get("stdlib"), Some("option"));
    assert_eq!(
        option_some.docs.as_deref(),
        Some("Wraps a value in Option::Some.")
    );

    let result_ok = registry.function_by_name("result::ok").expect("result::ok");
    assert_eq!(result_ok.module.as_deref(), Some("result"));
    assert_eq!(result_ok.params[0].name, "value");
    assert_eq!(result_ok.return_type.as_deref(), Some("any"));
    assert_eq!(result_ok.attrs.get("stdlib"), Some("result"));
    assert_eq!(
        result_ok.docs.as_deref(),
        Some("Wraps a success value in Result::Ok.")
    );

    let set_from_array = registry
        .function_by_name("set::from_array")
        .expect("set::from_array");
    assert_eq!(set_from_array.module.as_deref(), Some("set"));
    assert_eq!(set_from_array.params[0].name, "values");
    assert_eq!(set_from_array.params[0].type_hint.as_deref(), Some("array"));
    assert_eq!(set_from_array.return_type.as_deref(), Some("set"));
    assert_eq!(set_from_array.attrs.get("stdlib"), Some("set"));
    assert_eq!(
        set_from_array.docs.as_deref(),
        Some("Builds a set from array values.")
    );
    let bytes_from_hex = registry
        .function_by_name("bytes::from_hex")
        .expect("bytes::from_hex");
    assert_eq!(bytes_from_hex.module.as_deref(), Some("bytes"));
    assert_eq!(bytes_from_hex.params[0].name, "text");
    assert_eq!(
        bytes_from_hex.params[0].type_hint.as_deref(),
        Some("string")
    );
    assert_eq!(bytes_from_hex.return_type.as_deref(), Some("Result"));
    assert_eq!(bytes_from_hex.attrs.get("stdlib"), Some("bytes"));

    let program = engine
        .compile_source_with_id(SourceId::new(1),
            r#"
fn main() {
    let math = reflect::module("math");
    let option_module = reflect::module("option");
    let result_module = reflect::module("result");
    let set_module = reflect::module("set");
    let bytes_module = reflect::module("bytes");
    let string_type = reflect::type_info("string");
    let bytes_type = reflect::type_info("bytes");
    let char_type = reflect::type_info("char");
    let array_type = reflect::type_info("array");
    let option_type = reflect::type_info("Option");
    let result_type = reflect::type_info("Result");
    let null_value_type = reflect::type_of(null);
    let bool_value_type = reflect::type_of(true);
    let i8_type = reflect::type_info("i8");
    let i16_type = reflect::type_info("i16");
    let i32_type = reflect::type_info("i32");
    let int_value_type = reflect::type_of(42);
    let u8_type = reflect::type_info("u8");
    let u16_type = reflect::type_info("u16");
    let u32_type = reflect::type_info("u32");
    let u64_type = reflect::type_info("u64");
    let f32_type = reflect::type_info("f32");
    let float_value_type = reflect::type_of(1.5);
    let string_value_type = reflect::type_of("quest");
    let char_value_type = reflect::type_of('q');
    let array_value_type = reflect::type_of(["quest"]);
    let map_value_type = reflect::type_of({"quest": 1});
    let set_value_type = reflect::type_of(set::from_array(["quest"]));
    let option_value_type = reflect::type_of(option::some(1));
    let result_value_type = reflect::type_of(result::ok(1));
    let range_value_type = reflect::type_of(1..3);
    let option_variants = reflect::variants(option_type);
    let result_variants = reflect::variants(result_type);
    let string_methods = reflect::methods(string_type);
    let bytes_methods = reflect::methods(bytes_type);
    let char_methods = reflect::methods(char_type);
    let array_methods = reflect::methods(array_type);
    let option_methods = reflect::methods(option_type);
    let result_methods = reflect::methods(result_type);
    let map_type = reflect::type_info("map");
    let set_type = reflect::type_info("set");
    let range_type = reflect::type_info("range");
    let iterator_type = reflect::type_info("iterator");
    let map_methods = reflect::methods(map_type);
    let set_methods = reflect::methods(set_type);
    let range_methods = reflect::methods(range_type);
    let iterator_methods = reflect::methods(iterator_type);
    let trim = reflect::method(string_type, "trim");
    let split_once = reflect::method(string_type, "split_once");
    let parse_i64 = reflect::method(string_type, "parse_i64");
    let parse_char = reflect::method(string_type, "parse_char");
    let bytes_read_u32_le = reflect::method(bytes_type, "read_u32_le");
    let bytes_to_hex = reflect::method(bytes_type, "to_hex");
    let bytes_values = reflect::method(bytes_type, "values");
    let char_to_string = reflect::method(char_type, "to_string");
    let char_is_ascii_digit = reflect::method(char_type, "is_ascii_digit");
    let array_push = reflect::method(array_type, "push");
    let array_map = reflect::method(array_type, "map");
    let map_get = reflect::method(map_type, "get");
    let set_union = reflect::method(set_type, "union");
    let range_len = reflect::method(range_type, "len");
    let range_is_empty = reflect::method(range_type, "is_empty");
    let range_iter = reflect::method(range_type, "iter");
    let iterator_next = reflect::method(iterator_type, "next");
    let iterator_map = reflect::method(iterator_type, "map");
    let iterator_take = reflect::method(iterator_type, "take");
    let iterator_collect_array = reflect::method(iterator_type, "collect_array");
    let iterator_collect_set = reflect::method(iterator_type, "collect_set");
    let iterator_collect_map = reflect::method(iterator_type, "collect_map");
    let option_map = reflect::method(option_type, "map");
    let option_ok_or = reflect::method(option_type, "ok_or");
    let result_map_err = reflect::method(result_type, "map_err");
    let result_to_error = reflect::method(result_type, "to_error_option");
    let max = reflect::function("math::max");
    let sqrt = reflect::function("math::sqrt");
    let some = reflect::function("option::some");
    let ok = reflect::function("result::ok");
    let set_from_array = reflect::function("set::from_array");
    let bytes_from_hex = reflect::function("bytes::from_hex");
    let params = reflect::params(max);
    let some_params = reflect::params(some);
    let ok_params = reflect::params(ok);
    let set_params = reflect::params(set_from_array);
    let bytes_params = reflect::params(bytes_from_hex);
    let math_exports = reflect::exports(math);
    let option_exports = reflect::exports(option_module);
    let result_exports = reflect::exports(result_module);
    let set_exports = reflect::exports(set_module);
    let bytes_exports = reflect::exports(bytes_module);
    let type_of_checks = reflect::name(null_value_type) == "null"
        && reflect::kind(null_value_type) == "null"
        && reflect::name(bool_value_type) == "bool"
        && reflect::kind(bool_value_type) == "bool"
        && reflect::name(i8_type) == "i8"
        && reflect::kind(i8_type) == "i8"
        && reflect::name(i16_type) == "i16"
        && reflect::kind(i16_type) == "i16"
        && reflect::name(i32_type) == "i32"
        && reflect::kind(i32_type) == "i32"
        && reflect::name(int_value_type) == "i64"
        && reflect::kind(int_value_type) == "i64"
        && reflect::name(u8_type) == "u8"
        && reflect::kind(u8_type) == "u8"
        && reflect::name(u16_type) == "u16"
        && reflect::kind(u16_type) == "u16"
        && reflect::name(u32_type) == "u32"
        && reflect::kind(u32_type) == "u32"
        && reflect::name(u64_type) == "u64"
        && reflect::kind(u64_type) == "u64"
        && reflect::name(f32_type) == "f32"
        && reflect::kind(f32_type) == "f32"
        && reflect::name(float_value_type) == "f64"
        && reflect::kind(float_value_type) == "f64"
        && reflect::name(char_value_type) == "char"
        && reflect::kind(char_value_type) == "char"
        && reflect::name(string_value_type) == "string"
        && reflect::kind(string_value_type) == "string"
        && reflect::name(array_value_type) == "array"
        && reflect::kind(array_value_type) == "array"
        && reflect::name(map_value_type) == "map"
        && reflect::kind(map_value_type) == "map"
        && reflect::name(set_value_type) == "set"
        && reflect::kind(set_value_type) == "set"
        && reflect::name(option_value_type) == "Option"
        && reflect::kind(option_value_type) == "script_enum"
        && reflect::name(result_value_type) == "Result"
        && reflect::kind(result_value_type) == "script_enum"
        && reflect::name(range_value_type) == "range"
        && reflect::kind(range_value_type) == "range";
    return reflect::has_function("math::max")
        && reflect::has_function("math::sqrt")
        && reflect::has_function("option::some")
        && reflect::has_function("result::ok")
        && reflect::has_function("set::from_array")
        && reflect::has_function("bytes::from_hex")
        && reflect::has_type("string")
        && reflect::has_type("bytes")
        && reflect::has_type("i8")
        && reflect::has_type("i16")
        && reflect::has_type("i32")
        && reflect::has_type("i64")
        && reflect::has_type("u8")
        && reflect::has_type("u16")
        && reflect::has_type("u32")
        && reflect::has_type("u64")
        && reflect::has_type("f32")
        && reflect::has_type("f64")
        && reflect::has_type("char")
        && reflect::has_type("array")
        && reflect::has_type("map")
        && reflect::has_type("set")
        && reflect::has_type("range")
        && reflect::has_type("iterator")
        && reflect::has_type("Option")
        && reflect::has_type("Result")
        && reflect::kind(string_type) == "string"
        && reflect::kind(bytes_type) == "bytes"
        && reflect::kind(char_type) == "char"
        && reflect::kind(array_type) == "array"
        && reflect::kind(map_type) == "map"
        && reflect::kind(set_type) == "set"
        && reflect::kind(range_type) == "range"
        && reflect::kind(iterator_type) == "host"
        && reflect::kind(option_type) == "script_enum"
        && reflect::kind(result_type) == "script_enum"
        && type_of_checks
        && reflect::docs(math) == "Deterministic math standard-library helpers."
        && reflect::docs(option_module) == "Option standard-library propagation helpers."
        && reflect::docs(result_module) == "Result standard-library propagation helpers."
        && reflect::docs(set_module) == "Set standard-library construction helpers."
        && reflect::docs(bytes_module) == "Bytes standard-library conversion helpers."
        && reflect::attr(math, "stdlib") == "math"
        && reflect::attr(option_module, "stdlib") == "option"
        && reflect::attr(result_module, "stdlib") == "result"
        && reflect::attr(set_module, "stdlib") == "set"
        && reflect::attr(bytes_module, "stdlib") == "bytes"
        && reflect::attr(string_type, "stdlib") == "builtin"
        && reflect::attr(bytes_type, "stdlib") == "builtin"
        && reflect::attr(char_type, "stdlib") == "builtin"
        && reflect::attr(option_type, "stdlib") == "option"
        && reflect::attr(result_type, "stdlib") == "result"
        && string_methods.len() >= 33
        && bytes_methods.len() == 9
        && char_methods.len() == 4
        && reflect::has_method(string_type, "trim")
        && reflect::has_method(string_type, "split_once")
        && reflect::has_method(string_type, "parse_i64")
        && reflect::has_method(string_type, "parse_char")
        && reflect::has_method(bytes_type, "read_u32_le")
        && reflect::has_method(bytes_type, "to_hex")
        && reflect::has_method(bytes_type, "values")
        && reflect::has_method(char_type, "to_string")
        && reflect::has_method(char_type, "is_ascii_digit")
        && trim.owner == "string"
        && reflect::returns(trim) == "string"
        && reflect::attr(trim, "stdlib") == "string"
        && split_once.params.len() == 1
        && split_once.params[0].name == "separator"
        && split_once.params[0].type == "string"
        && reflect::returns(split_once) == "Option"
        && reflect::returns(parse_i64) == "Option"
        && reflect::returns(parse_char) == "Option"
        && bytes_read_u32_le.params[0].type == "i64"
        && reflect::returns(bytes_read_u32_le) == "u32"
        && reflect::attr(bytes_read_u32_le, "stdlib") == "bytes"
        && reflect::returns(bytes_to_hex) == "string"
        && reflect::attr(bytes_to_hex, "stdlib") == "bytes"
        && reflect::returns(bytes_values) == "iterator"
        && reflect::returns(char_to_string) == "string"
        && reflect::attr(char_to_string, "stdlib") == "char"
        && reflect::returns(char_is_ascii_digit) == "bool"
        && array_methods.len() >= 28
        && map_methods.len() >= 19
        && set_methods.len() >= 21
        && range_methods.len() == 3
        && iterator_methods.len() == 12
        && option_methods.len() >= 9
        && result_methods.len() >= 10
        && reflect::has_method(array_type, "push")
        && reflect::has_method(array_type, "map")
        && reflect::has_method(map_type, "get")
        && reflect::has_method(map_type, "map_values")
        && reflect::has_method(set_type, "union")
        && reflect::has_method(set_type, "is_subset")
        && reflect::has_method(range_type, "len")
        && reflect::has_method(range_type, "is_empty")
        && reflect::has_method(range_type, "iter")
        && reflect::has_method(iterator_type, "next")
        && reflect::has_method(iterator_type, "any")
        && reflect::has_method(iterator_type, "all")
        && reflect::has_method(iterator_type, "find")
        && reflect::has_method(iterator_type, "map")
        && reflect::has_method(iterator_type, "filter")
        && reflect::has_method(iterator_type, "take")
        && reflect::has_method(iterator_type, "skip")
        && reflect::has_method(iterator_type, "collect_array")
        && reflect::has_method(iterator_type, "collect_set")
        && reflect::has_method(iterator_type, "collect_map")
        && reflect::has_method(option_type, "map")
        && reflect::has_method(option_type, "ok_or")
        && reflect::has_method(result_type, "map_err")
        && reflect::has_method(result_type, "to_error_option")
        && array_push.params[0].name == "value"
        && array_push.params[0].type == "any"
        && reflect::returns(array_push) == "null"
        && array_map.params[0].type == "function"
        && reflect::returns(array_map) == "array"
        && map_get.params[0].name == "key"
        && reflect::returns(map_get) == "Option"
        && set_union.params[0].type == "set"
        && reflect::returns(set_union) == "set"
        && range_len.params.is_empty()
        && reflect::returns(range_len) == "i64"
        && reflect::attr(range_len, "stdlib") == "range"
        && range_is_empty.params.is_empty()
        && reflect::returns(range_is_empty) == "bool"
        && range_iter.params.is_empty()
        && reflect::returns(range_iter) == "iterator"
        && iterator_next.params.is_empty()
        && reflect::returns(iterator_next) == "Option"
        && iterator_map.params[0].name == "callback"
        && iterator_map.params[0].type == "function"
        && reflect::returns(iterator_map) == "iterator"
        && iterator_take.params[0].name == "count"
        && iterator_take.params[0].type == "i64"
        && reflect::returns(iterator_take) == "iterator"
        && iterator_collect_array.params.is_empty()
        && reflect::returns(iterator_collect_array) == "array"
        && iterator_collect_set.params.is_empty()
        && reflect::returns(iterator_collect_set) == "set"
        && iterator_collect_map.params.is_empty()
        && reflect::returns(iterator_collect_map) == "map"
        && option_map.params[0].name == "callback"
        && option_map.params[0].type == "function"
        && reflect::returns(option_map) == "Option"
        && reflect::attr(option_map, "stdlib") == "option"
        && option_ok_or.params[0].name == "error"
        && reflect::returns(option_ok_or) == "Result"
        && result_map_err.params[0].name == "callback"
        && result_map_err.params[0].type == "function"
        && reflect::returns(result_map_err) == "Result"
        && reflect::attr(result_map_err, "stdlib") == "result"
        && reflect::returns(result_to_error) == "Option"
        && option_variants.len() == 2
        && option_variants[0].name == "Some"
        && reflect::docs(option_variants[0]) == "Carries a present Option payload."
        && reflect::attr(option_variants[0], "stdlib") == "option"
        && option_variants[0].fields[0].name == "0"
        && option_variants[0].fields[0].type == "any"
        && reflect::docs(option_variants[0].fields[0]) == "Dynamic Option::Some payload value."
        && reflect::attr(option_variants[0].fields[0], "stdlib") == "option"
        && option_variants[1].name == "None"
        && reflect::docs(option_variants[1]) == "Represents expected absence without a payload."
        && reflect::attr(option_variants[1], "stdlib") == "option"
        && result_variants.len() == 2
        && result_variants[0].name == "Ok"
        && reflect::docs(result_variants[0]) == "Carries a successful Result payload."
        && reflect::attr(result_variants[0], "stdlib") == "result"
        && result_variants[0].fields[0].type == "any"
        && reflect::docs(result_variants[0].fields[0]) == "Dynamic Result::Ok payload value."
        && reflect::attr(result_variants[0].fields[0], "stdlib") == "result"
        && result_variants[1].name == "Err"
        && reflect::docs(result_variants[1]) == "Carries a recoverable Result error payload."
        && reflect::attr(result_variants[1], "stdlib") == "result"
        && result_variants[1].fields[0].type == "any"
        && reflect::docs(result_variants[1].fields[0]) == "Dynamic Result::Err payload value."
        && reflect::attr(result_variants[1].fields[0], "stdlib") == "result"
        && !reflect::has_function("math::random")
        && math_exports.len() == 14
        && math_exports.contains("math::max")
        && math_exports.contains("math::sqrt")
        && option_exports.len() == 7
        && option_exports.contains("option::some")
        && option_exports.contains("option::unwrap_or")
        && result_exports.len() == 8
        && result_exports.contains("result::ok")
        && result_exports.contains("result::to_option")
        && set_exports.len() == 1
        && set_exports.contains("set::from_array")
        && bytes_exports.len() == 1
        && bytes_exports.contains("bytes::from_hex")
        && reflect::attr(max, "stdlib") == "math"
        && reflect::attr(some, "stdlib") == "option"
        && reflect::attr(ok, "stdlib") == "result"
        && reflect::attr(set_from_array, "stdlib") == "set"
        && reflect::attr(bytes_from_hex, "stdlib") == "bytes"
        && reflect::docs(max) == "Returns the larger numeric value."
        && reflect::docs(sqrt) == "Returns the square root as a float."
        && reflect::docs(some) == "Wraps a value in Option::Some."
        && reflect::docs(ok) == "Wraps a success value in Result::Ok."
        && reflect::docs(set_from_array) == "Builds a set from array values."
        && reflect::docs(bytes_from_hex) == "Decodes hexadecimal text to bytes or returns an error string."
        && reflect::returns(max) == "any"
        && reflect::returns(sqrt) == "f64"
        && reflect::returns(some) == "any"
        && reflect::returns(ok) == "any"
        && reflect::returns(set_from_array) == "set"
        && reflect::returns(bytes_from_hex) == "Result"
        && params.len() == 2
        && params[0].name == "left"
        && params[1].name == "right"
        && some_params.len() == 1
        && some_params[0].name == "value"
        && ok_params.len() == 1
        && ok_params[0].name == "value"
        && set_params.len() == 1
        && set_params[0].name == "values"
        && set_params[0].type == "array"
        && bytes_params.len() == 1
        && bytes_params[0].name == "text"
        && bytes_params[0].type == "string";
}
"#,
        )
        .expect("program should compile");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        run_linked_program_with_host(&engine, &program, &[], &mut host),
        Ok(OwnedValue::Bool(true))
    );
}
