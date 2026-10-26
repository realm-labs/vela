//! Version 1 document slots accept inline text or exactly {"sourceFile": "sources/..."}.
//! Keep the finite slot list in sync with scripts/lsp-matrix/fixture-sources.js.
use std::path::Path;

use serde_json::Value;

fn source_text(value: &mut Value, root: &Path, nullable: bool) -> Result<(), String> {
    if value.is_string() || (nullable && value.is_null()) {
        return Ok(());
    }
    let reference = value
        .as_object()
        .filter(|object| object.len() == 1)
        .and_then(|object| object.get("sourceFile"))
        .and_then(Value::as_str)
        .ok_or("fixture source must be text or exactly { sourceFile: string }")?;
    let parts: Vec<_> = reference.split('/').collect();
    if parts.first() != Some(&"sources")
        || parts.len() < 2
        || reference.contains(['\\', ':', '\0'])
        || parts.iter().any(|part| matches!(*part, "" | "." | ".."))
    {
        return Err(format!("invalid fixture source path: {reference}"));
    }
    let mut target = root.canonicalize().map_err(|error| error.to_string())?;
    for part in parts {
        target.push(part);
        // Even internal symlinks evade the regular-file raw provenance walk.
        // Fixture trees are trusted inputs, not concurrently mutable sandboxes.
        if std::fs::symlink_metadata(&target)
            .map_err(|error| error.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err(format!("fixture source symlink: {reference}"));
        }
    }
    if !target.is_file() {
        return Err(format!("fixture source is not a file: {reference}"));
    }
    *value = Value::String(std::fs::read_to_string(target).map_err(|error| error.to_string())?);
    Ok(())
}

// Traversal is bounded by these authored schema routes, never by keys in JSON.
fn slot(value: &mut Value, keys: &[&str], root: &Path, nullable: bool) -> Result<(), String> {
    let Some((key, rest)) = keys.split_first() else {
        return source_text(value, root, nullable);
    };
    if *key == "*" {
        match value {
            Value::Array(values) => {
                for value in values {
                    slot(value, rest, root, nullable)?;
                }
            }
            Value::Object(values) => {
                for value in values.values_mut() {
                    slot(value, rest, root, nullable)?;
                }
            }
            _ => return Err("invalid fixture source container".into()),
        }
    } else if let Some(child) = value.get_mut(*key) {
        slot(child, rest, root, nullable)?;
    }
    Ok(())
}

pub(super) fn load_json(file: &Path) -> Result<Value, String> {
    let mut spec: Value =
        serde_json::from_str(&std::fs::read_to_string(file).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let root = file.parent().ok_or("fixture needs a parent directory")?;
    let id = spec["id"].as_str().unwrap_or_default().to_owned();
    let mut resolve = |route: &str, nullable| {
        slot(
            &mut spec,
            &route.split('/').collect::<Vec<_>>(),
            root,
            nullable,
        )
    };
    resolve("files/*", false)?;
    resolve("actions/*/source", true)?;
    if matches!(
        id.as_str(),
        "completion-source-callable-lifecycle"
            | "completion-source-overlay-lifecycle"
            | "completion-source-recovery"
    ) {
        resolve("oracle/sourceLifecycle/*/files/*", true)?;
        for key in ["action/source", "diskSource", "openSource"] {
            resolve(&format!("oracle/sourceLifecycle/*/{key}"), true)?;
        }
    }
    if id.starts_with("hover-") || id.starts_with("signature-") {
        resolve("oracle/phases/*/actions/*/source", true)?;
    }
    if id == "signature-editor" {
        resolve("oracle/recovery/source", false)?;
    }
    if id == "document-change" {
        resolve("oracle/cases/*/steps/*/text", false)?;
    }
    if id.starts_with("selection-")
        || id.starts_with("document-symbol-")
        || id.starts_with("workspace-symbol-")
    {
        resolve("oracle/cases/*/source", false)?;
        resolve("oracle/variants/*/source", false)?;
    }
    if id.starts_with("semantic-token-") {
        for key in [
            "negative/source",
            "negative/cases/*/source",
            "states/*/source",
            "states/*/queries/*/source",
            "initial/*/source",
            "steps/*/queries/*/source",
        ] {
            resolve(&format!("oracle/{key}"), false)?;
        }
        resolve("oracle/states/*/diskSources/*", true)?;
        resolve("oracle/states/*/openSources/*", true)?;
        resolve("oracle/states/*/action/source", true)?;
        resolve("oracle/steps/*/source", true)?;
    }
    if id == "input-semantic-tokens" {
        resolve("oracle/shifted", false)?;
        resolve("oracle/unknown", false)?;
    }
    Ok(spec)
}

#[cfg(test)]
#[path = "fixture_sources_tests.rs"]
mod tests;
