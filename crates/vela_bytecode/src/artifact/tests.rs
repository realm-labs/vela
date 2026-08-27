use vela_common::StateSlot;

use crate::{
    CacheSiteKind, FunctionIndex, InstructionOffset, Linker, Register, UnlinkedCodeObject,
    UnlinkedInstruction, UnlinkedInstructionKind, UnlinkedProgram,
};

#[test]
fn nested_local_cache_zero_sites_receive_distinct_generation_ids() {
    let first = cached_state_lambda("first", "main::first", StateSlot::new(0));
    let second = cached_state_lambda("second", "main::second", StateSlot::new(1));
    let mut main = UnlinkedCodeObject::new("main", 2);
    main.nested_functions = vec![first, second];
    main.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::MakeClosure {
            dst: Register(0),
            function: FunctionIndex(0),
            captures: Vec::new(),
        },
    ));
    main.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::MakeClosure {
            dst: Register(1),
            function: FunctionIndex(1),
            captures: Vec::new(),
        },
    ));
    main.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(1),
    }));

    let mut program = UnlinkedProgram::new();
    program.set_states([
        crate::StateDescriptor::test_extern(vela_def::StateId::new(1), "main::first"),
        crate::StateDescriptor::test_extern(vela_def::StateId::new(2), "main::second"),
    ]);
    program.insert_function(main);
    let artifact = Linker::new()
        .link_test_program(&program)
        .expect("artifact should link");

    let first = artifact
        .function(crate::ScriptFunctionHandle::new(1))
        .expect("first lambda");
    let second = artifact
        .function(crate::ScriptFunctionHandle::new(2))
        .expect("second lambda");
    let first_site = first.cache_sites.sites()[0].id;
    let second_site = second.cache_sites.sites()[0].id;
    assert_ne!(first_site, second_site);
    assert_eq!(artifact.cache_layout().len(), 2);
    assert_eq!(artifact.profile_layout().functions().len(), 3);
}

fn cached_state_lambda(name: &str, state: &str, slot: StateSlot) -> UnlinkedCodeObject {
    let mut code = UnlinkedCodeObject::new(name, 1);
    let site = code.push_cache_site(CacheSiteKind::ExternStateRead, InstructionOffset(0));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadExternState {
            dst: Register(0),
            state: state.to_owned(),
            slot: Some(slot),
            cache_site: Some(site),
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(0),
    }));
    code
}
