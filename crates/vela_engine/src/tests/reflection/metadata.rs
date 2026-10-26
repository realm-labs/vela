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
        .link_test_program(program)
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
        .register_type_desc(ReflectOnlyPlayer::script_reflect_type_desc())
        .build()
        .expect("engine should build with reflect schema");

    let registry = engine.registry();
    let reflected = registry
        .type_by_name("ReflectOnlyPlayer")
        .expect("reflect schema should be registered");
    assert_eq!(reflected.key.id, TypeId::new(9901));
    assert_eq!(reflected.fields[0].name, "level");
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

    let mut host = HostExecution { state_values: None };

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

    let mut host = HostExecution { state_values: None };

    assert_eq!(
        run_linked_program_with_host(&engine, &program, &[], &mut host),
        Ok(OwnedValue::Bool(true))
    );
}

#[test]
fn engine_reflection_type_descriptors_expose_tuple_payloads() {
    let engine = Engine::builder()
        .with_standard_natives()
        .reflection_permissions(ReflectPermissionSet::new().with(ReflectPermission::ReadTypeInfo))
        .build()
        .expect("engine should build with standard natives");

    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            include_str!("../../../tests/fixtures/reflection/tuple_descriptors.vela"),
        )
        .expect("program should compile");

    let mut host = HostExecution { state_values: None };

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
    assert_eq!(trim.return_type.as_deref(), Some("String"));
    assert_eq!(trim.attrs.get("stdlib"), Some("string"));
    let split_once = string_type
        .methods
        .iter()
        .find(|method| method.name == "split_once")
        .expect("string.split_once method metadata");
    assert_eq!(split_once.params[0].name, "separator");
    assert_eq!(split_once.params[0].type_hint.as_deref(), Some("String"));
    assert_eq!(
        split_once.return_type.as_deref(),
        Some("Option<(String, String)>")
    );
    let parse_i64 = string_type
        .methods
        .iter()
        .find(|method| method.name == "parse_i64")
        .expect("string.parse_i64 method metadata");
    assert_eq!(parse_i64.return_type.as_deref(), Some("Option<i64>"));
    let parse_char = string_type
        .methods
        .iter()
        .find(|method| method.name == "parse_char")
        .expect("string.parse_char method metadata");
    assert_eq!(parse_char.return_type.as_deref(), Some("Option<char>"));

    let array_type = registry.type_by_name("array").expect("array type");
    assert_eq!(array_type.kind, vela_reflect::registry::TypeKind::Array);
    assert_eq!(array_type.attrs.get("stdlib"), Some("builtin"));
    let array_push = array_type
        .methods
        .iter()
        .find(|method| method.name == "push")
        .expect("array.push method metadata");
    assert_eq!(array_push.params[0].type_hint.as_deref(), Some("Any"));
    assert_eq!(array_push.return_type.as_deref(), Some("()"));
    let array_map = array_type
        .methods
        .iter()
        .find(|method| method.name == "map")
        .expect("array.map method metadata");
    assert_eq!(array_map.params[0].type_hint.as_deref(), Some("Function"));
    assert_eq!(array_map.return_type.as_deref(), Some("Array"));
    let array_index_of = array_type
        .methods
        .iter()
        .find(|method| method.name == "index_of")
        .expect("array.index_of method metadata");
    assert_eq!(array_index_of.return_type.as_deref(), Some("Option<i64>"));
    let array_pop = array_type
        .methods
        .iter()
        .find(|method| method.name == "pop")
        .expect("array.pop method metadata");
    assert_eq!(array_pop.return_type.as_deref(), Some("Option<Any>"));

    let map_type = registry.type_by_name("map").expect("map type");
    assert_eq!(map_type.kind, vela_reflect::registry::TypeKind::Map);
    let map_get = map_type
        .methods
        .iter()
        .find(|method| method.name == "get")
        .expect("map.get method metadata");
    assert_eq!(map_get.params[0].name, "key");
    assert_eq!(map_get.return_type.as_deref(), Some("Option<Any>"));

    let set_type = registry.type_by_name("set").expect("set type");
    assert_eq!(set_type.kind, vela_reflect::registry::TypeKind::Set);
    let set_union = set_type
        .methods
        .iter()
        .find(|method| method.name == "union")
        .expect("set::union method metadata");
    assert_eq!(set_union.params[0].type_hint.as_deref(), Some("Set"));
    assert_eq!(set_union.return_type.as_deref(), Some("Set"));

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

    let iterator_type = registry.type_by_name("iterator").expect("iterator type");
    let iterator_next = iterator_type
        .methods
        .iter()
        .find(|method| method.name == "next")
        .expect("iterator.next method metadata");
    assert_eq!(iterator_next.return_type.as_deref(), Some("Option<Any>"));

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
        Some("Any")
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
    assert_eq!(option_map.params[0].type_hint.as_deref(), Some("Function"));
    assert_eq!(option_map.return_type.as_deref(), Some("Option<Any>"));
    assert_eq!(option_map.attrs.get("stdlib"), Some("option"));
    let option_ok_or = option_type
        .methods
        .iter()
        .find(|method| method.name == "ok_or")
        .expect("Option.ok_or method metadata");
    assert_eq!(option_ok_or.params[0].name, "error");
    assert_eq!(
        option_ok_or.return_type.as_deref(),
        Some("Result<Any, Any>")
    );

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
        Some("Any")
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
        Some("Any")
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
        Some("Function")
    );
    assert_eq!(
        result_map_err.return_type.as_deref(),
        Some("Result<Any, Any>")
    );
    assert_eq!(result_map_err.attrs.get("stdlib"), Some("result"));
    let result_to_error = result_type
        .methods
        .iter()
        .find(|method| method.name == "to_error_option")
        .expect("Result.to_error_option method metadata");
    assert_eq!(result_to_error.return_type.as_deref(), Some("Option<Any>"));

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
    assert_eq!(max.return_type.as_deref(), Some("Any"));
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
    assert_eq!(option_some.return_type.as_deref(), Some("Any"));
    assert_eq!(option_some.attrs.get("stdlib"), Some("option"));
    assert_eq!(
        option_some.docs.as_deref(),
        Some("Wraps a value in Option::Some.")
    );

    let result_ok = registry.function_by_name("result::ok").expect("result::ok");
    assert_eq!(result_ok.module.as_deref(), Some("result"));
    assert_eq!(result_ok.params[0].name, "value");
    assert_eq!(result_ok.return_type.as_deref(), Some("Any"));
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
    assert_eq!(set_from_array.params[0].type_hint.as_deref(), Some("Array"));
    assert_eq!(set_from_array.return_type.as_deref(), Some("Set"));
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
        Some("String")
    );
    assert_eq!(
        bytes_from_hex.return_type.as_deref(),
        Some("Result<Bytes, String>")
    );
    assert_eq!(bytes_from_hex.attrs.get("stdlib"), Some("bytes"));

    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            include_str!("../../../tests/fixtures/reflection/standard_metadata.vela"),
        )
        .expect("program should compile");

    let mut host = HostExecution { state_values: None };

    assert_eq!(
        run_linked_program_with_host(&engine, &program, &[], &mut host),
        Ok(OwnedValue::Bool(true))
    );
}
