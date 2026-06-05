#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RangeValue {
    pub(crate) start: i64,
    pub(crate) end: i64,
    pub(crate) inclusive: bool,
}

impl RangeValue {
    pub(crate) fn new(start: i64, end: i64, inclusive: bool) -> Self {
        Self {
            start,
            end,
            inclusive,
        }
    }

    pub(crate) fn cursor(&self) -> RangeCursor {
        RangeCursor {
            next: self.start,
            end: self.end,
            inclusive: self.inclusive,
            done: false,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        if self.inclusive {
            self.start > self.end
        } else {
            self.start >= self.end
        }
    }

    pub(crate) fn len(&self) -> Option<i64> {
        if self.is_empty() {
            return Some(0);
        }
        let distance = i128::from(self.end) - i128::from(self.start);
        let len = if self.inclusive {
            distance.checked_add(1)?
        } else {
            distance
        };
        i64::try_from(len).ok()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RangeCursor {
    next: i64,
    end: i64,
    inclusive: bool,
    done: bool,
}

impl RangeCursor {
    pub(crate) fn next(&mut self) -> Option<i64> {
        if self.done {
            return None;
        }
        let has_next = if self.inclusive {
            self.next <= self.end
        } else {
            self.next < self.end
        };
        if !has_next {
            self.done = true;
            return None;
        }

        let value = self.next;
        if self.next == i64::MAX {
            self.done = true;
        } else {
            self.next += 1;
        }
        Some(value)
    }
}
