use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::task::Waker;
use std::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CallStatus {
    Created,
    Running,
    Pending,
    Completed,
    Failed,
    Cancelled,
    DeadlineExceeded,
}

impl CallStatus {
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Running,
            2 => Self::Pending,
            3 => Self::Completed,
            4 => Self::Failed,
            5 => Self::Cancelled,
            6 => Self::DeadlineExceeded,
            _ => Self::Created,
        }
    }

    const fn terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::DeadlineExceeded
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallSnapshot {
    pub status: CallStatus,
    pub polls: u64,
    pub deadline: Option<Instant>,
}

#[derive(Clone, Debug)]
pub struct CallControl(Arc<CallControlState>);

#[derive(Debug)]
struct CallControlState {
    cancelled: AtomicBool,
    status: AtomicU8,
    polls: AtomicU64,
    deadline: parking_lot::Mutex<Option<Instant>>,
    waker: parking_lot::Mutex<Option<Waker>>,
}

impl Default for CallControl {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for CallControl {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for CallControl {}

impl CallControl {
    #[must_use]
    pub fn new() -> Self {
        Self(Arc::new(CallControlState {
            cancelled: AtomicBool::new(false),
            status: AtomicU8::new(CallStatus::Created as u8),
            polls: AtomicU64::new(0),
            deadline: parking_lot::Mutex::new(None),
            waker: parking_lot::Mutex::new(None),
        }))
    }

    pub fn cancel(&self) -> bool {
        if CallStatus::from_u8(self.0.status.load(Ordering::Acquire)).terminal() {
            return false;
        }
        if self.0.cancelled.swap(true, Ordering::AcqRel) {
            return false;
        }
        self.set_status(CallStatus::Cancelled);
        if let Some(waker) = self.0.waker.lock().take() {
            waker.wake();
        }
        true
    }

    #[must_use]
    pub fn snapshot(&self) -> CallSnapshot {
        CallSnapshot {
            status: CallStatus::from_u8(self.0.status.load(Ordering::Acquire)),
            polls: self.0.polls.load(Ordering::Relaxed),
            deadline: *self.0.deadline.lock(),
        }
    }

    pub(super) fn attach(&self, deadline: Option<Instant>) {
        *self.0.deadline.lock() = deadline;
        if !self.0.cancelled.load(Ordering::Acquire) {
            self.set_status(CallStatus::Running);
        }
    }

    pub(super) fn begin_poll(&self, waker: &Waker) -> bool {
        self.0.polls.fetch_add(1, Ordering::Relaxed);
        *self.0.waker.lock() = Some(waker.clone());
        self.0.cancelled.load(Ordering::Acquire)
    }

    pub(super) fn cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::Acquire)
    }

    pub(super) fn pending(&self) {
        self.set_status(CallStatus::Pending);
    }

    pub(super) fn finish(&self, status: CallStatus) {
        self.set_status(status);
        self.0.waker.lock().take();
    }

    fn set_status(&self, next: CallStatus) {
        let mut current = self.0.status.load(Ordering::Acquire);
        loop {
            if CallStatus::from_u8(current).terminal() {
                return;
            }
            match self.0.status.compare_exchange_weak(
                current,
                next as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return,
                Err(actual) => current = actual,
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct CallPolicy {
    pub deadline: Option<Instant>,
    pub control: Option<CallControl>,
}
