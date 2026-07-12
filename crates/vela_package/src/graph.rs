use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use vela_common::CapabilitySet;

use crate::{
    ManifestDiagnostic, ManifestFileId, ModulePath, PackageAlias, PackageId, PackageManifest,
    PackageName, PackageVersion, parse_manifest,
};

const MANIFEST_NAME: &str = "vela.toml";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageSource {
    pub package: PackageId,
    pub module: ModulePath,
    pub path: PathBuf,
    pub text: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SourceTable {
    manifests: BTreeMap<ManifestFileId, PathBuf>,
    sources: Vec<PackageSource>,
}

impl SourceTable {
    #[must_use]
    pub fn manifest_path(&self, file: ManifestFileId) -> Option<&Path> {
        self.manifests.get(&file).map(PathBuf::as_path)
    }

    #[must_use]
    pub fn sources(&self) -> &[PackageSource] {
        &self.sources
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageDescriptor {
    pub id: PackageId,
    pub name: PackageName,
    pub version: PackageVersion,
    pub manifest_path: PathBuf,
    pub root: PathBuf,
    pub source_roots: Vec<PathBuf>,
    pub required_capabilities: CapabilitySet,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PackageGraph {
    packages: BTreeMap<PackageId, PackageDescriptor>,
    dependencies: BTreeMap<PackageId, BTreeMap<PackageAlias, PackageId>>,
    sources: SourceTable,
    workspace_members: BTreeSet<PackageId>,
}

impl PackageGraph {
    #[must_use]
    pub fn packages(&self) -> &BTreeMap<PackageId, PackageDescriptor> {
        &self.packages
    }

    #[must_use]
    pub fn dependencies(&self, package: &PackageId) -> Option<&BTreeMap<PackageAlias, PackageId>> {
        self.dependencies.get(package)
    }

    #[must_use]
    pub const fn dependency_map(&self) -> &BTreeMap<PackageId, BTreeMap<PackageAlias, PackageId>> {
        &self.dependencies
    }

    #[must_use]
    pub const fn sources(&self) -> &SourceTable {
        &self.sources
    }

    #[must_use]
    pub fn workspace_members(&self) -> &BTreeSet<PackageId> {
        &self.workspace_members
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PackageGraphError {
    Io {
        path: PathBuf,
        message: String,
    },
    Manifest {
        path: PathBuf,
        diagnostics: Vec<ManifestDiagnostic>,
    },
    MissingPackage {
        path: PathBuf,
    },
    HostConfigurationInDependency {
        path: PathBuf,
    },
    UnauthorizedPath {
        path: PathBuf,
    },
    DuplicatePackageId {
        id: PackageId,
        first: PathBuf,
        second: PathBuf,
    },
    DependencyCycle {
        manifests: Vec<PathBuf>,
    },
}

impl fmt::Display for PackageGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, message } => write!(formatter, "{}: {message}", path.display()),
            Self::Manifest { path, .. } => {
                write!(formatter, "invalid manifest `{}`", path.display())
            }
            Self::MissingPackage { path } => write!(
                formatter,
                "manifest `{}` has no [package] table",
                path.display()
            ),
            Self::HostConfigurationInDependency { path } => write!(
                formatter,
                "dependency manifest `{}` cannot contain a [host] table",
                path.display()
            ),
            Self::UnauthorizedPath { path } => write!(
                formatter,
                "path `{}` escapes the authorized roots",
                path.display()
            ),
            Self::DuplicatePackageId { id, first, second } => write!(
                formatter,
                "package id `{id}` is declared by both `{}` and `{}`",
                first.display(),
                second.display()
            ),
            Self::DependencyCycle { manifests } => write!(
                formatter,
                "dependency cycle: {}",
                manifests
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(" -> ")
            ),
        }
    }
}

impl std::error::Error for PackageGraphError {}

pub fn load_package_graph(
    root_manifest: impl AsRef<Path>,
    authorized_roots: &[PathBuf],
) -> Result<PackageGraph, PackageGraphError> {
    let root_manifest = canonicalize(root_manifest.as_ref())?;
    let authorized_roots = authorized_roots
        .iter()
        .map(|root| canonicalize(root))
        .collect::<Result<Vec<_>, _>>()?;
    authorize(&root_manifest, &authorized_roots)?;
    let mut builder = GraphBuilder {
        graph: PackageGraph::default(),
        manifests_by_path: BTreeMap::new(),
        manifests_by_package: BTreeMap::new(),
        active: Vec::new(),
        authorized_roots,
        root_manifest: root_manifest.clone(),
        next_manifest_file: 1,
    };
    let root = builder.read_manifest(&root_manifest)?;
    let member_manifests = root
        .manifest
        .workspace
        .as_ref()
        .map_or_else(Vec::new, |workspace| {
            workspace
                .members
                .iter()
                .map(|member| root.root.join(member).join(MANIFEST_NAME))
                .collect()
        });
    if root.manifest.package.is_some() {
        let id = builder.load_package(root_manifest.clone())?;
        builder.graph.workspace_members.insert(id);
    }
    for member in member_manifests {
        let id = builder.load_package(canonicalize(&member)?)?;
        builder.graph.workspace_members.insert(id);
    }
    Ok(builder.graph)
}

#[derive(Clone)]
struct LoadedManifest {
    manifest: PackageManifest,
    path: PathBuf,
    root: PathBuf,
}

struct GraphBuilder {
    graph: PackageGraph,
    manifests_by_path: BTreeMap<PathBuf, LoadedManifest>,
    manifests_by_package: BTreeMap<PackageId, PathBuf>,
    active: Vec<PathBuf>,
    authorized_roots: Vec<PathBuf>,
    root_manifest: PathBuf,
    next_manifest_file: u32,
}

impl GraphBuilder {
    fn read_manifest(&mut self, path: &Path) -> Result<LoadedManifest, PackageGraphError> {
        if let Some(manifest) = self.manifests_by_path.get(path) {
            return Ok(manifest.clone());
        }
        authorize(path, &self.authorized_roots)?;
        let text = fs::read_to_string(path).map_err(|error| PackageGraphError::Io {
            path: path.to_owned(),
            message: error.to_string(),
        })?;
        let file = ManifestFileId::new(self.next_manifest_file);
        self.next_manifest_file = self.next_manifest_file.saturating_add(1);
        let parsed = parse_manifest(file, &text);
        let Some(manifest) = parsed.manifest else {
            return Err(PackageGraphError::Manifest {
                path: path.to_owned(),
                diagnostics: parsed.diagnostics,
            });
        };
        let root = path.parent().unwrap_or_else(|| Path::new(".")).to_owned();
        let loaded = LoadedManifest {
            manifest,
            path: path.to_owned(),
            root,
        };
        self.graph.sources.manifests.insert(file, path.to_owned());
        self.manifests_by_path
            .insert(path.to_owned(), loaded.clone());
        Ok(loaded)
    }

    fn load_package(&mut self, path: PathBuf) -> Result<PackageId, PackageGraphError> {
        if let Some(index) = self.active.iter().position(|active| active == &path) {
            let mut manifests = self.active[index..].to_vec();
            manifests.push(path);
            return Err(PackageGraphError::DependencyCycle { manifests });
        }
        if let Some(loaded) = self.manifests_by_path.get(&path)
            && let Some(package) = &loaded.manifest.package
            && self.graph.packages.contains_key(&package.id)
        {
            return Ok(package.id.clone());
        }
        self.active.push(path.clone());
        let loaded = self.read_manifest(&path)?;
        if path != self.root_manifest && loaded.manifest.host.is_some() {
            return Err(PackageGraphError::HostConfigurationInDependency { path });
        }
        let metadata = loaded
            .manifest
            .package
            .clone()
            .ok_or_else(|| PackageGraphError::MissingPackage { path: path.clone() })?;
        if let Some(first) = self.manifests_by_package.get(&metadata.id)
            && first != &path
        {
            return Err(PackageGraphError::DuplicatePackageId {
                id: metadata.id,
                first: first.clone(),
                second: path,
            });
        }
        self.manifests_by_package
            .insert(metadata.id.clone(), path.clone());
        let source_roots = loaded
            .manifest
            .source
            .roots
            .iter()
            .map(|root| canonicalize_existing_or_parent(&loaded.root.join(root)))
            .collect::<Result<Vec<_>, _>>()?;
        for root in &source_roots {
            authorize(root, &self.authorized_roots)?;
        }
        let descriptor = PackageDescriptor {
            id: metadata.id.clone(),
            name: metadata.name,
            version: metadata.version,
            manifest_path: loaded.path.clone(),
            root: loaded.root.clone(),
            source_roots: source_roots.clone(),
            required_capabilities: loaded.manifest.required_capabilities,
        };
        self.graph.packages.insert(metadata.id.clone(), descriptor);
        self.discover_sources(&metadata.id, &source_roots)?;
        let mut dependencies = BTreeMap::new();
        for (alias, dependency) in &loaded.manifest.dependencies {
            let dependency_manifest =
                canonicalize(&loaded.root.join(&dependency.path).join(MANIFEST_NAME))?;
            let dependency_id = self.load_package(dependency_manifest)?;
            dependencies.insert(alias.clone(), dependency_id);
        }
        self.graph
            .dependencies
            .insert(metadata.id.clone(), dependencies);
        self.active.pop();
        Ok(metadata.id)
    }

    fn discover_sources(
        &mut self,
        package: &PackageId,
        roots: &[PathBuf],
    ) -> Result<(), PackageGraphError> {
        let mut paths = Vec::new();
        for root in roots {
            collect_vela_files(root, &mut paths)?;
        }
        paths.sort();
        for path in paths {
            let root = roots
                .iter()
                .filter(|root| path.starts_with(root))
                .max_by_key(|root| root.components().count())
                .expect("source has an owning root");
            let relative = path
                .strip_prefix(root)
                .expect("authorized source is beneath root");
            let module = module_path(relative).ok_or_else(|| PackageGraphError::Io {
                path: path.clone(),
                message: "invalid UTF-8 Vela source path".to_owned(),
            })?;
            let text = fs::read_to_string(&path).map_err(|error| PackageGraphError::Io {
                path: path.clone(),
                message: error.to_string(),
            })?;
            self.graph.sources.sources.push(PackageSource {
                package: package.clone(),
                module,
                path,
                text,
            });
        }
        self.graph.sources.sources.sort_by(|left, right| {
            (&left.package, &left.module, &left.path).cmp(&(
                &right.package,
                &right.module,
                &right.path,
            ))
        });
        Ok(())
    }
}

fn canonicalize(path: &Path) -> Result<PathBuf, PackageGraphError> {
    fs::canonicalize(path).map_err(|error| PackageGraphError::Io {
        path: path.to_owned(),
        message: error.to_string(),
    })
}

fn canonicalize_existing_or_parent(path: &Path) -> Result<PathBuf, PackageGraphError> {
    if path.exists() {
        return canonicalize(path);
    }
    let parent = path.parent().ok_or_else(|| PackageGraphError::Io {
        path: path.to_owned(),
        message: "path has no existing ancestor".to_owned(),
    })?;
    let name = path.file_name().ok_or_else(|| PackageGraphError::Io {
        path: path.to_owned(),
        message: "path has no final component".to_owned(),
    })?;
    Ok(canonicalize_existing_or_parent(parent)?.join(name))
}

fn authorize(path: &Path, roots: &[PathBuf]) -> Result<(), PackageGraphError> {
    if roots.iter().any(|root| path.starts_with(root)) {
        Ok(())
    } else {
        Err(PackageGraphError::UnauthorizedPath {
            path: path.to_owned(),
        })
    }
}

fn collect_vela_files(root: &Path, output: &mut Vec<PathBuf>) -> Result<(), PackageGraphError> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(PackageGraphError::Io {
                path: root.to_owned(),
                message: error.to_string(),
            });
        }
    };
    for entry in entries {
        let entry = entry.map_err(|error| PackageGraphError::Io {
            path: root.to_owned(),
            message: error.to_string(),
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| PackageGraphError::Io {
            path: path.clone(),
            message: error.to_string(),
        })?;
        if file_type.is_dir() {
            collect_vela_files(&path, output)?;
        } else if file_type.is_file()
            && path.extension().and_then(|extension| extension.to_str()) == Some("vela")
        {
            output.push(path);
        }
    }
    Ok(())
}

