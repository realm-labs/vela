use crate::type_fact::TypeFact;
use vela_common::PrimitiveTag;
use vela_stdlib::{STD_FUNCTIONS, STD_METHODS, StdParamSpec, reflection_native_spec};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StdlibParameterMetadata {
    pub name: &'static str,
    pub defaulted: bool,
}

pub(super) fn function_parameters(
    name: &str,
    arity: usize,
) -> Option<Vec<StdlibParameterMetadata>> {
    if let Some((module, leaf)) = name.rsplit_once("::")
        && let Some(spec) = STD_FUNCTIONS
            .iter()
            .find(|spec| module == spec.module && leaf == spec.name)
    {
        return parameters(spec.params, arity);
    }
    let spec = reflection_native_spec(name)?;
    (spec.params.len() == arity).then(|| {
        spec.params
            .iter()
            .map(|name| StdlibParameterMetadata {
                name,
                defaulted: false,
            })
            .collect()
    })
}

pub(super) fn method_parameters(
    receiver: &TypeFact,
    method: &str,
    arity: usize,
) -> Option<Vec<StdlibParameterMetadata>> {
    let owner = match receiver {
        TypeFact::Array { .. } | TypeFact::ArrayView { .. } | TypeFact::ArrayMut { .. } => "Array",
        TypeFact::Map { .. } | TypeFact::MapView { .. } | TypeFact::MapMut { .. } => "Map",
        TypeFact::Set { .. } | TypeFact::SetView { .. } | TypeFact::SetMut { .. } => "Set",
        TypeFact::Iterator { .. } | TypeFact::ScopedIterator { .. } => "Iterator",
        TypeFact::Option { .. } | TypeFact::OptionSome { .. } | TypeFact::OptionNone => "Option",
        TypeFact::Result { .. } | TypeFact::ResultOk { .. } | TypeFact::ResultErr { .. } => {
            "Result"
        }
        TypeFact::Primitive(PrimitiveTag::String) => "String",
        TypeFact::Primitive(PrimitiveTag::Bytes) => "Bytes",
        TypeFact::Primitive(PrimitiveTag::Char) => "Char",
        TypeFact::Range => "Range",
        _ => return None,
    };
    let spec = STD_METHODS
        .iter()
        .find(|spec| spec.owner == owner && spec.name == method)?;
    parameters(spec.params, arity)
}

