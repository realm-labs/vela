use vela_bytecode::LinkedProgram;
use vela_common::{PrimitiveTag, script_shape_id};
use vela_def::{TypeId, VariantId};
use vela_mir::{MirCallableKind, MirTypeContract};

use crate::error::{VmError, VmErrorKind, VmResult};
use crate::owned_value::OwnedValue;

pub fn validate_owned_value_contract(
    value: &OwnedValue,
    contract: &MirTypeContract,
    program: &LinkedProgram,
    debug_name: &str,
) -> VmResult<()> {
    if owned_value_matches_contract(value, contract, program) {
        return Ok(());
    }
    Err(VmError::new(VmErrorKind::TypeContractViolation {
        expected: contract_name(contract, program),
        actual: owned_value_type_name(value),
        debug_name: debug_name.to_owned(),
    }))
}

fn owned_value_matches_contract(
    value: &OwnedValue,
    contract: &MirTypeContract,
    program: &LinkedProgram,
) -> bool {
    match contract {
        MirTypeContract::Any => true,
        MirTypeContract::Primitive(expected) => owned_primitive_tag(value) == Some(*expected),
        MirTypeContract::Range => matches!(value, OwnedValue::Range(_)),
        MirTypeContract::Array(element) => match value {
            OwnedValue::Array(values) => {
                optional_elements_match(values, element.as_deref(), program)
            }
            _ => false,
        },
        MirTypeContract::Map {
            key,
            value: element,
        } => match value {
            OwnedValue::Map(entries) => entries.iter().all(|entry| {
                optional_contract_matches(&entry.key, key.as_deref(), program)
                    && optional_contract_matches(&entry.value, element.as_deref(), program)
            }),
            _ => false,
        },
        MirTypeContract::Set(element) => match value {
            OwnedValue::Set(values) => optional_elements_match(values, element.as_deref(), program),
            _ => false,
        },
        MirTypeContract::Iterator(item) => match value {
            OwnedValue::Iterator(iterator) => {
                optional_elements_match(iterator.values(), item.as_deref(), program)
            }
            _ => false,
        },
        MirTypeContract::Tuple(elements) => match value {
            OwnedValue::Tuple(values) => {
                values.len() == elements.len()
                    && values.iter().zip(elements).all(|(value, contract)| {
                        optional_contract_matches(value, contract.as_ref(), program)
                    })
            }
            _ => false,
        },
        MirTypeContract::Option(some) => option_matches(value, some.as_deref(), program),
        MirTypeContract::Result { ok, err } => {
            result_matches(value, ok.as_deref(), err.as_deref(), program)
        }
        MirTypeContract::Callable {
            accepted_kinds,
            positional_arity,
        } => match value {
            OwnedValue::Closure(closure) if accepted_kinds.accepts(MirCallableKind::Closure) => {
                positional_arity.is_none_or(|expected| {
                    closure
                        .owner
                        .program()
                        .function(closure.function)
                        .and_then(|function| u32::try_from(function.params.len()).ok())
                        == Some(expected)
                })
            }
            _ => false,
        },
        MirTypeContract::Definition(expected) => {
            owned_script_type_id(value, program) == Some(*expected)
        }
        MirTypeContract::Shape { type_id, shape } => match value {
            OwnedValue::Record { type_name, fields }
                if resolve_owned_type_id(type_name, program) == Some(*type_id) =>
            {
                linked_type_name(program, *type_id).is_some_and(|owner| {
                    script_shape_id(owner, fields.iter().map(|(name, _)| name)) == *shape
                })
            }
            _ => false,
        },
        MirTypeContract::Variant { type_id, variant } => match value {
            OwnedValue::Enum {
                enum_name,
                variant: variant_name,
                ..
            } => {
                resolve_owned_type_id(enum_name, program) == Some(*type_id)
                    && linked_variant_matches(program, *type_id, *variant, variant_name)
            }
            _ => false,
        },
        MirTypeContract::Host(expected) => {
            matches!(value, OwnedValue::HostRef(host) if host.type_id == expected.runtime)
        }
    }
}

fn optional_elements_match(
    values: &[OwnedValue],
    contract: Option<&MirTypeContract>,
    program: &LinkedProgram,
) -> bool {
    values
        .iter()
        .all(|value| optional_contract_matches(value, contract, program))
}

fn optional_contract_matches(
    value: &OwnedValue,
    contract: Option<&MirTypeContract>,
    program: &LinkedProgram,
) -> bool {
    contract.is_none_or(|contract| owned_value_matches_contract(value, contract, program))
}

