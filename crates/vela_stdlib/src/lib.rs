//! Semantic standard-library manifest and registry installation.

mod manifest;
mod methods;
mod register;

pub use manifest::{
    STD_FIELDS, STD_FUNCTIONS, STD_TYPES, STD_VARIANTS, StdFieldSpec, StdFunctionSpec,
    StdMethodSpec, StdParamSpec, StdTypeSpec, StdVariantSpec,
};
pub use methods::STD_METHODS;
pub use register::{StdlibRegistration, register_stdlib, standard_registry};
