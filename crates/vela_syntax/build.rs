fn main() {
    // Rstest's file glob must be re-expanded when fixtures are added or removed.
    println!("cargo:rerun-if-changed=tests/fixtures/formatting");
}
