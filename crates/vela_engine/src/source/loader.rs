use std::fs;
use std::path::{Path, PathBuf};

use vela_common::SourceId;
use vela_hir::module_graph::{ModulePath, ModuleSource};

use super::EngineSourceError;

const SOURCE_EXTENSION: &str = "vela";

pub(crate) fn read_source_text(path: &Path) -> Result<String, EngineSourceError> {
    fs::read_to_string(path).map_err(|error| EngineSourceError::io(path, error))
}

pub(crate) fn load_module_sources(root: &Path) -> Result<Vec<ModuleSource>, EngineSourceError> {
    let mut files = Vec::new();
    collect_source_files(root, &mut files)?;
    files.sort();

    let mut sources = Vec::with_capacity(files.len());
    for (index, path) in files.iter().enumerate() {
        let source_id = source_id(index, files.len())?;
        let text = read_source_text(path)?;
        let module_path = module_path(root, path)?;
        sources.push(ModuleSource::new(source_id, module_path, text));
    }
    Ok(sources)
}

fn collect_source_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), EngineSourceError> {
    let entries = fs::read_dir(root).map_err(|error| EngineSourceError::io(root, error))?;
    for entry in entries {
        let entry = entry.map_err(|error| EngineSourceError::io(root, error))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| EngineSourceError::io(&path, error))?;
        if file_type.is_dir() {
            collect_source_files(&path, files)?;
        } else if file_type.is_file()
            && path.extension().and_then(|ext| ext.to_str()) == Some(SOURCE_EXTENSION)
        {
            files.push(path);
        }
    }
    Ok(())
}

fn source_id(index: usize, count: usize) -> Result<SourceId, EngineSourceError> {
    let raw = index
        .checked_add(1)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| EngineSourceError::too_many_sources(count))?;
    Ok(SourceId::new(raw))
}

fn module_path(root: &Path, path: &Path) -> Result<ModulePath, EngineSourceError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| EngineSourceError::invalid_path(path))?;
    let components = relative.components().collect::<Vec<_>>();
    let mut segments = Vec::new();
    for (index, component) in components.iter().enumerate() {
        let component_path = component.as_os_str();
        let segment = if index + 1 == components.len() {
            Path::new(component_path)
                .file_stem()
                .and_then(|stem| stem.to_str())
        } else {
            component_path.to_str()
        };
        let Some(segment) = segment else {
            return Err(EngineSourceError::invalid_path(path));
        };
        if !segment.is_empty() {
            segments.push(segment.to_owned());
        }
    }
    if segments.is_empty() {
        return Err(EngineSourceError::invalid_path(path));
    }
    Ok(ModulePath::new(segments))
}
