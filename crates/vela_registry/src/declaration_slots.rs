use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use vela_def::{FieldId, TypeId, VariantId};

use crate::{Def, FieldDef, RegistryCompileView, VariantDef};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RegistryDeclarationSlots {
    fields: BTreeMap<FieldId, u32>,
    variants: BTreeMap<VariantId, u32>,
}

impl RegistryDeclarationSlots {
    pub(crate) fn from_view(
        view: RegistryCompileView<'_>,
    ) -> Result<Self, RegistryDeclarationSlotError> {
        let mut fields_by_shape = BTreeMap::<(TypeId, Option<VariantId>), Vec<&FieldDef>>::new();
        let mut variants_by_owner = BTreeMap::<TypeId, Vec<&VariantDef>>::new();
        for definition in view.definitions() {
            match definition {
                Def::Field(field) => fields_by_shape
                    .entry((field.owner, field.variant))
                    .or_default()
                    .push(field),
                Def::Variant(variant) => variants_by_owner
                    .entry(variant.owner)
                    .or_default()
                    .push(variant),
                Def::Function(_) | Def::Type(_) | Def::Method(_) | Def::Trait(_) => {}
            }
        }

        let mut slots = Self::default();
        for ((owner, variant), fields) in &mut fields_by_shape {
            fields
                .sort_by_key(|field| (field.declaration_order, field.path.name.clone(), field.id));
            for (index, field) in fields.iter().enumerate() {
                let slot = u32::try_from(index).map_err(|_| {
                    RegistryDeclarationSlotError::FieldShapeTooLarge {
                        owner: *owner,
                        variant: *variant,
                    }
                })?;
                slots.fields.insert(field.id, slot);
            }
        }
        for (owner, variants) in &mut variants_by_owner {
            variants.sort_by_key(|variant| {
                (
                    variant.declaration_order,
                    variant.path.name.clone(),
                    variant.id,
                )
            });
            for (index, variant) in variants.iter().enumerate() {
                let slot = u32::try_from(index).map_err(|_| {
                    RegistryDeclarationSlotError::VariantShapeTooLarge { owner: *owner }
                })?;
                slots.variants.insert(variant.id, slot);
            }
        }
        Ok(slots)
    }

    pub fn field(&self, field: FieldId) -> Result<u32, RegistryDeclarationSlotError> {
        self.fields
            .get(&field)
            .copied()
            .ok_or(RegistryDeclarationSlotError::MissingField { field })
    }

    pub fn variant(&self, variant: VariantId) -> Result<u32, RegistryDeclarationSlotError> {
        self.variants
            .get(&variant)
            .copied()
            .ok_or(RegistryDeclarationSlotError::MissingVariant { variant })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryDeclarationSlotError {
    FieldShapeTooLarge {
        owner: TypeId,
        variant: Option<VariantId>,
    },
    VariantShapeTooLarge {
        owner: TypeId,
    },
    MissingField {
        field: FieldId,
    },
    MissingVariant {
        variant: VariantId,
    },
}

impl fmt::Display for RegistryDeclarationSlotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FieldShapeTooLarge { owner, variant } => write!(
                formatter,
                "registry field shape for type {owner:?} variant {variant:?} exceeds u32 slots"
            ),
            Self::VariantShapeTooLarge { owner } => write!(
                formatter,
                "registry variant shape for type {owner:?} exceeds u32 slots"
            ),
            Self::MissingField { field } => {
                write!(formatter, "registry declaration slots omit field {field:?}")
            }
            Self::MissingVariant { variant } => {
                write!(
                    formatter,
                    "registry declaration slots omit variant {variant:?}"
                )
            }
        }
    }
}

impl Error for RegistryDeclarationSlotError {}
