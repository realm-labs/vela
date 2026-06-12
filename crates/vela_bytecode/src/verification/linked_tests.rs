use vela_registry::DebugNameId;

use crate::{
    CacheSiteDesc, CacheSiteId, CacheSiteKind, CacheSiteLayout, FieldSlot, Instruction,
    InstructionKind, InstructionOffset, LinkedCodeObject, NativeHandle, Register,
};

use super::*;

fn linked_native_call_code() -> LinkedCodeObject {
    let mut code = LinkedCodeObject::new(DebugNameId::new(0), 1);
    code.push_instruction(Instruction::new(InstructionKind::CallNative {
        dst: None,
        native: NativeHandle::new(0),
        debug_name: DebugNameId::new(0),
        cache_site: None,
        args: Vec::new(),
    }));
    code
}

#[test]
fn linked_code_rejects_cache_site_layout_id_mismatch() {
    let mut code = linked_native_call_code();
    code.cache_sites = CacheSiteLayout::new(vec![CacheSiteDesc::new(
        CacheSiteId::new(3),
        CacheSiteKind::NativeCall,
        "<linked>",
        InstructionOffset(0),
    )]);

    assert_eq!(
        verify_linked_code_object(&code),
        Err(error(
            "<linked code>",
            None,
            VerificationErrorKind::CacheSiteIdMismatch {
                expected: CacheSiteId::new(0),
                actual: CacheSiteId::new(3)
            }
        ))
    );
}

#[test]
fn linked_code_rejects_cache_site_layout_instruction_offset_out_of_bounds() {
    let mut code = linked_native_call_code();
    code.cache_sites = CacheSiteLayout::new(vec![CacheSiteDesc::new(
        CacheSiteId::new(0),
        CacheSiteKind::NativeCall,
        "<linked>",
        InstructionOffset(9),
    )]);

    assert_eq!(
        verify_linked_code_object(&code),
        Err(error(
            "<linked code>",
            None,
            VerificationErrorKind::InstructionOutOfBounds {
                target: InstructionOffset(9),
                instruction_count: 1
            }
        ))
    );
}

#[test]
fn linked_code_rejects_cache_site_layout_instruction_kind_mismatch_for_sidecar_only_sites() {
    let mut code = linked_native_call_code();
    code.cache_sites = CacheSiteLayout::new(vec![CacheSiteDesc::new(
        CacheSiteId::new(0),
        CacheSiteKind::MethodCall,
        "<linked>",
        InstructionOffset(0),
    )]);

    assert_eq!(
        verify_linked_code_object(&code),
        Err(error(
            "<linked code>",
            Some(0),
            VerificationErrorKind::CacheSiteInstructionKindMismatch {
                site: CacheSiteId::new(0),
                expected: CacheSiteKind::MethodCall,
                actual: Some(CacheSiteKind::NativeCall)
            }
        ))
    );
}

#[test]
fn linked_code_rejects_cache_site_layout_on_uncacheable_instruction() {
    let mut code = LinkedCodeObject::new(DebugNameId::new(0), 1);
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(0),
    }));
    code.cache_sites = CacheSiteLayout::new(vec![CacheSiteDesc::new(
        CacheSiteId::new(0),
        CacheSiteKind::NativeCall,
        "<linked>",
        InstructionOffset(0),
    )]);

    assert_eq!(
        verify_linked_code_object(&code),
        Err(error(
            "<linked code>",
            Some(0),
            VerificationErrorKind::CacheSiteInstructionKindMismatch {
                site: CacheSiteId::new(0),
                expected: CacheSiteKind::NativeCall,
                actual: None
            }
        ))
    );
}

#[test]
fn linked_code_rejects_invalid_record_cache_site_operands() {
    let mut native_code = linked_native_call_code();
    let native_site = native_code.push_cache_site(CacheSiteKind::MethodCall, InstructionOffset(0));
    if let InstructionKind::CallNative { cache_site, .. } = &mut native_code.instructions[0].kind {
        *cache_site = Some(native_site);
    }
    assert_eq!(
        verify_linked_code_object(&native_code),
        Err(error(
            "<linked code>",
            Some(0),
            VerificationErrorKind::CacheSiteKindMismatch {
                site: native_site,
                expected: CacheSiteKind::NativeCall,
                actual: CacheSiteKind::MethodCall,
            }
        ))
    );

    let mut read_code = LinkedCodeObject::new(DebugNameId::new(0), 2);
    let read_site =
        read_code.push_cache_site(CacheSiteKind::RecordFieldWrite, InstructionOffset(0));
    read_code.push_instruction(Instruction::new(InstructionKind::GetRecordSlot {
        dst: Register(0),
        record: Register(1),
        field: FieldSlot::new(0),
        debug_name: DebugNameId::new(0),
        cache_site: Some(read_site),
    }));
    assert_eq!(
        verify_linked_code_object(&read_code),
        Err(error(
            "<linked code>",
            Some(0),
            VerificationErrorKind::CacheSiteKindMismatch {
                site: read_site,
                expected: CacheSiteKind::RecordFieldRead,
                actual: CacheSiteKind::RecordFieldWrite,
            }
        ))
    );

    let mut write_code = LinkedCodeObject::new(DebugNameId::new(0), 2);
    let write_site =
        write_code.push_cache_site(CacheSiteKind::RecordFieldRead, InstructionOffset(0));
    write_code.push_instruction(Instruction::new(InstructionKind::SetRecordSlot {
        record: Register(0),
        field: FieldSlot::new(0),
        debug_name: DebugNameId::new(0),
        cache_site: Some(write_site),
        src: Register(1),
    }));
    assert_eq!(
        verify_linked_code_object(&write_code),
        Err(error(
            "<linked code>",
            Some(0),
            VerificationErrorKind::CacheSiteKindMismatch {
                site: write_site,
                expected: CacheSiteKind::RecordFieldWrite,
                actual: CacheSiteKind::RecordFieldRead,
            }
        ))
    );
}
