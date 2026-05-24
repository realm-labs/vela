#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LoopContext {
    continue_target: usize,
    break_jumps: Vec<usize>,
    continue_jumps: Vec<usize>,
}

impl LoopContext {
    pub(super) fn new(continue_target: usize) -> Self {
        Self {
            continue_target,
            break_jumps: Vec::new(),
            continue_jumps: Vec::new(),
        }
    }

    pub(super) fn continue_target(&self) -> usize {
        self.continue_target
    }

    pub(super) fn break_jumps(&self) -> &[usize] {
        &self.break_jumps
    }

    pub(super) fn continue_jumps(&self) -> &[usize] {
        &self.continue_jumps
    }

    pub(super) fn push_break(&mut self, offset: usize) {
        self.break_jumps.push(offset);
    }

    pub(super) fn push_continue(&mut self, offset: usize) {
        self.continue_jumps.push(offset);
    }
}
