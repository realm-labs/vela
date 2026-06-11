mod array;
mod bytes;
mod map;
mod option;
mod range;
mod result;
mod set;
mod string;

use vela_common::HostMethodId;
use vela_reflect::registry::{MethodDesc, MethodParamDesc};

pub(crate) use array::array_method_descs;
pub(crate) use bytes::bytes_method_descs;
pub(crate) use map::map_method_descs;
pub(crate) use option::option_method_descs;
pub(crate) use range::range_method_descs;
pub(crate) use result::result_method_descs;
pub(crate) use set::set_method_descs;
pub(crate) use string::string_method_descs;

#[derive(Clone, Copy)]
struct ParamSpec {
    name: &'static str,
    type_hint: &'static str,
    defaulted: bool,
}

impl ParamSpec {
    const fn new(name: &'static str, type_hint: &'static str) -> Self {
        Self {
            name,
            type_hint,
            defaulted: false,
        }
    }

    const fn optional(name: &'static str, type_hint: &'static str) -> Self {
        Self {
            name,
            type_hint,
            defaulted: true,
        }
    }
}

#[derive(Clone, Copy)]
struct MethodSpec {
    name: &'static str,
    params: &'static [ParamSpec],
    return_type: &'static str,
    docs: &'static str,
}

impl MethodSpec {
    const fn new(
        name: &'static str,
        params: &'static [ParamSpec],
        return_type: &'static str,
        docs: &'static str,
    ) -> Self {
        Self {
            name,
            params,
            return_type,
            docs,
        }
    }
}

fn descs(owner: &'static str, specs: &[MethodSpec], stdlib: &'static str) -> Vec<MethodDesc> {
    specs
        .iter()
        .map(|spec| desc(owner, *spec, stdlib))
        .collect()
}

fn desc(owner: &'static str, spec: MethodSpec, stdlib: &'static str) -> MethodDesc {
    let mut desc = MethodDesc::new(std_method_host_id(owner, spec.name), spec.name)
        .return_type(spec.return_type)
        .attr("stdlib", stdlib)
        .docs(spec.docs);
    for param in spec.params {
        desc = desc.param(
            MethodParamDesc::new(param.name)
                .type_hint(param.type_hint)
                .defaulted(param.defaulted),
        );
    }
    desc
}

fn std_method_host_id(owner: &str, name: &str) -> HostMethodId {
    let Some(id) = vela_stdlib::std_method_id(owner, name) else {
        panic!("missing standard method identity for {owner}::{name}");
    };
    HostMethodId::new(id.get())
}
