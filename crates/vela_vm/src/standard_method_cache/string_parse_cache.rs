use vela_def::MethodId;

use crate::std_method_ids::{StdMethodIds, std_method_ids};
use crate::{
    ExecutionBudget, HeapExecution, StandardMethodInlineCacheTarget, Value, VmResult,
    string_methods,
};

pub(super) fn target_matches_method_id(
    target: StandardMethodInlineCacheTarget,
    method_id: MethodId,
) -> bool {
    let ids = std_method_ids();
    match target {
        StandardMethodInlineCacheTarget::ParseI8 => method_id == ids.string_parse_i8,
        StandardMethodInlineCacheTarget::ParseI16 => method_id == ids.string_parse_i16,
        StandardMethodInlineCacheTarget::ParseI32 => method_id == ids.string_parse_i32,
        StandardMethodInlineCacheTarget::ParseI64 => method_id == ids.string_parse_i64,
        StandardMethodInlineCacheTarget::ParseU8 => method_id == ids.string_parse_u8,
        StandardMethodInlineCacheTarget::ParseU16 => method_id == ids.string_parse_u16,
        StandardMethodInlineCacheTarget::ParseU32 => method_id == ids.string_parse_u32,
        StandardMethodInlineCacheTarget::ParseU64 => method_id == ids.string_parse_u64,
        StandardMethodInlineCacheTarget::ParseF32 => method_id == ids.string_parse_f32,
        StandardMethodInlineCacheTarget::ParseF64 => method_id == ids.string_parse_f64,
        StandardMethodInlineCacheTarget::ParseBool => method_id == ids.string_parse_bool,
        StandardMethodInlineCacheTarget::ParseChar => method_id == ids.string_parse_char,
        _ => false,
    }
}

pub(super) fn target_for_method_id(
    method_id: MethodId,
    ids: &StdMethodIds,
) -> Option<StandardMethodInlineCacheTarget> {
    Some(match method_id {
        id if id == ids.string_parse_i8 => StandardMethodInlineCacheTarget::ParseI8,
        id if id == ids.string_parse_i16 => StandardMethodInlineCacheTarget::ParseI16,
        id if id == ids.string_parse_i32 => StandardMethodInlineCacheTarget::ParseI32,
        id if id == ids.string_parse_i64 => StandardMethodInlineCacheTarget::ParseI64,
        id if id == ids.string_parse_u8 => StandardMethodInlineCacheTarget::ParseU8,
        id if id == ids.string_parse_u16 => StandardMethodInlineCacheTarget::ParseU16,
        id if id == ids.string_parse_u32 => StandardMethodInlineCacheTarget::ParseU32,
        id if id == ids.string_parse_u64 => StandardMethodInlineCacheTarget::ParseU64,
        id if id == ids.string_parse_f32 => StandardMethodInlineCacheTarget::ParseF32,
        id if id == ids.string_parse_f64 => StandardMethodInlineCacheTarget::ParseF64,
        id if id == ids.string_parse_bool => StandardMethodInlineCacheTarget::ParseBool,
        id if id == ids.string_parse_char => StandardMethodInlineCacheTarget::ParseChar,
        _ => return None,
    })
}

pub(super) fn is_parse_target(target: StandardMethodInlineCacheTarget) -> bool {
    matches!(
        target,
        StandardMethodInlineCacheTarget::ParseI8
            | StandardMethodInlineCacheTarget::ParseI16
            | StandardMethodInlineCacheTarget::ParseI32
            | StandardMethodInlineCacheTarget::ParseI64
            | StandardMethodInlineCacheTarget::ParseU8
            | StandardMethodInlineCacheTarget::ParseU16
            | StandardMethodInlineCacheTarget::ParseU32
            | StandardMethodInlineCacheTarget::ParseU64
            | StandardMethodInlineCacheTarget::ParseF32
            | StandardMethodInlineCacheTarget::ParseF64
            | StandardMethodInlineCacheTarget::ParseBool
            | StandardMethodInlineCacheTarget::ParseChar
    )
}

pub(super) fn call_parse_method(
    target: StandardMethodInlineCacheTarget,
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    Some(match target {
        StandardMethodInlineCacheTarget::ParseI8 => {
            string_methods::parse_i8(receiver, args, heap, budget)
        }
        StandardMethodInlineCacheTarget::ParseI16 => {
            string_methods::parse_i16(receiver, args, heap, budget)
        }
        StandardMethodInlineCacheTarget::ParseI32 => {
            string_methods::parse_i32(receiver, args, heap, budget)
        }
        StandardMethodInlineCacheTarget::ParseI64 => {
            string_methods::parse_i64(receiver, args, heap, budget)
        }
        StandardMethodInlineCacheTarget::ParseU8 => {
            string_methods::parse_u8(receiver, args, heap, budget)
        }
        StandardMethodInlineCacheTarget::ParseU16 => {
            string_methods::parse_u16(receiver, args, heap, budget)
        }
        StandardMethodInlineCacheTarget::ParseU32 => {
            string_methods::parse_u32(receiver, args, heap, budget)
        }
        StandardMethodInlineCacheTarget::ParseU64 => {
            string_methods::parse_u64(receiver, args, heap, budget)
        }
        StandardMethodInlineCacheTarget::ParseF32 => {
            string_methods::parse_f32(receiver, args, heap, budget)
        }
        StandardMethodInlineCacheTarget::ParseF64 => {
            string_methods::parse_f64(receiver, args, heap, budget)
        }
        StandardMethodInlineCacheTarget::ParseBool => {
            string_methods::parse_bool(receiver, args, heap, budget)
        }
        StandardMethodInlineCacheTarget::ParseChar => {
            string_methods::parse_char(receiver, args, heap, budget)
        }
        _ => return None,
    })
}
