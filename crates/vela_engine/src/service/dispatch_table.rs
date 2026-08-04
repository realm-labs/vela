#[doc(hidden)]
#[must_use]
pub fn dense_service_slot(entries: &[(u128, u32)], id: u128) -> Option<u32> {
    entries
        .binary_search_by_key(&id, |(candidate, _)| *candidate)
        .ok()
        .map(|index| entries[index].1)
}

#[doc(hidden)]
pub const fn sorted_service_slots<const N: usize>(
    mut entries: [(u128, u32); N],
) -> [(u128, u32); N] {
    let mut outer = 1;
    while outer < N {
        let mut inner = outer;
        while inner > 0 && entries[inner - 1].0 > entries[inner].0 {
            let previous = entries[inner - 1];
            entries[inner - 1] = entries[inner];
            entries[inner] = previous;
            inner -= 1;
        }
        outer += 1;
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::{dense_service_slot, sorted_service_slots};

    #[test]
    fn dense_slots_resolve_sorted_stable_ids() {
        const ENTRIES: [(u128, u32); 3] = sorted_service_slots([(90, 1), (10, 2), (40, 0)]);

        assert_eq!(ENTRIES, [(10, 2), (40, 0), (90, 1)]);
        assert_eq!(dense_service_slot(&ENTRIES, 10), Some(2));
        assert_eq!(dense_service_slot(&ENTRIES, 40), Some(0));
        assert_eq!(dense_service_slot(&ENTRIES, 90), Some(1));
        assert_eq!(dense_service_slot(&ENTRIES, 11), None);
    }
}
