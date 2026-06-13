use super::standard_id_dispatch::{
    RecordingNativeCaches, native_cache_code, run_linked_standard_id_code,
    run_linked_standard_id_code_with_caches,
};
use super::*;

fn borrowed_native_arg_code(name: &str, native_id: FunctionId) -> UnlinkedCodeObject {
    let mut code = UnlinkedCodeObject::new(name, 2);
    let value = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(41)));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: value,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallNative {
            dst: Some(Register(1)),
            name: "diagnostic_name".into(),
            native: native_id,
            cache_site: None,
            args: vec![Register(0)],
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(1),
    }));
    code
}

#[test]
fn linked_borrowed_native_call_receives_runtime_values() {
    let native_id = vela_def::FunctionId::new(79);
    let mut vm = Vm::new();
    vm.register_borrowed_native_with_id(native_id, |args, _heap, _budget| {
        let [Value::Scalar(vela_common::ScalarValue::I64(value))] = args else {
            return Ok(OwnedValue::Null);
        };
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(
            *value + 1,
        )))
    });

    assert_eq!(
        run_linked_standard_id_code(&vm, borrowed_native_arg_code("borrowed_native", native_id)),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(42)))
    );
}

#[test]
fn linked_borrowed_native_call_inline_cache_populates_and_reuses_resolved_target() {
    let native_id = vela_def::FunctionId::new(80);
    let mut vm = Vm::new();
    vm.register_borrowed_native_with_id(native_id, |args, _heap, _budget| {
        assert!(args.is_empty());
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(4)))
    });
    let (code, cache_site) = native_cache_code("borrowed_native_cache", native_id);
    let caches = RecordingNativeCaches::new(1);

    assert_eq!(
        run_linked_standard_id_code_with_caches(&vm, code.clone(), &caches),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(4)))
    );
    assert_eq!(caches.set_count(), 1);
    assert_eq!(
        caches
            .entry(cache_site)
            .expect("borrowed native cache should populate")
            .native_id(),
        native_id
    );

    assert_eq!(
        run_linked_standard_id_code_with_caches(&vm, code, &caches),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(4)))
    );
    assert_eq!(caches.set_count(), 1);
}
