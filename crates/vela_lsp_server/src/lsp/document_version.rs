use vela_language_service::SourceVersion;

// Source versions are opaque identities in the service; workspace generations
// order analysis work. Keep the signed LSP bits without changing positive IDs.
pub(crate) fn from_lsp(version: i32) -> SourceVersion {
    SourceVersion::new(u64::from(u32::from_ne_bytes(version.to_ne_bytes())))
}

pub(crate) fn to_lsp(version: SourceVersion) -> i32 {
    let bits = u32::try_from(version.get()).expect("LSP document version should fit in 32 bits");
    i32::from_ne_bytes(bits.to_ne_bytes())
}
