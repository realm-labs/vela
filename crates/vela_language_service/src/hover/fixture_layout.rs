use std::path::PathBuf;

use crate::matrix_fixture::FixtureWorkspace;
use crate::{DocumentId, LanguageServiceDatabases};
use serde_json::Value;

pub(super) struct Layout {
    root: PathBuf,
    pub(super) package: Option<vela_package::PackageGraph>,
}

impl Layout {
    pub(super) fn new(fixture: &FixtureWorkspace, package: bool) -> Self {
        if !package {
            return Self {
                root: PathBuf::from("/workspace/中文 % hover"),
                package: None,
            };
        }
        let root = std::env::temp_dir().join(format!(
            "vela-hover-matrix-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fixture.materialize(&root).expect("owned package inputs");
        let package =
            vela_package::load_package_graph(root.join("vela.toml"), std::slice::from_ref(&root))
                .expect("package graph");
        Self {
            root,
            package: Some(package),
        }
    }

    pub(super) fn uri(&self, file: &str) -> DocumentId {
        DocumentId::from(self.path(file))
    }

    pub(super) fn path(&self, file: &str) -> String {
        self.root.join(file).to_string_lossy().replace('\\', "/")
    }
}

impl Drop for Layout {
    fn drop(&mut self) {
        if self.package.is_some() {
            let temp = std::env::temp_dir().canonicalize().expect("temp root");
            let owned = self.root.canonicalize().expect("owned fixture root");
            assert_eq!(owned.parent(), Some(temp.as_path()));
            assert!(
                owned
                    .file_name()
                    .expect("fixture name")
                    .to_string_lossy()
                    .starts_with("vela-hover-matrix-")
            );
            std::fs::remove_dir_all(owned).expect("remove owned package fixture");
        }
    }
}

pub(super) fn assert_schema_locations(
    db: &LanguageServiceDatabases,
    fixture: &FixtureWorkspace,
    oracle: &Value,
    layout: &Layout,
    missing: bool,
) {
    for row in oracle["locations"].as_array().into_iter().flatten() {
        let name = row["name"].as_str().expect("schema name");
        let locations = db.schema_db().source_locations();
        let actual = match row["kind"].as_str().expect("location kind") {
            "type" => locations.type_span(name),
            "trait" => locations.trait_span(name),
            "module" => locations.module_span(name),
            "function" => locations.function_span(name),
            kind => panic!("unreviewed location {kind}"),
        };
        let file = row["file"].as_str().expect("location file");
        let spelling = row["spelling"].as_str().expect("declaration spelling");
        let start = fixture.disk[file]
            .text
            .find(spelling)
            .expect("independent source spelling");
        let source = db.source_db().records()[&layout.uri(file)].source_id();
        let expected = (!missing).then(|| {
            vela_common::Span::new(
                source,
                u32::try_from(start).expect("start"),
                u32::try_from(start + spelling.len()).expect("end"),
            )
        });
        assert_eq!(
            actual, expected,
            "{name} source location, missing={missing}"
        );
    }
}
