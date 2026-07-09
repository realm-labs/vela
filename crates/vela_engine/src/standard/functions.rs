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
    parse_type_hint(hint).unwrap_or(TypeHint::Any)
}

fn parse_type_hint(hint: &str) -> Option<TypeHint> {
    let hint = hint.trim();
    if let Some((name, args)) = type_args(hint) {
        let args = split_top_level_args(args)?;
        return match name {
            "Array" | "array" if args.len() == 1 => {
                Some(TypeHint::array_of(parse_type_hint(args[0])?))
            }
            "Map" | "map" if args.len() == 2 => Some(TypeHint::map_of(
                parse_type_hint(args[0])?,
                parse_type_hint(args[1])?,
            )),
            "Set" | "set" if args.len() == 1 => Some(TypeHint::set_of(parse_type_hint(args[0])?)),
            "Iterator" | "iterator" if args.len() == 1 => {
                Some(TypeHint::iterator_of(parse_type_hint(args[0])?))
            }
            "Option" | "option" if args.len() == 1 => {
                Some(TypeHint::option_of(parse_type_hint(args[0])?))
            }
            "Result" | "result" if args.len() == 2 => Some(TypeHint::result_of(
                parse_type_hint(args[0])?,
                parse_type_hint(args[1])?,
            )),
            _ => None,
        };
    }

    match hint {
        "any" | "Any" => Some(TypeHint::Any),
        "()" => Some(TypeHint::unit()),
        "bool" => Some(TypeHint::boolean()),
        "char" => Some(TypeHint::char()),
        "i8" => Some(TypeHint::i8()),
        "i16" => Some(TypeHint::i16()),
        "i32" => Some(TypeHint::i32()),
        "i64" => Some(TypeHint::i64()),
        "u8" => Some(TypeHint::u8()),
        "u16" => Some(TypeHint::u16()),
        "u32" => Some(TypeHint::u32()),
        "u64" => Some(TypeHint::u64()),
        "f32" => Some(TypeHint::f32()),
        "f64" => Some(TypeHint::f64()),
        "string" | "String" => Some(TypeHint::string()),
        "bytes" | "Bytes" => Some(TypeHint::bytes()),
        "array" | "Array" => Some(TypeHint::Array),
        "map" | "Map" => Some(TypeHint::Map),
        "set" | "Set" => Some(TypeHint::Set),
        "iterator" | "Iterator" => Some(TypeHint::Iterator),
        "function" | "Function" => Some(TypeHint::Function),
        "option" | "Option" => {
            let id = vela_stdlib::std_type_id("Option")
                .unwrap_or_else(|| panic!("missing standard enum type identity for Option"));
            Some(TypeHint::Enum(TypeKey::new(id, "Option")))
        }
        "result" | "Result" => {
            let id = vela_stdlib::std_type_id("Result")
                .unwrap_or_else(|| panic!("missing standard enum type identity for Result"));
            Some(TypeHint::Enum(TypeKey::new(id, "Result")))
        }
        _ => None,
    }
}

fn type_args(hint: &str) -> Option<(&str, &str)> {
    let open = hint.find('<')?;
    if !hint.ends_with('>') {
        return None;
    }
    Some((hint[..open].trim(), &hint[open + 1..hint.len() - 1]))
}

fn split_top_level_args(args: &str) -> Option<Vec<&str>> {
    let mut split = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;

    for (index, ch) in args.char_indices() {
        match ch {
            '<' => depth = depth.checked_add(1)?,
            '>' => depth = depth.checked_sub(1)?,
            ',' if depth == 0 => {
                let arg = args[start..index].trim();
                if arg.is_empty() {
                    return None;
                }
                split.push(arg);
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }

    if depth != 0 {
        return None;
    }
    let tail = args[start..].trim();
    if tail.is_empty() {
        return None;
    }
    split.push(tail);
    Some(split)
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
        let bytes_from_hex = descs
            .iter()
            .find(|desc| desc.name == "bytes::from_hex")
            .expect("bytes::from_hex should be generated from the manifest");
        let i8_try_from_i64 = descs
            .iter()
            .find(|desc| desc.name == "i8::try_from_i64")
            .expect("i8::try_from_i64 should be generated from the manifest");

        assert_eq!(lerp.returns, TypeHint::f64());
        assert_eq!(lerp.params[2].name, "t");
        assert_eq!(lerp.params[2].hint, TypeHint::Any);
        assert_eq!(set_from_array.returns, TypeHint::Set);
        assert_eq!(set_from_array.params[0].hint, TypeHint::Array);
        assert_eq!(
            bytes_from_hex.returns,
            TypeHint::result_of(TypeHint::bytes(), TypeHint::string())
        );
        assert_eq!(
            i8_try_from_i64.returns,
            TypeHint::result_of(TypeHint::i8(), TypeHint::string())
        );
    }
}
