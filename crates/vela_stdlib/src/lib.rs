//! Semantic standard-library manifest and registry installation.

mod ids;
mod manifest;
mod methods;
mod register;

pub use ids::{std_field_id, std_function_id, std_method_id, std_type_id, std_variant_id};
pub use manifest::{
    STD_FIELDS, STD_FUNCTIONS, STD_TYPES, STD_VARIANTS, StdFieldSpec, StdFunctionSpec,
    StdMethodSpec, StdParamSpec, StdTypeSpec, StdVariantSpec,
};
pub use methods::STD_METHODS;
pub use register::{StdlibRegistration, register_stdlib, standard_registry};