fn parameters(params: &[StdParamSpec], arity: usize) -> Option<Vec<StdlibParameterMetadata>> {
    (params.len() == arity).then(|| {
        params
            .iter()
            .map(|p| StdlibParameterMetadata {
                name: p.name,
                defaulted: p.defaulted,
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdlib::{stdlib_function_completion_facts, stdlib_method_facts};

    #[test]
    fn standard_function_parameter_names_match_the_registration_manifest() {
        let facts = stdlib_function_completion_facts();
        let mut missing = Vec::new();
        for fact in &facts {
            let Some(parameters) = fact.parameter_metadata() else {
                missing.push(fact.name);
                continue;
            };
            assert_eq!(
                fact.param_names,
                parameters.iter().map(|p| p.name).collect::<Vec<_>>(),
                "{}",
                fact.name
            );
            assert_eq!(parameters.len(), fact.params.len(), "{}", fact.name);
            let registered = STD_FUNCTIONS
                .iter()
                .find(|spec| fact.name == format!("{}::{}", spec.module, spec.name))
                .map(|spec| spec.signature())
                .or_else(|| reflection_native_spec(fact.name).map(|spec| spec.signature()))
                .expect("registered signature");
            assert_eq!(
                parameters
                    .iter()
                    .map(|p| (p.name, p.defaulted))
                    .collect::<Vec<_>>(),
                registered
                    .params
                    .iter()
                    .map(|p| (p.name.as_str(), p.has_default))
                    .collect::<Vec<_>>(),
                "{}",
                fact.name
            );
        }
        missing.sort_unstable();
        assert_eq!(
            missing,
            [
                "fs::read_to_string",
                "fs::write_string",
                "io::print",
                "io::println",
                "math::random",
                "reflect::call",
                "reflect::fields",
                "reflect::methods",
                "reflect::traits",
                "reflect::variants",
                "task::spawn_scoped",
                "task::spawn_scoped_then",
                "time::elapsed_since",
                "time::now",
                "time::tick"
            ]
        );
        let absent = STD_FUNCTIONS
            .iter()
            .filter(|spec| {
                !facts
                    .iter()
                    .any(|fact| fact.name == format!("{}::{}", spec.module, spec.name))
            })
            .map(|spec| format!("{}::{}", spec.module, spec.name))
            .collect::<Vec<_>>();
        assert!(
            absent.is_empty(),
            "registered functions missing analysis facts: {absent:?}"
        );
        assert_eq!(
            function_parameters("option::unwrap_or", 2)
                .expect("registered")
                .iter()
                .map(|p| p.name)
                .collect::<Vec<_>>(),
            ["option", "fallback"]
        );
        assert!(function_parameters("missing::unwrap_or", 2).is_none());
        assert!(function_parameters("option::unwrap_or", 1).is_none());
    }

    #[test]
    fn standard_method_parameter_names_and_defaults_cover_builtin_families() {
        let receivers = [
            (TypeFact::array(TypeFact::I64), "Array"),
            (TypeFact::map(TypeFact::STRING, TypeFact::I64), "Map"),
            (TypeFact::set(TypeFact::I64), "Set"),
            (TypeFact::iterator(TypeFact::I64), "Iterator"),
            (TypeFact::option(TypeFact::I64), "Option"),
            (TypeFact::option(TypeFact::option(TypeFact::I64)), "Option"),
            (TypeFact::result(TypeFact::I64, TypeFact::STRING), "Result"),
            (
                TypeFact::result(
                    TypeFact::result(TypeFact::I64, TypeFact::STRING),
                    TypeFact::STRING,
                ),
                "Result",
            ),
            (TypeFact::STRING, "String"),
            (TypeFact::BYTES, "Bytes"),
            (TypeFact::CHAR, "Char"),
            (TypeFact::Range, "Range"),
        ];
        let mut seen = std::collections::BTreeSet::new();
        for (receiver, owner) in receivers {
            for fact in stdlib_method_facts(&receiver, None) {
                let params = fact.parameter_metadata().unwrap_or_else(|| {
                    panic!(
                        "missing metadata: {}.{} (arity {})",
                        receiver.display_name(),
                        fact.method,
                        fact.params.len()
                    )
                });
                assert_eq!(params.len(), fact.params.len());
                let registered = STD_METHODS
                    .iter()
                    .find(|spec| spec.owner == owner && spec.name == fact.method)
                    .expect("registered method")
                    .signature();
                assert_eq!(
                    params
                        .iter()
                        .map(|p| (p.name, p.defaulted))
                        .collect::<Vec<_>>(),
                    registered
                        .params
                        .iter()
                        .map(|p| (p.name.as_str(), p.has_default))
                        .collect::<Vec<_>>(),
                    "{owner}.{}",
                    fact.method
                );
                seen.insert((owner, fact.method));
            }
        }
        let absent = STD_METHODS
            .iter()
            .filter(|spec| !seen.contains(&(spec.owner, spec.name)))
            .map(|spec| (spec.owner, spec.name))
            .collect::<Vec<_>>();
        assert!(
            absent.is_empty(),
            "registered methods missing analysis facts: {absent:?}"
        );
        assert_eq!(
            method_parameters(&TypeFact::array(TypeFact::I64), "sum", 1),
            Some(vec![StdlibParameterMetadata {
                name: "callback",
                defaulted: true
            }])
        );
        assert!(method_parameters(&TypeFact::Any, "push", 1).is_none());
        assert!(method_parameters(&TypeFact::STRING, "push", 1).is_none());
    }
}
