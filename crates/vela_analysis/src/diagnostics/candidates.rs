pub(super) fn ranked_names(
    name: &str,
    candidates: impl IntoIterator<Item = impl Into<String>>,
) -> Vec<String> {
    let mut candidates = candidates
        .into_iter()
        .map(Into::into)
        .map(|candidate| (edit_distance(name, &candidate), candidate))
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    candidates
        .into_iter()
        .take(3)
        .map(|(_, candidate)| candidate)
        .collect()
}

fn edit_distance(left: &str, right: &str) -> usize {
    let right_chars = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right_chars.len()).collect::<Vec<_>>();
    for (left_index, left_char) in left.chars().enumerate() {
        let mut current = Vec::with_capacity(right_chars.len() + 1);
        current.push(left_index + 1);
        for (right_index, right_char) in right_chars.iter().enumerate() {
            let substitution_cost = usize::from(left_char != *right_char);
            current.push(
                (previous[right_index + 1] + 1)
                    .min(current[right_index] + 1)
                    .min(previous[right_index] + substitution_cost),
            );
        }
        previous = current;
    }
    previous[right_chars.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_names_by_edit_distance_then_name() {
        assert_eq!(
            ranked_names("Actve", ["Finished", "Active", "Accepted"]),
            vec![
                "Active".to_owned(),
                "Accepted".to_owned(),
                "Finished".to_owned()
            ]
        );
    }
}