fn module_path(relative: &Path) -> Option<ModulePath> {
    let mut segments = relative
        .parent()
        .into_iter()
        .flat_map(Path::components)
        .map(|component| component.as_os_str().to_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()?;
    segments.push(relative.file_stem()?.to_str()?.to_owned());
    Some(ModulePath::new(segments))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_DIR: AtomicU64 = AtomicU64::new(1);

    fn fixture(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "vela_package_{name}_{}_{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).expect("create fixture");
        path
    }

    fn write(path: &Path, text: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, text).expect("write fixture");
    }

    #[test]
    fn path_dependency_resolves_relative_to_manifest_and_sources_are_deterministic() {
        let root = fixture("dependency");
        write(
            &root.join("vela.toml"),
            "[package]\nid=\"com.example.app\"\nname=\"app\"\nversion=\"0.1.0\"\n[dependencies]\nutil={path=\"util\"}\n",
        );
        write(&root.join("src/z.vela"), "fn z() {}\n");
        write(&root.join("src/a.vela"), "fn a() {}\n");
        write(
            &root.join("util/vela.toml"),
            "[package]\nid=\"com.example.util\"\nname=\"util\"\nversion=\"0.1.0\"\n",
        );
        write(
            &root.join("util/src/lib.vela"),
            "pub fn value() { return 1 }\n",
        );
        let graph =
            load_package_graph(root.join("vela.toml"), std::slice::from_ref(&root)).expect("graph");
        let app = PackageId::new("com.example.app").expect("id");
        assert_eq!(
            graph
                .dependencies(&app)
                .expect("dependencies")
                .values()
                .next()
                .expect("dependency")
                .as_str(),
            "com.example.util"
        );
        let modules = graph
            .sources()
            .sources()
            .iter()
            .map(|source| format!("{}:{}", source.package, source.module.join()))
            .collect::<Vec<_>>();
        assert_eq!(
            modules,
            [
                "com.example.app:a",
                "com.example.app:z",
                "com.example.util:lib"
            ]
        );
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn source_root_cannot_escape_authorized_package_root() {
        let parent = fixture("escape");
        let root = parent.join("app");
        fs::create_dir_all(&root).expect("app root");
        write(&parent.join("outside/main.vela"), "fn main() {}\n");
        write(
            &root.join("vela.toml"),
            "[package]\nid=\"com.example.app\"\nname=\"app\"\nversion=\"0.1.0\"\n[source]\nroots=[\"../outside\"]\n",
        );
        let error = load_package_graph(root.join("vela.toml"), std::slice::from_ref(&root))
            .expect_err("escape rejected");
        assert!(matches!(error, PackageGraphError::UnauthorizedPath { .. }));
        fs::remove_dir_all(parent).expect("remove fixture");
    }

    #[test]
    fn duplicate_package_id_at_different_manifests_is_rejected() {
        let root = fixture("duplicate_id");
        write(
            &root.join("vela.toml"),
            "[workspace]\nmembers=[\"a\",\"b\"]\n",
        );
        for member in ["a", "b"] {
            write(
                &root.join(member).join("vela.toml"),
                "[package]\nid=\"com.example.same\"\nname=\"same\"\nversion=\"0.1.0\"\n",
            );
            write(&root.join(member).join("src/main.vela"), "fn main() {}\n");
        }
        let error = load_package_graph(root.join("vela.toml"), std::slice::from_ref(&root))
            .expect_err("duplicate rejected");
        assert!(matches!(
            error,
            PackageGraphError::DuplicatePackageId { .. }
        ));
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn dependency_cycle_reports_manifest_edge_chain() {
        let root = fixture("cycle");
        write(
            &root.join("a/vela.toml"),
            "[package]\nid=\"com.example.a\"\nname=\"a\"\nversion=\"0.1.0\"\n[dependencies]\nb={path=\"../b\"}\n",
        );
        write(&root.join("a/src/main.vela"), "fn main() {}\n");
        write(
            &root.join("b/vela.toml"),
            "[package]\nid=\"com.example.b\"\nname=\"b\"\nversion=\"0.1.0\"\n[dependencies]\na={path=\"../a\"}\n",
        );
        write(&root.join("b/src/main.vela"), "fn main() {}\n");
        let error = load_package_graph(root.join("a/vela.toml"), std::slice::from_ref(&root))
            .expect_err("cycle rejected");
        let PackageGraphError::DependencyCycle { manifests } = error else {
            panic!("expected cycle")
        };
        assert_eq!(manifests.len(), 3);
        assert_eq!(manifests.first(), manifests.last());
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn dependency_manifest_cannot_override_host_configuration() {
        let root = fixture("dependency_host");
        write(
            &root.join("vela.toml"),
            "[package]\nid=\"com.example.app\"\nname=\"app\"\nversion=\"0.1.0\"\n[dependencies]\nutil={path=\"util\"}\n",
        );
        write(&root.join("src/main.vela"), "fn main() {}\n");
        write(
            &root.join("util/vela.toml"),
            "[package]\nid=\"com.example.util\"\nname=\"util\"\nversion=\"0.1.0\"\n[host]\nschema=\"schema.json\"\n",
        );
        write(&root.join("util/src/lib.vela"), "fn value() {}\n");
        let error = load_package_graph(root.join("vela.toml"), std::slice::from_ref(&root))
            .expect_err("dependency host config rejected");
        assert!(matches!(
            error,
            PackageGraphError::HostConfigurationInDependency { .. }
        ));
        fs::remove_dir_all(root).expect("remove fixture");
    }
}
