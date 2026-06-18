use super::SemanticToken;

pub(super) fn semantic_tokens_result_id(tokens: &[SemanticToken]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    hash_result_id_part(&mut hash, tokens.len() as u64);
    for token in tokens {
        hash_result_id_part(&mut hash, token.start().line as u64);
        hash_result_id_part(&mut hash, token.start().character as u64);
        hash_result_id_part(&mut hash, token.length() as u64);
        hash_result_id_part(&mut hash, u64::from(token.token_type().legend_index()));
        hash_result_id_part(&mut hash, u64::from(token.modifiers().bits()));
    }
    format!("v1:{}:{hash:016x}", tokens.len())
}

pub(super) fn semantic_token_count_from_result_id(result_id: &str) -> usize {
    let mut parts = result_id.split(':');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some("v1"), Some(count), Some(_hash), None) => count.parse().unwrap_or(0),
        _ => 0,
    }
}

fn hash_result_id_part(hash: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}
