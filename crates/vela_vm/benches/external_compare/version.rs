pub(crate) fn node_version_text_is_supported(text: &str) -> bool {
    let Some(version) = text.trim().strip_prefix('v') else {
        return false;
    };
    let parts = version.split('.').collect::<Vec<_>>();
    parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && (part.len() == 1 || !part.starts_with('0'))
        })
}

pub(crate) fn python_major_from_version_text(text: &str) -> Option<u32> {
    let version = text
        .split_whitespace()
        .find(|part| part.chars().next().is_some_and(|ch| ch.is_ascii_digit()))?;
    version.split('.').next()?.parse().ok()
}
