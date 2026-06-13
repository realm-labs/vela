pub(crate) fn python_major_from_version_text(text: &str) -> Option<u32> {
    let version = text
        .split_whitespace()
        .find(|part| part.chars().next().is_some_and(|ch| ch.is_ascii_digit()))?;
    version.split('.').next()?.parse().ok()
}
