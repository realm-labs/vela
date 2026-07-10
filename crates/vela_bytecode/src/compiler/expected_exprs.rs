use crate::GuardLocation;

use super::value_types::TypeContractContext;

pub(super) fn guard_location_and_name(
    context: TypeContractContext,
) -> Option<(GuardLocation, String)> {
    match context {
        TypeContractContext::TypedLet { name } => Some((GuardLocation::Local, name)),
        TypeContractContext::Field { name } => Some((GuardLocation::Field, name)),
        TypeContractContext::NativeParameter { name, index, .. } => {
            Some((GuardLocation::Parameter { index }, name))
        }
        TypeContractContext::FunctionParameter { .. } => None,
    }
}
