use vela_bytecode::compiler::compile_program_source;
use vela_common::{FieldId, HostMethodId, HostObjectId, HostTypeId, SourceId, Span, TypeId};
use vela_host::{HostPath, HostRef, HostValue, MockStateAdapter, PatchOp, PatchTx};
use vela_hot_reload::{FunctionAbi, MethodAbi};
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
    assert_eq!(
        function.access.required_permissions(),
        &["game.add".to_owned()]
    );
    assert_eq!(function.docs.as_deref(), Some("Adds two integers."));
    assert_eq!(function.attrs.get("domain"), Some("gameplay"));
    assert_eq!(function.attrs.get("stable"), Some("true"));
    assert_eq!(function.source_span, Some(source_span));

    let function_abi = FunctionAbi::from_function(function);
    assert_eq!(function_abi.source_span, Some(source_span));
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
