use std::collections::BTreeMap;

use vela_common::HostMethodId;
use vela_def::{FunctionId, MethodId, TypeId, VariantId};

use crate::{
    CacheSiteId, CacheSiteInstruction, HostTargetPlanId, InstructionOffset, LinkedCodeObject,
    LinkedProgram, MethodDispatchHandle, NativeHandle, ScriptFunctionHandle, TypeHandle,
    UnlinkedCodeObject, VariantHandle,
};

pub(super) struct LinkContext<'linker, 'registry> {
    pub(super) linker: &'linker super::Linker<'registry>,
    pub(super) linked: LinkedProgram,
    pub(super) script_functions_by_name: BTreeMap<String, ScriptFunctionHandle>,
    pub(super) script_functions_by_id: BTreeMap<FunctionId, ScriptFunctionHandle>,
    pub(super) script_methods_by_id: BTreeMap<MethodId, ScriptFunctionHandle>,
    pub(super) native_handles: BTreeMap<FunctionId, NativeHandle>,
    pub(super) method_handles: BTreeMap<MethodDispatchKey, MethodDispatchHandle>,
    pub(super) type_handles: BTreeMap<TypeId, TypeHandle>,
    pub(super) variant_handles: BTreeMap<VariantId, VariantHandle>,
}

pub(super) struct LinkInstructionContext<'a> {
    pub(super) program: &'a crate::ProgramImage,
    pub(super) code: &'a UnlinkedCodeObject,
    pub(super) host_target_map: &'a [HostTargetPlanId],
    pub(super) linked_code: &'a mut LinkedCodeObject,
    pub(super) instruction_offset: InstructionOffset,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum MethodDispatchKey {
    Script(MethodId, ScriptFunctionHandle),
    Value(MethodId),
    Host(HostMethodId),
}

pub(super) fn cache_site_at(
    code: &UnlinkedCodeObject,
    instruction_offset: InstructionOffset,
) -> Option<CacheSiteId> {
    let kind = code
        .instructions
        .get(instruction_offset.0)?
        .kind
        .cache_site_policy()?
        .kind;
    code.cache_sites
        .sites()
        .iter()
        .find(|site| site.instruction_offset == instruction_offset && site.kind == kind)
        .map(|site| site.id)
}

pub(super) fn sorted_field_slots<'field>(
    fields: impl IntoIterator<Item = &'field String>,
) -> BTreeMap<String, usize> {
    let mut fields = fields.into_iter().cloned().collect::<Vec<_>>();
    fields.sort_unstable();
    fields.dedup();
    fields
        .into_iter()
        .enumerate()
        .map(|(slot, field)| (field, slot))
        .collect()
}
