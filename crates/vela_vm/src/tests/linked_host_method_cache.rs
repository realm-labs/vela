use super::linked_standard_method_cache_support::RecordingMethodCaches;
use super::*;

#[test]
fn linked_host_method_cache_misses_wrong_method_target_guard() {
    let host_ref = player_ref(3);
    let method_id = HostMethodId::new(8);
    let stale_method_id = HostMethodId::new(9);
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let player_name = program.intern_debug_name("player");
    let method_name = program.intern_debug_name("debug_only_host_method");
    let method = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Host { method_id },
    ));

    let mut code =
        vela_bytecode::LinkedCodeObject::new(main_name, 3).with_params(vec![player_name]);
    let amount = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(20)));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(1),
            constant: amount,
        },
    ));
    let cache_site = code.push_cache_site(CacheSiteKind::MethodCall, InstructionOffset(1));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst: Register(2),
            receiver: Register(0),
            dispatch: method,
            debug_name: method_name,
            cache_site: Some(cache_site),
            args: vec![vela_bytecode::CallArgument::Register(Register(1))],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(2) },
    ));
    let main = program.push_function(code);
    program.set_entry_point(main_name, main);

    let caches = RecordingMethodCaches::new(1);
    caches.prime(
        cache_site,
        MethodInlineCacheEntry {
            dispatch: method,
            debug_name: method_name,
            target: MethodInlineCacheTarget::Host {
                method_id: stale_method_id,
            },
        },
    );
    let mut adapter = host_adapter(
        host_ref,
        HostValue::Scalar(vela_common::ScalarValue::I64(9)),
    );
    adapter.insert_method_return(
        method_id,
        HostValue::Scalar(vela_common::ScalarValue::I64(12)),
    );
    adapter.insert_method_return(
        stale_method_id,
        HostValue::Scalar(vela_common::ScalarValue::I64(99)),
    );
    let mut access = HostAccess::new();
    let mut budget = ExecutionBudget::unbounded();
    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut access,
            script_globals: None,
        };
        let code = program.function(main).expect("main linked code exists");
        Vm::new().execute_linked_call(
            crate::linked_execution::LinkedExecutionCall {
                code,
                program: &program,
                captures: &[],
                args: &[Value::HostRef(host_ref)],
                check_param_guards: true,
                call_site: None,
                call_site_offset: None,
                inline_caches: Some(&caches),
                bytecode_profiler: None,
            },
            Some(&mut host),
            None,
            Some(&mut budget),
        )
    };

    assert_eq!(result, Ok(Value::Scalar(vela_common::ScalarValue::I64(12))));
    assert_eq!(
        adapter.method_calls(),
        &[(
            HostPath::new(host_ref),
            method_id,
            vec![HostValue::Scalar(vela_common::ScalarValue::I64(20))]
        )]
    );
    assert_eq!(caches.set_count_for(cache_site), 1);
    assert_eq!(
        caches.entry(cache_site),
        Some(MethodInlineCacheEntry {
            dispatch: method,
            debug_name: method_name,
            target: MethodInlineCacheTarget::Host { method_id },
        })
    );
}
