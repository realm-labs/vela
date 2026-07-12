use std::collections::BTreeMap;
use std::ops::Range;

use toml_edit::{Document, Item, TableLike, Value};
use vela_common::{Capability, CapabilitySet};

use crate::{PackageAlias, PackageId, PackageName, PackageVersion};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ManifestFileId(u32);

impl ManifestFileId {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManifestSpan {
    pub file: ManifestFileId,
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestDiagnostic {
    pub message: String,
    pub span: ManifestSpan,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WorkspaceManifest {
    pub members: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageMetadata {
    pub id: PackageId,
    pub name: PackageName,
    pub version: PackageVersion,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceManifest {
    pub roots: Vec<String>,
}

impl Default for SourceManifest {
    fn default() -> Self {
        Self {
            roots: vec!["src".to_owned()],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyManifest {
    pub path: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HostManifest {
    pub schema: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PackageManifest {
    pub workspace: Option<WorkspaceManifest>,
    pub package: Option<PackageMetadata>,
    pub source: SourceManifest,
    pub dependencies: BTreeMap<PackageAlias, DependencyManifest>,
    pub required_capabilities: CapabilitySet,
    pub host: Option<HostManifest>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestParse {
    pub manifest: Option<PackageManifest>,
    pub diagnostics: Vec<ManifestDiagnostic>,
}

#[must_use]
pub fn parse_manifest(file: ManifestFileId, text: &str) -> ManifestParse {
    let document = match Document::parse(text.to_owned()) {
        Ok(document) => document,
        Err(error) => {
            let range = error.span().unwrap_or(0..text.len());
            return ManifestParse {
                manifest: None,
                diagnostics: vec![diagnostic(file, range, error.message())],
            };
        }
    };
    let mut diagnostics = Vec::new();
    reject_unknown_keys(
        file,
        document.as_table(),
        &[
            "workspace",
            "package",
            "source",
            "dependencies",
            "capabilities",
            "host",
        ],
        &mut diagnostics,
    );

    let workspace = table(&document, "workspace").map(|value| {
        reject_unknown_keys(file, value, &["members"], &mut diagnostics);
        WorkspaceManifest {
            members: string_array(
                file,
                value.get("members"),
                "workspace.members",
                &mut diagnostics,
            )
            .unwrap_or_default(),
        }
    });
    let package = table(&document, "package").and_then(|value| {
        reject_unknown_keys(file, value, &["id", "name", "version"], &mut diagnostics);
        let id = validated_string(
            file,
            value.get("id"),
            "package.id",
            |text| PackageId::new(text),
            &mut diagnostics,
        )?;
        let name = validated_string(
            file,
            value.get("name"),
            "package.name",
            |text| PackageName::new(text),
            &mut diagnostics,
        )?;
        let version = validated_string(
            file,
            value.get("version"),
            "package.version",
            |text| PackageVersion::new(text),
            &mut diagnostics,
        )?;
        Some(PackageMetadata { id, name, version })
    });
    let source = table(&document, "source").map_or_else(SourceManifest::default, |value| {
        reject_unknown_keys(file, value, &["roots"], &mut diagnostics);
        SourceManifest {
            roots: string_array(file, value.get("roots"), "source.roots", &mut diagnostics)
                .unwrap_or_default(),
        }
    });
    let dependencies = parse_dependencies(file, table(&document, "dependencies"), &mut diagnostics);
    let required_capabilities =
        parse_capabilities(file, table(&document, "capabilities"), &mut diagnostics);
    let host = table(&document, "host").map(|value| {
        reject_unknown_keys(file, value, &["schema"], &mut diagnostics);
        HostManifest {
            schema: optional_string(file, value.get("schema"), "host.schema", &mut diagnostics),
        }
    });
    let valid = diagnostics.is_empty();
    ManifestParse {
        manifest: valid.then_some(PackageManifest {
            workspace,
            package,
            source,
            dependencies,
            required_capabilities,
            host,
        }),
        diagnostics,
    }
}

fn table<'a>(document: &'a Document<String>, key: &str) -> Option<&'a dyn TableLike> {
    document.get(key).and_then(Item::as_table_like)
}

fn reject_unknown_keys(
    file: ManifestFileId,
    table: &dyn TableLike,
    allowed: &[&str],
    diagnostics: &mut Vec<ManifestDiagnostic>,
) {
    for (key, item) in table.iter() {
        if !allowed.contains(&key) {
            diagnostics.push(diagnostic(
                file,
                table
                    .key(key)
                    .and_then(toml_edit::Key::span)
                    .or_else(|| item.span())
                    .unwrap_or(0..0),
                format!("unknown vela.toml key `{key}`"),
            ));
        }
    }
}

fn parse_dependencies(
    file: ManifestFileId,
    table: Option<&dyn TableLike>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> BTreeMap<PackageAlias, DependencyManifest> {
    let mut dependencies = BTreeMap::new();
    let Some(table) = table else {
        return dependencies;
    };
    for (name, item) in table.iter() {
        let alias = match PackageAlias::new(name) {
            Ok(alias) => alias,
            Err(error) => {
                diagnostics.push(diagnostic(
                    file,
                    item.span().unwrap_or(0..0),
                    error.to_string(),
                ));
                continue;
            }
        };
        let Some(inline) = item.as_inline_table() else {
            diagnostics.push(diagnostic(
                file,
                item.span().unwrap_or(0..0),
                "dependency must be an inline table with a path",
            ));
            continue;
        };
        for (key, value) in inline.iter() {
            if key != "path" {
                diagnostics.push(diagnostic(
                    file,
                    value.span().unwrap_or(0..0),
                    format!("unknown dependency key `{key}`"),
                ));
            }
        }
        if let Some(path) = inline.get("path").and_then(value_string) {
            dependencies.insert(
                alias,
                DependencyManifest {
                    path: path.to_owned(),
                },
            );
        } else {
            diagnostics.push(diagnostic(
                file,
                item.span().unwrap_or(0..0),
                "dependency.path must be a string",
            ));
        }
    }
    dependencies
}

fn parse_capabilities(
    file: ManifestFileId,
    table: Option<&dyn TableLike>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> CapabilitySet {
    let Some(table) = table else {
        return CapabilitySet::new();
    };
    reject_unknown_keys(file, table, &["requires"], diagnostics);
    let Some(names) = string_array(
        file,
        table.get("requires"),
        "capabilities.requires",
        diagnostics,
    ) else {
        return CapabilitySet::new();
    };
    let mut result = CapabilitySet::new();
    for name in names {
        match Capability::from_name(&name) {
            Some(capability) => result.insert(capability),
            None => diagnostics.push(diagnostic(
                file,
                table.get("requires").and_then(Item::span).unwrap_or(0..0),
                format!("unknown capability `{name}`"),
            )),
        }
    }
    result
}

fn validated_string<T, E: std::fmt::Display>(
    file: ManifestFileId,
    item: Option<&Item>,
    name: &str,
    validate: impl FnOnce(&str) -> Result<T, E>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<T> {
    let value = optional_string(file, item, name, diagnostics)?;
    match validate(&value) {
        Ok(value) => Some(value),
        Err(error) => {
            diagnostics.push(diagnostic(
                file,
                item.and_then(Item::span).unwrap_or(0..0),
                error.to_string(),
            ));
            None
        }
    }
}

fn optional_string(
    file: ManifestFileId,
    item: Option<&Item>,
    name: &str,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<String> {
    let item = item?;
    match item.as_str() {
        Some(value) => Some(value.to_owned()),
        None => {
            diagnostics.push(diagnostic(
                file,
                item.span().unwrap_or(0..0),
                format!("{name} must be a string"),
            ));
            None
        }
    }
}

fn string_array(
    file: ManifestFileId,
    item: Option<&Item>,
    name: &str,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<Vec<String>> {
    let item = item?;
    let Some(array) = item.as_array() else {
        diagnostics.push(diagnostic(
            file,
            item.span().unwrap_or(0..0),
            format!("{name} must be an array of strings"),
        ));
        return None;
    };
    let mut values = Vec::with_capacity(array.len());
    for value in array {
        if let Some(value) = value_string(value) {
            values.push(value.to_owned());
        } else {
            diagnostics.push(diagnostic(
                file,
                value.span().unwrap_or(0..0),
                format!("{name} must contain only strings"),
            ));
        }
    }
    Some(values)
}

fn value_string(value: &Value) -> Option<&str> {
    value.as_str()
}

fn diagnostic(
    file: ManifestFileId,
    range: Range<usize>,
    message: impl Into<String>,
) -> ManifestDiagnostic {
    ManifestDiagnostic {
        message: message.into(),
        span: ManifestSpan {
            file,
            start: u32::try_from(range.start).unwrap_or(u32::MAX),
            end: u32::try_from(range.end).unwrap_or(u32::MAX),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_parses_workspace_package_sources_dependencies_and_capabilities() {
        let parsed = parse_manifest(
            ManifestFileId::new(1),
            r#"
[workspace]
members = ["plugins/sort"]
[package]
id = "com.example.app"
name = "app"
version = "0.1.0"
[source]
roots = ["src", "generated"]
[dependencies]
utils = { path = "../utils" }
[capabilities]
requires = ["host_read", "time"]
[host]
schema = "target/schema.json"
"#,
        );
        let manifest = parsed.manifest.expect("valid manifest");
        assert_eq!(
            manifest.workspace.expect("workspace").members,
            ["plugins/sort"]
        );
        assert_eq!(
            manifest.package.expect("package").id.as_str(),
            "com.example.app"
        );
        assert_eq!(manifest.source.roots, ["src", "generated"]);
        assert_eq!(manifest.dependencies.len(), 1);
        assert!(
            manifest
                .required_capabilities
                .contains(Capability::HostRead)
        );
        assert!(manifest.required_capabilities.contains(Capability::Time));
    }

    #[test]
    fn manifest_reports_unknown_keys_with_spans() {
        let parsed = parse_manifest(
            ManifestFileId::new(7),
            "[package]\nid = \"com.example.app\"\nunknown = true\n",
        );
        assert!(parsed.manifest.is_none());
        assert_eq!(parsed.diagnostics[0].span.file, ManifestFileId::new(7));
        assert!(parsed.diagnostics[0].span.end > parsed.diagnostics[0].span.start);
    }

    #[test]
    fn manifest_and_engine_use_the_same_capability_ids() {
        let parsed = parse_manifest(
            ManifestFileId::new(1),
            "[capabilities]\nrequires=[\"host_write\",\"reflection_call\"]\n",
        );
        let capabilities = parsed.manifest.expect("manifest").required_capabilities;
        let expected = CapabilitySet::new()
            .with(Capability::HostWrite)
            .with(Capability::ReflectionCall);
        assert_eq!(capabilities.bits(), expected.bits());
    }
}
