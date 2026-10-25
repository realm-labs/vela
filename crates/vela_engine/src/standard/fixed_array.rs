use vela_reflect::registry::{TraitDesc, TypeDesc, TypeKind};

use crate::args::{FromScriptArg, IntoScriptArg};
use crate::interop::VelaValueBoundary;
use crate::metadata::type_hint_display;
use crate::type_binding::TypeBinding;

use super::bindings::{StandardTypeBinding, concrete_type_desc};

impl<T, const N: usize> StandardTypeBinding for [T; N]
where
    T: VelaValueBoundary + IntoScriptArg + FromScriptArg + 'static,
{
    fn standard_type_binding() -> TypeBinding<Self> {
        TypeBinding::value(fixed_array_type_desc::<T, N>())
    }
}

fn fixed_array_type_desc<T, const N: usize>() -> TypeDesc
where
    T: VelaValueBoundary,
{
    let element = type_hint_display(&T::vela_type_hint());
    concrete_type_desc(
        "fixed_array",
        format!("rust::array::Array<{element}, {N}>"),
        &format!("{element};{N}"),
        TypeKind::Array,
    )
    .trait_impl(TraitDesc::new("Sequence"))
    .trait_impl(TraitDesc::new("Iterable"))
    .docs(format!(
        "Concrete Rust [{element}; {N}] value binding using fixed-length Vela Array behavior."
    ))
    .attr("rust_standard_family", "fixed_array")
    .attr("vela_collection_protocol", "Sequence,Iterable")
    .attr("vela_collection_element", element)
    .attr("vela_collection_growth", "fixed")
    .attr("vela_collection_length", N.to_string())
}

#[cfg(test)]
mod tests {}