fn option_matches(
    value: &OwnedValue,
    some: Option<&MirTypeContract>,
    program: &LinkedProgram,
) -> bool {
    let OwnedValue::Enum {
        enum_name,
        variant,
        fields,
    } = value
    else {
        return false;
    };
    if enum_name.rsplit("::").next() != Some("Option") {
        return false;
    }
    match variant.as_str() {
        "None" => fields.is_empty(),
        "Some" => {
            fields.len() == 1
                && fields
                    .get("0")
                    .is_some_and(|value| optional_contract_matches(value, some, program))
        }
        _ => false,
    }
}

fn result_matches(
    value: &OwnedValue,
    ok: Option<&MirTypeContract>,
    err: Option<&MirTypeContract>,
    program: &LinkedProgram,
) -> bool {
    let OwnedValue::Enum {
        enum_name,
        variant,
        fields,
    } = value
    else {
        return false;
    };
    if enum_name.rsplit("::").next() != Some("Result") || fields.len() != 1 {
        return false;
    }
    match variant.as_str() {
        "Ok" => fields
            .get("0")
            .is_some_and(|value| optional_contract_matches(value, ok, program)),
        "Err" => fields
            .get("0")
            .is_some_and(|value| optional_contract_matches(value, err, program)),
        _ => false,
    }
}

fn owned_script_type_id(value: &OwnedValue, program: &LinkedProgram) -> Option<TypeId> {
    match value {
        OwnedValue::Record { type_name, .. } => resolve_owned_type_id(type_name, program),
        OwnedValue::Enum { enum_name, .. } => resolve_owned_type_id(enum_name, program),
        _ => None,
    }
}

fn resolve_owned_type_id(type_name: &str, program: &LinkedProgram) -> Option<TypeId> {
    let exact = program
        .types()
        .find_map(|(_, ty)| (program.debug_name(ty.debug_name) == type_name).then_some(ty.id));
    if exact.is_some() || type_name.contains("::") {
        return exact;
    }

    let mut matches = program
        .types()
        .filter_map(|(_, ty)| {
            let linked_name = program.debug_name(ty.debug_name);
            (linked_name.rsplit("::").next() == Some(type_name)).then_some(ty.id)
        })
        .collect::<Vec<_>>();
    matches.sort_unstable();
    matches.dedup();
    (matches.len() == 1).then(|| matches[0])
}

fn linked_type_name(program: &LinkedProgram, expected: TypeId) -> Option<&str> {
    program
        .types()
        .find_map(|(_, ty)| (ty.id == expected).then(|| program.debug_name(ty.debug_name)))
}

fn linked_variant_matches(
    program: &LinkedProgram,
    expected_type: TypeId,
    expected_variant: VariantId,
    variant_name: &str,
) -> bool {
    program.variants().any(|(_, variant)| {
        variant.id == expected_variant
            && program
                .ty(variant.owner)
                .is_some_and(|ty| ty.id == expected_type)
            && program.debug_name(variant.debug_name).rsplit("::").next() == Some(variant_name)
    })
}

fn owned_primitive_tag(value: &OwnedValue) -> Option<PrimitiveTag> {
    match value {
        OwnedValue::Unit => Some(PrimitiveTag::Unit),
        OwnedValue::Bool(_) => Some(PrimitiveTag::Bool),
        OwnedValue::Char(_) => Some(PrimitiveTag::Char),
        OwnedValue::Scalar(value) => Some(value.primitive_tag()),
        OwnedValue::String(_) => Some(PrimitiveTag::String),
        OwnedValue::Bytes(_) => Some(PrimitiveTag::Bytes),
        _ => None,
    }
}

