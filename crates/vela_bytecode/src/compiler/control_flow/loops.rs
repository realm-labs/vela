use crate::Register;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::compiler) struct LoopContext {
    continue_target: usize,
    break_jumps: Vec<usize>,
    continue_jumps: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::compiler) enum LoopIterable {
    Generic {
        iterator: Register,
    },
    Range {
        cursor: Register,
        end: Register,
        done: Register,
        inclusive: bool,
    },
}

impl LoopContext {
    pub(in crate::compiler) fn new(continue_target: usize) -> Self {
        Self {
            continue_target,
            break_jumps: Vec::new(),
            continue_jumps: Vec::new(),
        }
    }

    pub(in crate::compiler) fn continue_target(&self) -> usize {
        self.continue_target
    }

    pub(in crate::compiler) fn break_jumps(&self) -> &[usize] {
        &self.break_jumps
    }

    pub(in crate::compiler) fn continue_jumps(&self) -> &[usize] {
        &self.continue_jumps
    }

    pub(super) fn push_break(&mut self, offset: usize) {
        self.break_jumps.push(offset);
    }

    pub(super) fn push_continue(&mut self, offset: usize) {
        self.continue_jumps.push(offset);
    }
}
