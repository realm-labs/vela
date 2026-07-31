#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};

use syn::visit::{self, Visit};

/// Files permitted to contain `unsafe` Rust.
///
/// The workspace lint is `unsafe_code = "deny"` rather than `forbid`, so a
/// module may opt in locally when a reviewed invariant justifies it. This audit
/// is the workspace-wide gate that keeps those opt-ins enumerated: adding
/// `#[allow(unsafe_code)]` without adding the file here fails this test. Each
/// entry must carry a module-level safety invariant explaining what proves the
/// unchecked operation sound, and every `unsafe` block needs a `SAFETY:`
/// comment (enforced separately by `clippy::undocumented_unsafe_blocks`).
const REVIEWED_UNSAFE_BOUNDARIES: &[&str] = &[
    "crates/vela_c_api/src/lib.rs",
    "crates/vela_host/src/erased_reborrow.rs",
    "crates/vela_host/src/erased_slice.rs",
];

#[test]
fn unsafe_rust_is_confined_to_reviewed_boundary_files() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("vela_host must be located under the workspace crates directory");
    let mut rust_files = Vec::new();
    for root in ["crates", "examples", "fuzz", "tests"] {
        collect_rust_files(&workspace.join(root), &mut rust_files);
    }

    let mut violations = Vec::new();
    for path in rust_files {
        let relative = path
            .strip_prefix(workspace)
            .expect("collected source must stay inside the workspace")
            .to_string_lossy()
            .replace('\\', "/");
        let source = fs::read_to_string(&path).expect("Rust source must be readable");
        if !source.contains("unsafe") {
            continue;
        }
        let syntax = syn::parse_file(&source).expect("Rust source must parse for the unsafe audit");
        let approved = REVIEWED_UNSAFE_BOUNDARIES.contains(&relative.as_str());
        let mut audit = UnsafeAudit::default();
        audit.visit_file(&syntax);
        if !approved && !audit.kinds.is_empty() {
            violations.push(format!("{relative}: {}", audit.kinds.join(", ")));
        }
    }

    assert!(
        violations.is_empty(),
        "unsafe Rust escaped the reviewed boundaries:\n{}",
        violations.join("\n")
    );
}

fn collect_rust_files(root: &Path, output: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries {
        let path = entry
            .expect("source directory entry must be readable")
            .path();
        if path.is_dir() {
            collect_rust_files(&path, output);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            output.push(path);
        }
    }
}

#[derive(Default)]
struct UnsafeAudit {
    kinds: Vec<&'static str>,
}

impl<'ast> Visit<'ast> for UnsafeAudit {
    fn visit_expr_unsafe(&mut self, node: &'ast syn::ExprUnsafe) {
        self.kinds.push("unsafe block");
        visit::visit_expr_unsafe(self, node);
    }

    fn visit_signature(&mut self, node: &'ast syn::Signature) {
        if node.unsafety.is_some() {
            self.kinds.push("unsafe function");
        }
        visit::visit_signature(self, node);
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        if node.unsafety.is_some() {
            self.kinds.push("unsafe impl");
        }
        visit::visit_item_impl(self, node);
    }

    fn visit_item_trait(&mut self, node: &'ast syn::ItemTrait) {
        if node.unsafety.is_some() {
            self.kinds.push("unsafe trait");
        }
        visit::visit_item_trait(self, node);
    }

    fn visit_item_foreign_mod(&mut self, node: &'ast syn::ItemForeignMod) {
        if node.unsafety.is_some() {
            self.kinds.push("unsafe extern block");
        }
        visit::visit_item_foreign_mod(self, node);
    }

    fn visit_attribute(&mut self, node: &'ast syn::Attribute) {
        if node.path().is_ident("unsafe") {
            self.kinds.push("unsafe attribute");
        }
        visit::visit_attribute(self, node);
    }
}
