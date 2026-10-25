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
        .expect("engine reflection native test program should link");
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
fn engine_builder_registers_module_reflection_metadata() {
    let engine = Engine::builder()
        .register_module(
            ModuleDesc::new("game::reward")
                .docs("Reward module.")
                .attr("domain", "gameplay"),
        )
        .register_native_fn(
            NativeFunctionDesc::new("game::reward::grant", NativeFunctionId::new(221))
                .returns(TypeHint::boolean()),
            |_| Ok(OwnedValue::Bool(true)),
        )
        .build()
        .expect("engine should build");

    let registry = engine.registry();
    let module = registry
        .module_by_name("game::reward")
        .expect("registered module metadata");
    assert_eq!(module.docs.as_deref(), Some("Reward module."));
    assert_eq!(module.attrs.get("domain"), Some("gameplay"));
    assert_eq!(module.exports.len(), 1);
    assert_eq!(module.exports[0].name, "game::reward::grant");
}

#[test]
fn public_reflection_metadata_lists_do_not_need_engine_permissions() {
    let engine = Engine::builder()
        .register_type_desc(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .field(FieldDesc::new(FieldId::new(1), "secret_level")),
        )
        .register_native_fn(
            NativeFunctionDesc::new("game::secret_bonus", NativeFunctionId::new(77))
                .returns(TypeHint::i64())
                .access(FunctionAccess::public().reflect_callable(true)),
            |_| Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(5))),
        )
        .reflection_permissions(ReflectPermissionSet::new().with(ReflectPermission::ReadTypeInfo))
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
fn main() {
    let fields: Array = reflect::fields();
    let functions: Array = reflect::functions();
    if fields[0].owner == "Player"
        && fields[0].name == "secret_level"
        && functions[0].name == "game::secret_bonus" {
        return 1;
    }
    return 0;
}
"#,
        )
        .expect("program should compile");

    let mut host = HostExecution { state_values: None };

    assert_eq!(
        run_linked_program_with_host(&engine, &program, &[], &mut host),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn engine_missing_permissions_hide_reflection_metadata_lists() {
    let engine = Engine::builder()
        .register_type_desc(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player")).field(
                FieldDesc::new(FieldId::new(1), "secret_level")
                    .access(FieldAccess::new().require_permission("player.inspect")),
            ),
        )
        .register_native_fn(
            NativeFunctionDesc::new("game::secret_bonus", NativeFunctionId::new(77))
                .returns(TypeHint::i64())
                .access(FunctionAccess::public().reflect_callable(true)),
            |_| Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(5))),
        )
        .reflection_permissions(ReflectPermissionSet::new().with(ReflectPermission::ReadTypeInfo))
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
fn main() {
    let fields: Array = reflect::fields();
    let functions: Array = reflect::functions();
    if functions[0].name == "game::secret_bonus" {
        return 1;
    }
    return 0;
}
"#,
        )
        .expect("program should compile");

    let mut host = HostExecution { state_values: None };

    assert_eq!(
        run_linked_program_with_host(&engine, &program, &[], &mut host),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}
