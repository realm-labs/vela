use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub(crate) const CONFIG_FILE: &str = "vela.toml";
pub(crate) const SOURCE_EXTENSION: &str = ".vela";

pub(crate) fn document_path_uri(path: &str) -> String {
    // Canonical package roots may use Windows verbatim spelling. It is a
    // filesystem prefix, not part of the standard file URI presented to clients.
    let path = if cfg!(windows) {
        let path = normalized_path(path);
        if let Some(unc) = path.strip_prefix("//?/UNC/") {
            format!("//{unc}")
        } else {
            path.strip_prefix("//?/").unwrap_or(&path).to_owned()
        }
    } else {
        path.to_owned()
    };
    if let Ok(uri) = lsp_types::Url::from_file_path(&path) {
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

/// Keep package discovery and file events in the client's workspace spelling.
/// Physical identity is resolved here, at the server's filesystem boundary;
/// service queries and outgoing locations retain the workspace URI.
pub(crate) fn workspace_document_uri(path: &Path, roots: &BTreeSet<String>) -> String {
    let physical = canonicalize_existing_ancestor(path);
    let projected = physical.as_ref().and_then(|physical| {
        roots
            .iter()
            .filter_map(|root| {
                let root = document_uri_path(root);
                let physical_root = canonicalize_existing_ancestor(&root)?;
                let relative = physical.strip_prefix(&physical_root).ok()?;
                Some((physical_root.components().count(), root.join(relative)))
            })
            .max_by_key(|(depth, _)| *depth)
            .map(|(_, path)| path)
    });
    document_path_uri(&projected.as_deref().unwrap_or(path).display().to_string())
}

fn canonicalize_existing_ancestor(path: &Path) -> Option<PathBuf> {
    // Deletion notifications arrive after the file (or directory) is gone.
    // Resolve the closest existing ancestor and preserve the missing suffix.
    for ancestor in path.ancestors() {
        if let Ok(physical) = std::fs::canonicalize(ancestor) {
            return Some(physical.join(path.strip_prefix(ancestor).ok()?));
        }
    }
    None
}

#[cfg(test)]
mod uri_tests {
    #[test]
    fn file_uris_encode_unicode_and_normalize_windows_verbatim_roots() {
        let encoded = "%E4%B8%AD%E6%96%87%20%25%20source";
        #[cfg(windows)]
        let cases = [
            (
                r"C:\workspace\中文 % source",
                format!("file:///C:/workspace/{encoded}"),
            ),
            (
                r"\\?\C:\workspace\中文 % source",
                format!("file:///C:/workspace/{encoded}"),
            ),
            (
                "//?/C:/workspace/中文 % source",
                format!("file:///C:/workspace/{encoded}"),
            ),
            (
                r"\\?\UNC\server\share\中文 % source",
                format!("file://server/share/{encoded}"),
            ),
            (
                "//?/UNC/server/share/中文 % source",
                format!("file://server/share/{encoded}"),
            ),
        ];
        #[cfg(not(windows))]
        let cases = [
            (
                "/workspace/中文 % source",
                format!("file:///workspace/{encoded}"),
            ),
            (
                r"/workspace/中文 % source\literal",
                format!("file:///workspace/{encoded}%5Cliteral"),
            ),
        ];
        for (path, expected) in cases {
            let actual = super::document_path_uri(path);
            assert_eq!(actual, expected);
            let uri = lsp_types::Url::parse(&actual).expect("standard file URI");
            assert!(uri.query().is_none());
            assert!(uri.fragment().is_none());
            let physical = uri.to_file_path().expect("usable filesystem path");
            let name = if path.ends_with(r"\literal") {
                r"中文 % source\literal"
            } else {
                "中文 % source"
            };
            assert_eq!(
                physical.file_name().and_then(|name| name.to_str()),
                Some(name)
            );
        }
    }
}
