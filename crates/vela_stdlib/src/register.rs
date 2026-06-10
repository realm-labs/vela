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
    for spec in STD_VARIANTS {
        registry.register_variant(spec.def())?;
    }
    for spec in STD_FIELDS {
        registry.register_field(spec.def())?;
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
