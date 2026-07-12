use super::*;
use crate::binding::{BindingResolution, ConstructorResolution, LocalBindingKind};
use crate::type_hint::{EnumVariantFieldsHint, ImplMetadataKind};
fn source(id: u32, module: &str, text: &str) -> ModuleSource {
    ModuleSource::new(
        SourceId::new(id),
        vela_package::PackageId::anonymous(),
        ModulePath::from_qualified(module),
        text,
    )
}

fn package_source(id: u32, package: &PackageId, module: &str, text: &str) -> ModuleSource {
    ModuleSource::new(
        SourceId::new(id),
        package.clone(),
        ModulePath::from_qualified(module),
        text,
    )
}

mod bindings;
mod ingestion;
mod metadata;
mod resolution;
