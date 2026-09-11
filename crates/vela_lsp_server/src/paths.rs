use std::path::{Path, PathBuf};

pub(crate) const CONFIG_FILE: &str = "vela.toml";
pub(crate) const SOURCE_EXTENSION: &str = ".vela";

pub(crate) fn document_path_uri(path: &str) -> String {
    if let Ok(uri) = lsp_types::Url::from_file_path(path) {
        return uri.to_string();
    }
    let path = normalized_path(path);
    if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    }
}

pub(crate) fn document_uri_path(uri: &str) -> PathBuf {
    if let Ok(uri) = lsp_types::Url::parse(uri)
        && let Ok(path) = uri.to_file_path()
    {
        return path;
    }
    let path = uri.strip_prefix("file://").unwrap_or(uri);
    if cfg!(windows) {
        let path = path.replace('/', "\\");
        let path = path
            .strip_prefix("\\")
            .filter(|path| path.as_bytes().get(1) == Some(&b':'))
            .unwrap_or(&path);
        PathBuf::from(path)
    } else {
        PathBuf::from(path)
    }
}

pub(crate) fn normalized_path(path: impl AsRef<Path>) -> String {
    path.as_ref().display().to_string().replace('\\', "/")
}
