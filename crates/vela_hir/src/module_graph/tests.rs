use super::*;
use crate::binding::{BindingResolution, LocalBindingKind};
use crate::type_hint::{EnumVariantFieldsHint, ImplMetadataKind};
fn source(id: u32, module: &str, text: &str) -> ModuleSource {
    ModuleSource::new(SourceId::new(id), ModulePath::from_qualified(module), text)
}

mod bindings;
mod metadata;
mod resolution;
