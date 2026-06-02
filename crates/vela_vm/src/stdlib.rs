use crate::Vm;

pub(crate) fn register(vm: &mut Vm) {
    crate::option_result::register(vm);
    crate::math_stdlib::register(vm);
    vm.register_native("set::from_array", crate::set_methods::from_array);
}

#[cfg(test)]
mod tests;
