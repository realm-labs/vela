use super::model::Import;

pub(super) fn closest_name(
    wanted: &str,
    candidates: impl IntoIterator<Item = impl AsRef<str>>,
) -> Option<String> {
    candidates
        .into_iter()
        .map(|candidate| candidate.as_ref().to_owned())
        .min_by_key(|candidate| candidate_distance(wanted, candidate))
        .filter(|candidate| candidate_distance(wanted, candidate) <= 3)
}

pub(super) fn impl_declaration_name(trait_path: &[String], target_path: &[String]) -> String {
    format!(
        "impl {} for {}",
        trait_path.join("::"),
        target_path.join("::")
    )
}

pub(super) fn import_binding_name(import: &Import) -> Option<String> {
    import.alias.clone().or_else(|| import.path.last().cloned())
}

pub(super) fn candidate_distance(wanted: &str, candidate: &str) -> usize {
    if wanted.contains(candidate) || candidate.contains(wanted) {
        return 0;
    }
    levenshtein(wanted, candidate)
}

pub(super) fn levenshtein(lhs: &str, rhs: &str) -> usize {
    let mut previous = (0..=rhs.chars().count()).collect::<Vec<_>>();
    let mut current = vec![0; previous.len()];

    for (lhs_index, lhs_char) in lhs.chars().enumerate() {
        current[0] = lhs_index + 1;
        for (rhs_index, rhs_char) in rhs.chars().enumerate() {
            let cost = usize::from(lhs_char != rhs_char);
            current[rhs_index + 1] = (previous[rhs_index + 1] + 1)
                .min(current[rhs_index] + 1)
                .min(previous[rhs_index] + cost);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[rhs.chars().count()]
}
