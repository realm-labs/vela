#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SmallStorage<T> {
    Empty,
    One([T; 1]),
    Two([T; 2]),
    Three([T; 3]),
    Four([T; 4]),
    Five([T; 5]),
    Six([T; 6]),
    Seven([T; 7]),
    Eight([T; 8]),
    Many(Vec<T>),
}

impl<T> SmallStorage<T> {
    #[inline]
    pub(crate) fn try_from_slice_map<U, E>(
        items: &[U],
        inline_limit: usize,
        mut map: impl FnMut(&U) -> Result<T, E>,
    ) -> Result<Self, E> {
        match items {
            [] => Ok(Self::Empty),
            [first] if inline_limit >= 1 => Ok(Self::One([map(first)?])),
            [first, second] if inline_limit >= 2 => Ok(Self::Two([map(first)?, map(second)?])),
            [first, second, third] if inline_limit >= 3 => {
                Ok(Self::Three([map(first)?, map(second)?, map(third)?]))
            }
            [first, second, third, fourth] if inline_limit >= 4 => Ok(Self::Four([
                map(first)?,
                map(second)?,
                map(third)?,
                map(fourth)?,
            ])),
            [first, second, third, fourth, fifth] if inline_limit >= 5 => Ok(Self::Five([
                map(first)?,
                map(second)?,
                map(third)?,
                map(fourth)?,
                map(fifth)?,
            ])),
            [first, second, third, fourth, fifth, sixth] if inline_limit >= 6 => Ok(Self::Six([
                map(first)?,
                map(second)?,
                map(third)?,
                map(fourth)?,
                map(fifth)?,
                map(sixth)?,
            ])),
            [first, second, third, fourth, fifth, sixth, seventh] if inline_limit >= 7 => {
                Ok(Self::Seven([
                    map(first)?,
                    map(second)?,
                    map(third)?,
                    map(fourth)?,
                    map(fifth)?,
                    map(sixth)?,
                    map(seventh)?,
                ]))
            }
            [first, second, third, fourth, fifth, sixth, seventh, eighth] if inline_limit >= 8 => {
                Ok(Self::Eight([
                    map(first)?,
                    map(second)?,
                    map(third)?,
                    map(fourth)?,
                    map(fifth)?,
                    map(sixth)?,
                    map(seventh)?,
                    map(eighth)?,
                ]))
            }
            _ => {
                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    values.push(map(item)?);
                }
                Ok(Self::Many(values))
            }
        }
    }

    #[inline]
    pub(crate) fn try_from_prefix_and_slice_map<U, E>(
        prefix: T,
        items: &[U],
        inline_limit: usize,
        mut map: impl FnMut(&U) -> Result<T, E>,
    ) -> Result<Self, E> {
        match items {
            [] if inline_limit >= 1 => Ok(Self::One([prefix])),
            [first] if inline_limit >= 2 => Ok(Self::Two([prefix, map(first)?])),
            [first, second] if inline_limit >= 3 => {
                Ok(Self::Three([prefix, map(first)?, map(second)?]))
            }
            [first, second, third] if inline_limit >= 4 => {
                Ok(Self::Four([prefix, map(first)?, map(second)?, map(third)?]))
            }
            [first, second, third, fourth] if inline_limit >= 5 => Ok(Self::Five([
                prefix,
                map(first)?,
                map(second)?,
                map(third)?,
                map(fourth)?,
            ])),
            [first, second, third, fourth, fifth] if inline_limit >= 6 => Ok(Self::Six([
                prefix,
                map(first)?,
                map(second)?,
                map(third)?,
                map(fourth)?,
                map(fifth)?,
            ])),
            [first, second, third, fourth, fifth, sixth] if inline_limit >= 7 => Ok(Self::Seven([
                prefix,
                map(first)?,
                map(second)?,
                map(third)?,
                map(fourth)?,
                map(fifth)?,
                map(sixth)?,
            ])),
            [first, second, third, fourth, fifth, sixth, seventh] if inline_limit >= 8 => {
                Ok(Self::Eight([
                    prefix,
                    map(first)?,
                    map(second)?,
                    map(third)?,
                    map(fourth)?,
                    map(fifth)?,
                    map(sixth)?,
                    map(seventh)?,
                ]))
            }
            _ => {
                let mut values = Vec::with_capacity(items.len() + 1);
                values.push(prefix);
                for item in items {
                    values.push(map(item)?);
                }
                Ok(Self::Many(values))
            }
        }
    }

    #[inline]
    pub(crate) fn as_slice(&self) -> &[T] {
        match self {
            Self::Empty => &[],
            Self::One(values) => values,
            Self::Two(values) => values,
            Self::Three(values) => values,
            Self::Four(values) => values,
            Self::Five(values) => values,
            Self::Six(values) => values,
            Self::Seven(values) => values,
            Self::Eight(values) => values,
            Self::Many(values) => values,
        }
    }

    #[inline]
    pub(crate) fn into_vec(self) -> Vec<T> {
        match self {
            Self::Empty => Vec::new(),
            Self::One(values) => Vec::from(values),
            Self::Two(values) => Vec::from(values),
            Self::Three(values) => Vec::from(values),
            Self::Four(values) => Vec::from(values),
            Self::Five(values) => Vec::from(values),
            Self::Six(values) => Vec::from(values),
            Self::Seven(values) => Vec::from(values),
            Self::Eight(values) => Vec::from(values),
            Self::Many(values) => values,
        }
    }

    #[inline]
    pub(crate) fn spilled_capacity(&self) -> usize {
        match self {
            Self::Many(values) => values.capacity(),
            Self::Empty
            | Self::One(_)
            | Self::Two(_)
            | Self::Three(_)
            | Self::Four(_)
            | Self::Five(_)
            | Self::Six(_)
            | Self::Seven(_)
            | Self::Eight(_) => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SmallStorage;

    #[test]
    fn stores_inline_values_up_to_requested_limit() {
        let storage =
            SmallStorage::try_from_slice_map(&[1, 2, 3, 4], 4, |value| Ok::<_, ()>(value * 2))
                .expect("inline storage");

        assert_eq!(storage.as_slice(), &[2, 4, 6, 8]);
    }

    #[test]
    fn spills_to_vec_after_requested_limit() {
        let storage =
            SmallStorage::try_from_slice_map(&[1, 2, 3, 4, 5], 4, |value| Ok::<_, ()>(value * 2))
                .expect("vec storage");

        assert_eq!(storage.as_slice(), &[2, 4, 6, 8, 10]);
        assert_eq!(storage.into_vec(), vec![2, 4, 6, 8, 10]);
    }

    #[test]
    fn spilled_capacity_reports_only_vec_storage() {
        let inline =
            SmallStorage::try_from_slice_map(&[1, 2, 3, 4], 4, |value| Ok::<_, ()>(*value))
                .expect("inline storage");
        let spilled =
            SmallStorage::try_from_slice_map(&[1, 2, 3, 4, 5], 4, |value| Ok::<_, ()>(*value))
                .expect("vec storage");

        assert_eq!(inline.spilled_capacity(), 0);
        assert!(spilled.spilled_capacity() >= 5);
    }

    #[test]
    fn supports_eight_value_inline_storage() {
        let storage = SmallStorage::try_from_slice_map(&[1, 2, 3, 4, 5, 6, 7, 8], 8, |value| {
            Ok::<_, ()>(*value)
        })
        .expect("inline storage");

        assert_eq!(storage.as_slice(), &[1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn stores_prefix_and_slice_inline_up_to_requested_limit() {
        let storage =
            SmallStorage::try_from_prefix_and_slice_map(1, &[2, 3], 3, |value| Ok::<_, ()>(*value))
                .expect("inline storage");

        assert_eq!(storage.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn spills_prefix_and_slice_after_requested_limit() {
        let storage = SmallStorage::try_from_prefix_and_slice_map(1, &[2, 3, 4], 3, |value| {
            Ok::<_, ()>(*value)
        })
        .expect("vec storage");

        assert_eq!(storage.as_slice(), &[1, 2, 3, 4]);
        assert_eq!(storage.into_vec(), vec![1, 2, 3, 4]);
    }
}
