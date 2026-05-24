use vela_common::Span;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReflectCandidate {
    pub name: String,
    pub source_span: Option<Span>,
}

impl ReflectCandidate {
    #[must_use]
    pub fn new(name: impl Into<String>, source_span: Option<Span>) -> Self {
        Self {
            name: name.into(),
            source_span,
        }
    }
}

pub(crate) fn name_candidates<'a>(
    name: &str,
    candidates: impl Iterator<Item = &'a str>,
) -> Vec<String> {
    ranked_candidates(name, candidates.map(|candidate| (candidate, None)))
        .into_iter()
        .map(|candidate| candidate.name)
        .collect()
}

pub(crate) fn ranked_candidates<'a>(
    name: &str,
    candidates: impl Iterator<Item = (&'a str, Option<Span>)>,
) -> Vec<ReflectCandidate> {
    let mut candidates = candidates
        .map(|(candidate, source_span)| {
            (
                edit_distance(name, candidate),
                candidate.to_owned(),
                source_span,
            )
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    candidates
        .into_iter()
        .take(3)
        .map(|(_, candidate, source_span)| ReflectCandidate::new(candidate, source_span))
        .collect()
}

pub(crate) fn candidate_names(candidates: &[ReflectCandidate]) -> Vec<String> {
    candidates
        .iter()
        .map(|candidate| candidate.name.clone())
        .collect()
}

fn edit_distance(left: &str, right: &str) -> usize {
    let left = left.chars().collect::<Vec<_>>();
    let right = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right.len()).collect::<Vec<_>>();
    let mut current = vec![0; right.len() + 1];

    for (left_index, left_ch) in left.iter().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_ch) in right.iter().enumerate() {
            let substitution = usize::from(left_ch != right_ch);
            current[right_index + 1] = (previous[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(previous[right_index] + substitution);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[right.len()]
}
