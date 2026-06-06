use crate::Vm;
use vela_common::standard_ids::SET_FROM_ARRAY_FUNCTION_ID;

pub(crate) fn register(vm: &mut Vm) {
    crate::option_result::register(vm);
    crate::math_stdlib::register(vm);
    vm.register_native_with_id(
        SET_FROM_ARRAY_FUNCTION_ID,
        "set::from_array",
        crate::set_methods::from_array,
    );
}

#[cfg(test)]
mod tests;