fn contract_name(contract: &MirTypeContract, program: &LinkedProgram) -> String {
    match contract {
        MirTypeContract::Any => "Any".to_owned(),
        MirTypeContract::Primitive(value) => value.name().to_owned(),
        MirTypeContract::Range => "Range".to_owned(),
        MirTypeContract::Array(value) => parameterized_name("Array", value.as_deref(), program),
        MirTypeContract::Map { key, value } => format!(
            "Map<{}, {}>",
            optional_contract_name(key.as_deref(), program),
            optional_contract_name(value.as_deref(), program)
        ),
        MirTypeContract::Set(value) => parameterized_name("Set", value.as_deref(), program),
        MirTypeContract::Iterator(value) => {
            parameterized_name("Iterator", value.as_deref(), program)
        }
        MirTypeContract::Tuple(values) => format!(
            "({})",
            values
                .iter()
                .map(|value| optional_contract_name(value.as_ref(), program))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        MirTypeContract::Option(value) => parameterized_name("Option", value.as_deref(), program),
        MirTypeContract::Result { ok, err } => format!(
            "Result<{}, {}>",
            optional_contract_name(ok.as_deref(), program),
            optional_contract_name(err.as_deref(), program)
        ),
        MirTypeContract::Callable { .. } => "callable".to_owned(),
        MirTypeContract::Definition(type_id) | MirTypeContract::Shape { type_id, .. } => {
            linked_type_name(program, *type_id)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("{type_id:?}"))
        }
        MirTypeContract::Variant { type_id, variant } => program
            .variants()
            .find_map(|(_, candidate)| {
                (candidate.id == *variant)
                    .then(|| program.debug_name(candidate.debug_name).to_owned())
            })
            .or_else(|| linked_type_name(program, *type_id).map(str::to_owned))
            .unwrap_or_else(|| format!("{type_id:?}::{variant:?}")),
        MirTypeContract::Host(target) => linked_type_name(program, target.semantic)
            .map(str::to_owned)
            .unwrap_or_else(|| format!("host({:?})", target.runtime)),
    }
}

fn parameterized_name(
    name: &str,
    contract: Option<&MirTypeContract>,
    program: &LinkedProgram,
) -> String {
    contract.map_or_else(
        || name.to_owned(),
        |contract| format!("{name}<{}>", contract_name(contract, program)),
    )
}

fn optional_contract_name(contract: Option<&MirTypeContract>, program: &LinkedProgram) -> String {
    contract.map_or_else(|| "Any".to_owned(), |value| contract_name(value, program))
}

