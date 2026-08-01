use std::collections::BTreeSet;

use vela_common::{Detachability, NonDetachableValueKind};
use vela_def::TypeId;

use crate::{CompileTypeClass, MirTargetTable, MirTypeContract};

/// Recursive ownership proof for one sealed MIR value contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirDetachabilityReport {
    pub fact: Detachability,
    /// Field/variant suffix of the first statically rejected edge.
    pub rejection_path: Vec<String>,
}

impl MirDetachabilityReport {
    fn detachable() -> Self {
        Self {
            fact: Detachability::Detachable,
            rejection_path: Vec::new(),
        }
    }

    fn runtime_checked() -> Self {
        Self {
            fact: Detachability::RuntimeChecked,
            rejection_path: Vec::new(),
        }
    }

    fn rejected(kind: NonDetachableValueKind) -> Self {
        Self {
            fact: Detachability::NonDetachable(kind),
            rejection_path: Vec::new(),
        }
    }

    fn with_prefix(mut self, prefix: String) -> Self {
        if self.fact.rejection().is_some() {
            self.rejection_path.insert(0, prefix);
        }
        self
    }

    fn union(self, other: Self) -> Self {
        match self.fact.union(other.fact) {
            Detachability::NonDetachable(_) if self.fact.rejection().is_some() => self,
            Detachability::NonDetachable(_) => other,
            Detachability::RuntimeChecked => Self::runtime_checked(),
            Detachability::Detachable => Self::detachable(),
        }
    }
}

/// Computes the authoritative recursive detachment fact from the sealed type
/// table. Recursive nominal edges are accepted provisionally; every concrete
/// field edge is still visited once, and runtime graph copying preserves value
/// cycles independently.
#[must_use]
pub fn contract_detachability(
    targets: &MirTargetTable,
    contract: Option<&MirTypeContract>,
) -> MirDetachabilityReport {
    let Some(contract) = contract else {
        return MirDetachabilityReport::runtime_checked();
    };
    contract_detachability_inner(targets, contract, &mut BTreeSet::new())
}

fn contract_detachability_inner(
    targets: &MirTargetTable,
    contract: &MirTypeContract,
    visiting: &mut BTreeSet<TypeId>,
) -> MirDetachabilityReport {
    match contract {
        MirTypeContract::Any => MirDetachabilityReport::runtime_checked(),
        MirTypeContract::Primitive(_) | MirTypeContract::Range => {
            MirDetachabilityReport::detachable()
        }
        MirTypeContract::Array(element)
        | MirTypeContract::Set(element)
        | MirTypeContract::Option(element)
        | MirTypeContract::Iterator(element) => {
            if matches!(contract, MirTypeContract::Iterator(_)) {
                return MirDetachabilityReport::rejected(NonDetachableValueKind::Iterator);
            }
            nested(targets, element.as_deref(), visiting)
        }
        MirTypeContract::Map { key, value }
        | MirTypeContract::Result {
            ok: key,
            err: value,
        } => nested(targets, key.as_deref(), visiting).union(nested(
            targets,
            value.as_deref(),
            visiting,
        )),
        MirTypeContract::Tuple(elements) => elements.iter().enumerate().fold(
            MirDetachabilityReport::detachable(),
            |report, (index, element)| {
                report.union(
                    nested(targets, element.as_ref(), visiting).with_prefix(format!("[{index}]")),
                )
            },
        ),
        MirTypeContract::Callable { .. } => {
            MirDetachabilityReport::rejected(NonDetachableValueKind::Callable)
        }
        MirTypeContract::Host(_) => {
            MirDetachabilityReport::rejected(NonDetachableValueKind::HostReference)
        }
        MirTypeContract::Definition(type_id) | MirTypeContract::Shape { type_id, .. } => {
            nominal_detachability(targets, *type_id, visiting)
        }
        MirTypeContract::Variant { type_id, variant } => {
            if !visiting.insert(*type_id) {
                return MirDetachabilityReport::detachable();
            }
            let report = targets.variant(*variant).map_or_else(
                MirDetachabilityReport::runtime_checked,
                |descriptor| {
                    descriptor.fields.iter().fold(
                        MirDetachabilityReport::detachable(),
                        |report, field| {
                            report.union(field_detachability(targets, *field, visiting))
                        },
                    )
                },
            );
            visiting.remove(type_id);
            report
        }
    }
}

