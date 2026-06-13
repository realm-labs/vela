use crate::native::{EffectSet, FunctionAccess, NativeFunctionDesc, TypeHint};
use vela_reflect::registry::TypeKey;

pub(crate) fn standard_native_function_descs() -> Vec<NativeFunctionDesc> {
    vela_stdlib::STD_FUNCTIONS
        .iter()
        .map(|spec| {
            let mut desc =
                NativeFunctionDesc::new(format!("{}::{}", spec.module, spec.name), spec.id())
                    .returns(type_hint(spec.return_type))
                    .effects(EffectSet::pure())
                    .access(FunctionAccess::public().reflect_callable(true))
                    .docs(spec.docs)
                    .attr("stdlib", spec.module);

            for param in spec.params {
                desc = desc.param(param.name, type_hint(param.type_hint));
            }

            desc
        })
        .collect()
}

fn type_hint(hint: &str) -> TypeHint {
    match hint {
        "any" => TypeHint::Any,
        "null" => TypeHint::null(),
        "bool" => TypeHint::boolean(),
        "char" => TypeHint::char(),
        "i8" => TypeHint::i8(),
        "i16" => TypeHint::i16(),
        "i32" => TypeHint::i32(),
        "i64" => TypeHint::i64(),
        "u8" => TypeHint::u8(),
        "u16" => TypeHint::u16(),
        "u32" => TypeHint::u32(),
        "u64" => TypeHint::u64(),
        "f32" => TypeHint::f32(),
        "f64" => TypeHint::f64(),
        "string" => TypeHint::string(),
        "bytes" => TypeHint::bytes(),
        "array" => TypeHint::Array,
        "map" => TypeHint::Map,
        "set" => TypeHint::Set,
        "iterator" => TypeHint::Iterator,
        "function" => TypeHint::Function,
        "Option" | "Result" => {
            let id = vela_stdlib::std_type_id(hint)
                .unwrap_or_else(|| panic!("missing standard enum type identity for {hint}"));
            TypeHint::Enum(TypeKey::new(id, hint))
        }
        _ => TypeHint::Any,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_standard_function_descs_match_manifest() {
        let descs = standard_native_function_descs();

        assert_eq!(descs.len(), vela_stdlib::STD_FUNCTIONS.len());
        for (desc, spec) in descs.iter().zip(vela_stdlib::STD_FUNCTIONS) {
            assert_eq!(desc.id, spec.id());
            assert_eq!(desc.name, format!("{}::{}", spec.module, spec.name));
            assert_eq!(desc.params.len(), spec.params.len());
            assert_eq!(desc.docs.as_deref(), Some(spec.docs));
            assert_eq!(desc.attrs.get("stdlib"), Some(spec.module));
            assert_eq!(desc.effects, EffectSet::pure());
            assert!(desc.access.public);
            assert!(desc.access.reflect_visible);
            assert!(desc.access.reflect_callable);
        }
    }

    #[test]
    fn generated_standard_function_descs_preserve_type_hints() {
        let descs = standard_native_function_descs();
        let lerp = descs
            .iter()
            .find(|desc| desc.name == "math::lerp")
            .expect("math::lerp should be generated from the manifest");
        let set_from_array = descs
            .iter()
            .find(|desc| desc.name == "set::from_array")
            .expect("set::from_array should be generated from the manifest");

        assert_eq!(lerp.returns, TypeHint::f64());
        assert_eq!(lerp.params[2].name, "t");
        assert_eq!(lerp.params[2].hint, TypeHint::Any);
        assert_eq!(set_from_array.returns, TypeHint::Set);
        assert_eq!(set_from_array.params[0].hint, TypeHint::Array);
    }
}
