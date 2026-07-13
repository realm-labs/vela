use std::fs;
use std::path::{Path, PathBuf};

use vela_package::load_package_graph;

use crate::{
    DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, Workspace,
    assemble_package_project_sources,
};

#[test]
fn definition_follows_provider_to_service_trait_across_package() {
    let fixture = ProviderToolingFixture::new("definition");
    let databases = fixture.databases();
    let line = fixture
        .plugin_text
        .lines()
        .nth(3)
        .expect("provider impl line");
    let definition = databases
        .definition(
            &fixture.plugin_document,
            Position::new(3, line.find("CommandProvider").expect("service use")),
        )
        .expect("provider service definition");

    assert_eq!(definition.document_id(), &fixture.api_document);
    fixture.remove();
}

#[test]
fn references_find_service_provider_impls_across_packages() {
    let fixture = ProviderToolingFixture::new("references");
    let databases = fixture.databases();
    let line = fixture.api_text.lines().next().expect("trait line");
    let references = databases.references(
        &fixture.api_document,
        Position::new(0, line.find("CommandProvider").expect("trait declaration")),
        true,
    );

    assert!(
        references
            .iter()
            .any(|reference| reference.document_id() == &fixture.plugin_document),
        "{references:?}"
    );
    fixture.remove();
}

#[test]
fn workspace_symbols_expose_package_provider_identity() {
    let fixture = ProviderToolingFixture::new("symbols");
    let databases = fixture.databases();
    let symbols = databases.workspace_symbols("command");

    assert!(
        symbols
            .iter()
            .any(|symbol| symbol.name() == "dev.vela.plugin::command"),
        "{symbols:?}"
    );
    fixture.remove();
}

struct ProviderToolingFixture {
    root: PathBuf,
    api_document: DocumentId,
    plugin_document: DocumentId,
    api_text: String,
    plugin_text: String,
}

impl ProviderToolingFixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "vela_provider_tooling_{name}_{}",
            std::process::id()
        ));
        let api_text = "pub trait CommandProvider { fn run(self) -> i64; }\n".to_owned();
        let plugin_text = "use api::api::CommandProvider\npub struct Command {}\n#[provider(id = \"command\")]\nimpl CommandProvider for Command { pub fn run(self) -> i64 { return 1; } }\n".to_owned();
        write_file(
            &root.join("api/vela.toml"),
            "[package]\nid = \"dev.vela.api\"\nname = \"api\"\nversion = \"0.1.0\"\n[source]\nroots = [\"src\"]\n",
        );
        write_file(&root.join("api/src/api.vela"), &api_text);
        write_file(
            &root.join("plugin/vela.toml"),
            "[package]\nid = \"dev.vela.plugin\"\nname = \"plugin\"\nversion = \"0.1.0\"\n[source]\nroots = [\"src\"]\n[dependencies]\napi = { path = \"../api\" }\n",
        );
        write_file(&root.join("plugin/src/plugin.vela"), &plugin_text);
        Self {
            api_document: document(&root.join("api/src/api.vela")),
            plugin_document: document(&root.join("plugin/src/plugin.vela")),
            root,
            api_text,
            plugin_text,
        }
    }

    fn databases(&self) -> LanguageServiceDatabases {
        let graph = load_package_graph(
            self.root.join("plugin/vela.toml"),
            std::slice::from_ref(&self.root),
        )
        .expect("package graph");
        let files = graph
            .sources()
            .sources()
            .iter()
            .map(|source| SourceFileSnapshot::new(document(&source.path), source.text.as_ref()))
            .collect::<Vec<_>>();
        let project =
            assemble_package_project_sources(&graph, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        databases
    }

    fn remove(self) {
        fs::remove_dir_all(self.root).expect("remove provider tooling fixture");
    }
}

fn write_file(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().expect("file parent")).expect("create fixture directory");
    fs::write(path, text).expect("write fixture file");
}

fn document(path: &Path) -> DocumentId {
    DocumentId::from(
        fs::canonicalize(path)
            .expect("canonical fixture path")
            .display()
            .to_string(),
    )
}
