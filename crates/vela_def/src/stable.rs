/// Returns the stable FNV-1a identity used by Vela's existing schema and
/// script-member namespaces.
///
/// The zero separators are part of the identity contract. New typed
/// definition families should expose focused helpers rather than repeating
/// namespace strings at call sites.
#[must_use]
pub const fn stable_id(namespace: &str, owner: &str, name: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    stable_hash_bytes(&mut hash, namespace.as_bytes());
    stable_hash_bytes(&mut hash, &[0]);
    stable_hash_bytes(&mut hash, owner.as_bytes());
    stable_hash_bytes(&mut hash, &[0]);
    stable_hash_bytes(&mut hash, name.as_bytes());
    if hash == 0 { 1 } else { hash }
}

const fn stable_hash_bytes(hash: &mut u64, bytes: &[u8]) {
    let mut index = 0;
    while index < bytes.len() {
        *hash ^= bytes[index] as u64;
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        index += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::stable_id;

    #[rstest::rstest]
    #[case::inherent_method("inherent_method", "main::Player", "bonus", 0xe0dc_50cc_b2ea_1381)]
    #[case::trait_method("trait_method", "main::BonusSource", "bonus", 0xbc3f_86dc_30f1_b48f)]
    #[case::builtin_trait_method("trait_method", "PartialEq", "eq", 0xafff_db83_17bd_1f5c)]
    fn fnv64_fixture_outputs_are_stable(
        #[case] namespace: &str,
        #[case] owner: &str,
        #[case] name: &str,
        #[case] expected: u64,
    ) {
        assert_eq!(stable_id(namespace, owner, name), expected);
    }
}
