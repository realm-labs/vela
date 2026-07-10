use std::collections::BTreeMap;

use vela_registry::{DefinitionRegistry, RegistryError};

use crate::{STD_FIELDS, STD_FUNCTIONS, STD_METHODS, STD_TYPES, STD_VARIANTS};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StdlibRegistration {
    pub types: usize,
    pub variants: usize,
    pub fields: usize,
    pub functions: usize,
    pub methods: usize,
}

pub fn register_stdlib(
    registry: &mut DefinitionRegistry,
) -> Result<StdlibRegistration, RegistryError> {
    for spec in STD_TYPES {
        registry.register_type(spec.def())?;
    }
    let mut variant_orders = BTreeMap::<&str, u32>::new();
    for spec in STD_VARIANTS {
        let order = variant_orders.entry(spec.owner).or_default();
        registry.register_variant(spec.def().declaration_order(*order))?;
        *order += 1;
    }
    let mut field_orders = BTreeMap::<&str, u32>::new();
    for spec in STD_FIELDS {
        let order = field_orders.entry(spec.owner).or_default();
        registry.register_field(spec.def().declaration_order(*order))?;
        *order += 1;
    }
    for spec in STD_FUNCTIONS {
        registry.register_function(spec.def())?;
    }
    for spec in STD_METHODS {
        registry.register_method(spec.def())?;
    }

    Ok(StdlibRegistration {
        types: STD_TYPES.len(),
        variants: STD_VARIANTS.len(),
        fields: STD_FIELDS.len(),
        functions: STD_FUNCTIONS.len(),
        methods: STD_METHODS.len(),
    })
}

pub fn standard_registry() -> Result<DefinitionRegistry, RegistryError> {
    let mut registry = DefinitionRegistry::new();
    register_stdlib(&mut registry)?;
    Ok(registry)
}
