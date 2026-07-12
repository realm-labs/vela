#![cfg_attr(not(test), deny(clippy::wildcard_imports))]

mod graph;
mod identity;
mod manifest;

pub use graph::{
    PackageDescriptor, PackageGraph, PackageGraphError, PackageSource, SourceTable,
    load_package_graph,
};
pub use identity::{
    IdentityError, ModuleKey, ModulePath, PackageAlias, PackageId, PackageName, PackageVersion,
};
pub use manifest::{
    DependencyManifest, HostManifest, ManifestDiagnostic, ManifestFileId, ManifestParse,
    ManifestSpan, PackageManifest, PackageMetadata, SourceManifest, WorkspaceManifest,
    parse_manifest,
};
