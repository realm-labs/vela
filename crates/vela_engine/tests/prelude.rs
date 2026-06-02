use vela_bytecode::compiler::compile_program_source_with_options;
use vela_engine::prelude::*;
use vela_host::mock::MockStateAdapter;
use vela_host::patch::PatchOp;

#[test]
fn prelude_imports_cover_runtime_embedding_flow() {
    let method = HostMethodId::new(23);
    let engine = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .host_type(HostTypeId::new(1))
                .method(MethodDesc::new(method, "grant_exp")),
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player: Player, amount: int) {
    player.grant_exp(amount);
    return amount;
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let args = args![host!(1, 42, 1), 12];

    let result = runtime
        .call(
            "main",
            &args,
            CallOptions::gameplay(),
            &mut adapter,
            &mut tx,
        )
        .expect("runtime call should run");

    assert_eq!(result, Value::Int(12));
    assert_eq!(
        tx.patches()[0].path,
        HostPath::new(HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1)),
    );
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::Int(12)],
        },
    );
}
