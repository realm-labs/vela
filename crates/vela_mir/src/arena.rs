use std::marker::PhantomData;

use crate::ids::ArenaId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Arena<I, T> {
    values: Vec<T>,
    marker: PhantomData<fn(I) -> I>,
}

impl<I, T> Default for Arena<I, T> {
    fn default() -> Self {
        Self {
            values: Vec::new(),
            marker: PhantomData,
        }
    }
}

impl<I: ArenaId, T> Arena<I, T> {
    pub(crate) fn allocate(&mut self, value: T) -> I {
        let index = u32::try_from(self.values.len())
            .expect("MIR arena cannot contain more than u32::MAX entries");
        self.values.push(value);
        I::from_arena_index(index)
    }

    pub(crate) fn get(&self, id: I) -> Option<&T> {
        usize::try_from(id.arena_index())
            .ok()
            .and_then(|index| self.values.get(index))
    }

    pub(crate) fn get_mut(&mut self, id: I) -> Option<&mut T> {
        usize::try_from(id.arena_index())
            .ok()
            .and_then(|index| self.values.get_mut(index))
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (I, &T)> {
        self.values.iter().enumerate().map(|(index, value)| {
            let index = u32::try_from(index).expect("MIR arena index was allocated as u32");
            (I::from_arena_index(index), value)
        })
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub(crate) fn len(&self) -> usize {
        self.values.len()
    }
}