fn owned_value_type_name(value: &OwnedValue) -> String {
    match value {
        OwnedValue::Unit => "()".to_owned(),
        OwnedValue::Bool(_) => "bool".to_owned(),
        OwnedValue::Char(_) => "char".to_owned(),
        OwnedValue::Scalar(value) => value.primitive_tag().name().to_owned(),
        OwnedValue::String(_) => "String".to_owned(),
        OwnedValue::Bytes(_) => "Bytes".to_owned(),
        OwnedValue::Tuple(_) => "tuple".to_owned(),
        OwnedValue::Array(_) => "Array".to_owned(),
        OwnedValue::Map(_) => "Map".to_owned(),
        OwnedValue::Set(_) => "Set".to_owned(),
        OwnedValue::Record { type_name, .. } => type_name.clone(),
        OwnedValue::Enum {
            enum_name, variant, ..
        } => format!("{enum_name}::{variant}"),
        OwnedValue::Closure(_) => "Closure".to_owned(),
        OwnedValue::Range(_) => "Range".to_owned(),
        OwnedValue::HostRef(_) => "host_ref".to_owned(),
        OwnedValue::PathProxy(_) => "path_proxy".to_owned(),
        OwnedValue::Iterator(_) => "Iterator".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use vela_bytecode::{LinkedProgram, LinkedType, LinkedVariant};
    use vela_common::{PrimitiveTag, script_shape_id};
    use vela_def::{TypeId, VariantId};
    use vela_mir::MirTypeContract;

    use super::validate_owned_value_contract;
    use crate::owned_value::OwnedValue;

    #[test]
    fn recursive_owned_contracts_cover_parameterized_values() {
        let program = LinkedProgram::new();
        let i64_contract = MirTypeContract::Primitive(PrimitiveTag::I64);
        let string_contract = MirTypeContract::Primitive(PrimitiveTag::String);
        let cases = [
            (
                MirTypeContract::Array(Some(Box::new(i64_contract.clone()))),
                OwnedValue::array([1_i64, 2_i64]),
                OwnedValue::array([OwnedValue::from(1_i64), OwnedValue::from("wrong")]),
            ),
            (
                MirTypeContract::Map {
                    key: Some(Box::new(string_contract.clone())),
                    value: Some(Box::new(i64_contract.clone())),
                },
                OwnedValue::map([("score", 7_i64)]),
                OwnedValue::map([("score", OwnedValue::from("wrong"))]),
            ),
            (
                MirTypeContract::Set(Some(Box::new(i64_contract.clone()))),
                OwnedValue::set([1_i64, 2_i64]),
                OwnedValue::set([OwnedValue::from(1_i64), OwnedValue::from("wrong")]),
            ),
            (
                MirTypeContract::Tuple(vec![
                    Some(i64_contract.clone()),
                    Some(string_contract.clone()),
                ]),
                OwnedValue::tuple([OwnedValue::from(1_i64), OwnedValue::from("ready")]),
                OwnedValue::tuple([OwnedValue::from(1_i64), OwnedValue::from(2_i64)]),
            ),
            (
                MirTypeContract::Option(Some(Box::new(i64_contract.clone()))),
                OwnedValue::enum_variant("Option", "Some", [("0", OwnedValue::from(1_i64))]),
                OwnedValue::enum_variant("Option", "Some", [("0", OwnedValue::from("wrong"))]),
            ),
            (
                MirTypeContract::Result {
                    ok: Some(Box::new(i64_contract)),
                    err: Some(Box::new(string_contract)),
                },
                OwnedValue::enum_variant("Result", "Ok", [("0", OwnedValue::from(1_i64))]),
                OwnedValue::enum_variant("Result", "Ok", [("0", OwnedValue::from("wrong"))]),
            ),
        ];

        for (contract, valid, invalid) in cases {
            assert_eq!(
                validate_owned_value_contract(&valid, &contract, &program, "main::state"),
                Ok(())
            );
            assert!(
                validate_owned_value_contract(&invalid, &contract, &program, "main::state")
                    .is_err(),
                "malformed nested value passed {contract:?}"
            );
        }
    }

    #[test]
    fn recursive_owned_contracts_resolve_qualified_records_and_enums() {
        let mut program = LinkedProgram::new();
        let player_id = TypeId::new(41);
        let status_id = TypeId::new(42);
        let ready_id = VariantId::new(43);
        let player_name = program.intern_debug_name("game::Player");
        let status_name = program.intern_debug_name("game::Status");
        let ready_name = program.intern_debug_name("game::Status::Ready");
        program.push_type(LinkedType::new(player_id, player_name));
        let status = program.push_type(LinkedType::new(status_id, status_name));
        program.push_variant(LinkedVariant::new(ready_id, status, ready_name));

        let player_contract = MirTypeContract::Shape {
            type_id: player_id,
            shape: script_shape_id("game::Player", ["level"].into_iter()),
        };
        let valid_player = OwnedValue::record("game::Player", [("level", 1_i64)]);
        let invalid_player = OwnedValue::record("game::Player", [("name", "wrong")]);
        assert_eq!(
            validate_owned_value_contract(
                &valid_player,
                &player_contract,
                &program,
                "main::player"
            ),
            Ok(())
        );
        assert!(
            validate_owned_value_contract(
                &invalid_player,
                &player_contract,
                &program,
                "main::player"
            )
            .is_err()
        );

        let status_contract = MirTypeContract::Variant {
            type_id: status_id,
            variant: ready_id,
        };
        let valid_status =
            OwnedValue::enum_variant("game::Status", "Ready", Vec::<(&str, OwnedValue)>::new());
        let invalid_status =
            OwnedValue::enum_variant("game::Status", "Waiting", Vec::<(&str, OwnedValue)>::new());
        assert_eq!(
            validate_owned_value_contract(
                &valid_status,
                &status_contract,
                &program,
                "main::status"
            ),
            Ok(())
        );
        assert!(
            validate_owned_value_contract(
                &invalid_status,
                &status_contract,
                &program,
                "main::status"
            )
            .is_err()
        );
    }

    #[test]
    fn qualified_owned_type_names_resolve_exactly_without_leaf_fallback() {
        let mut program = LinkedProgram::new();
        let alpha_id = TypeId::new(51);
        let beta_id = TypeId::new(52);
        let alpha_name = program.intern_debug_name("alpha::Player");
        let beta_name = program.intern_debug_name("beta::Player");
        program.push_type(LinkedType::new(alpha_id, alpha_name));
        program.push_type(LinkedType::new(beta_id, beta_name));
        let alpha_contract = MirTypeContract::Definition(alpha_id);
        let beta_contract = MirTypeContract::Definition(beta_id);

        assert_eq!(
            validate_owned_value_contract(
                &OwnedValue::record("alpha::Player", Vec::<(&str, OwnedValue)>::new()),
                &alpha_contract,
                &program,
                "main::alpha"
            ),
            Ok(())
        );
        assert_eq!(
            validate_owned_value_contract(
                &OwnedValue::record("beta::Player", Vec::<(&str, OwnedValue)>::new()),
                &beta_contract,
                &program,
                "main::beta"
            ),
            Ok(())
        );
        assert!(
            validate_owned_value_contract(
                &OwnedValue::record("spoofed::Player", Vec::<(&str, OwnedValue)>::new()),
                &alpha_contract,
                &program,
                "main::alpha"
            )
            .is_err()
        );
        assert!(
            validate_owned_value_contract(
                &OwnedValue::record("Player", Vec::<(&str, OwnedValue)>::new()),
                &alpha_contract,
                &program,
                "main::alpha"
            )
            .is_err()
        );
    }
}
