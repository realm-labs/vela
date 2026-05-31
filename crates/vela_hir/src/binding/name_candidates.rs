use vela_common::Span;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NameCandidate {
    pub name: String,
    pub span: Option<Span>,
}

impl NameCandidate {
    pub(super) fn new(name: impl Into<String>, span: Option<Span>) -> Self {
        Self {
            name: name.into(),
            span,
        }
    }
}

pub(super) fn closest_name_candidate(
    wanted: &str,
    candidates: impl IntoIterator<Item = NameCandidate>,
) -> Option<NameCandidate> {
    candidates
        .into_iter()
        .map(|candidate| (candidate_distance(wanted, &candidate.name), candidate))
        .min_by(
            |(left_distance, left_candidate), (right_distance, right_candidate)| {
                left_distance
                    .cmp(right_distance)
                    .then_with(|| left_candidate.name.cmp(&right_candidate.name))
            },
        )
        .and_then(|(distance, candidate)| (distance <= 3).then_some(candidate))
}

fn candidate_distance(wanted: &str, candidate: &str) -> usize {
    if wanted.contains(candidate) || candidate.contains(wanted) {
        return 0;
    }
    levenshtein(wanted, candidate)
}

fn levenshtein(lhs: &str, rhs: &str) -> usize {
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