fn nested(
    targets: &MirTargetTable,
    contract: Option<&MirTypeContract>,
    visiting: &mut BTreeSet<TypeId>,
) -> MirDetachabilityReport {
    contract.map_or_else(MirDetachabilityReport::runtime_checked, |contract| {
        contract_detachability_inner(targets, contract, visiting)
    })
}

fn nominal_detachability(
    targets: &MirTargetTable,
    type_id: TypeId,
    visiting: &mut BTreeSet<TypeId>,
) -> MirDetachabilityReport {
    if !visiting.insert(type_id) {
        return MirDetachabilityReport::detachable();
    }
    let report = targets.type_descriptor(type_id).map_or_else(
        MirDetachabilityReport::runtime_checked,
        |descriptor| match descriptor.class {
            CompileTypeClass::Host { .. } => {
                MirDetachabilityReport::rejected(NonDetachableValueKind::HostReference)
            }
            CompileTypeClass::OpaqueExternal => MirDetachabilityReport::runtime_checked(),
            CompileTypeClass::Registry
                if descriptor.fields.is_empty() && descriptor.variants.is_empty() =>
            {
                MirDetachabilityReport::runtime_checked()
            }
            CompileTypeClass::ScriptRecord
            | CompileTypeClass::ScriptEnum
            | CompileTypeClass::Registry
            | CompileTypeClass::Standard => descriptor
                .fields
                .iter()
                .fold(MirDetachabilityReport::detachable(), |report, field| {
                    report.union(field_detachability(targets, *field, visiting))
                })
                .union(descriptor.variants.iter().fold(
                    MirDetachabilityReport::detachable(),
                    |report, variant_id| {
                        let name = targets
                            .variant(*variant_id)
                            .map(|variant| variant.name.clone())
                            .unwrap_or_else(|| format!("#{}", variant_id.get()));
                        let nested = targets.variant(*variant_id).map_or_else(
                            MirDetachabilityReport::runtime_checked,
                            |variant| {
                                variant.fields.iter().fold(
                                    MirDetachabilityReport::detachable(),
                                    |report, field| {
                                        report.union(field_detachability(targets, *field, visiting))
                                    },
                                )
                            },
                        );
                        report.union(nested.with_prefix(format!("::{name}")))
                    },
                )),
        },
    );
    visiting.remove(&type_id);
    report
}

fn field_detachability(
    targets: &MirTargetTable,
    field: vela_def::FieldId,
    visiting: &mut BTreeSet<TypeId>,
) -> MirDetachabilityReport {
    targets
        .field(field)
        .map_or_else(MirDetachabilityReport::runtime_checked, |field| {
            nested(targets, field.contract.as_ref(), visiting)
                .with_prefix(format!(".{}", field.name))
        })
}

#[cfg(test)]
mod tests {
    use vela_common::{HostTypeId, ShapeId};
    use vela_def::{FieldId, TypeId};

    use super::*;
    use crate::{
        CompileFieldAccess, CompileFieldDescriptor, CompileTypeDescriptor, HostTypeTarget,
    };

    #[test]
    fn nominal_contract_reports_the_nested_host_field_path() {
        let envelope = TypeId::new(41);
        let context = TypeId::new(42);
        let field = FieldId::new(43);
        let mut targets = MirTargetTable::default();
        assert!(targets.insert_type(CompileTypeDescriptor {
            id: envelope,
            canonical_name: "Envelope".to_owned(),
            runtime_name: "Envelope".to_owned(),
            class: CompileTypeClass::ScriptRecord,
            shape: Some(ShapeId::new(1)),
            fields: vec![field],
            variants: Vec::new(),
        }));
        assert!(targets.insert_type(CompileTypeDescriptor {
            id: context,
            canonical_name: "Context".to_owned(),
            runtime_name: "Context".to_owned(),
            class: CompileTypeClass::Host {
                runtime: HostTypeId::new(7),
            },
            shape: None,
            fields: Vec::new(),
            variants: Vec::new(),
        }));
        assert!(targets.insert_field(CompileFieldDescriptor {
            id: field,
            owner: envelope,
            variant: None,
            name: "context".to_owned(),
            contract: Some(MirTypeContract::Host(HostTypeTarget {
                semantic: context,
                runtime: HostTypeId::new(7),
            })),
            declaration_order: 0,
            access: CompileFieldAccess::script(),
            host_runtime: None,
        }));

        assert_eq!(
            contract_detachability(&targets, Some(&MirTypeContract::Definition(envelope))),
            MirDetachabilityReport {
                fact: Detachability::NonDetachable(NonDetachableValueKind::HostReference),
                rejection_path: vec![".context".to_owned()],
            }
        );
    }
}
