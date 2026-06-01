use vela_bytecode::compiler::compile_program_source;
use vela_common::{FieldId, HostMethodId, HostObjectId, HostTypeId, SourceId, Span, TypeId};
use vela_host::{HostPath, HostRef, HostValue, MockStateAdapter, PatchOp, PatchTx};
use vela_hot_reload::{AccessAbi, FunctionAbi, MethodAbi};
use vela_reflect::{
    FieldAccess, FieldDesc, ModuleDesc, ReflectErrorKind, ReflectPermission, ReflectPermissionSet,
    TypeDesc, TypeKey,
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

    let array_type = registry.type_by_name("array").expect("array type");
    assert_eq!(array_type.kind, vela_reflect::TypeKind::Array);
    assert_eq!(array_type.attrs.get("stdlib"), Some("builtin"));

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

    let math = registry.module_by_name("math").expect("math module");
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
    let option = reflect.module("option");
    let result = reflect.module("result");
    let set = reflect.module("set");
    let string_type = reflect.type_info("string");
    let array_type = reflect.type_info("array");
    let option_type = reflect.type_info("Option");
    let result_type = reflect.type_info("Result");
    let option_variants = reflect.variants(option_type);
    let result_variants = reflect.variants(result_type);
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
    let option_exports = reflect.exports(option);
    let result_exports = reflect.exports(result);
    let set_exports = reflect.exports(set);
    return reflect.has_function("math.max")
        && reflect.has_function("math.sqrt")
        && reflect.has_function("option.some")
        && reflect.has_function("result.ok")
        && reflect.has_function("set.from_array")
        && reflect.has_type("string")
        && reflect.has_type("array")
        && reflect.has_type("Option")
        && reflect.has_type("Result")
        && reflect.kind(string_type) == "string"
        && reflect.kind(array_type) == "array"
        && reflect.kind(option_type) == "script_enum"
        && reflect.kind(result_type) == "script_enum"
        && reflect.attr(string_type, "stdlib") == "builtin"
        && reflect.attr(option_type, "stdlib") == "option"
        && reflect.attr(result_type, "stdlib") == "result"
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
