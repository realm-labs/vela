use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CallStatus {
    Created,
    Running,
    Completed,
    Failed,
    Cancelled,
    DeadlineExceeded,
}

impl CallStatus {
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Running,
            2 => Self::Completed,
            3 => Self::Failed,
            4 => Self::Cancelled,
            5 => Self::DeadlineExceeded,
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
    pub deadline: Option<Instant>,
}

#[derive(Clone, Debug)]
pub struct CallControl(Arc<CallControlState>);

#[derive(Debug)]
struct CallControlState {
    cancelled: AtomicBool,
    status: AtomicU8,
    deadline: parking_lot::Mutex<Option<Instant>>,
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
            deadline: parking_lot::Mutex::new(None),
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
        true
    }

    #[must_use]
    pub fn snapshot(&self) -> CallSnapshot {
        CallSnapshot {
            status: CallStatus::from_u8(self.0.status.load(Ordering::Acquire)),
            deadline: *self.0.deadline.lock(),
        }
    }

    pub(super) fn attach(&self, deadline: Option<Instant>) {
        *self.0.deadline.lock() = deadline;
        if !self.0.cancelled.load(Ordering::Acquire) {
            self.set_status(CallStatus::Running);
        }
    }

    pub(super) fn cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::Acquire)
    }

    pub(super) fn finish(&self, status: CallStatus) {
        self.set_status(status);
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

impl CallPolicy {
    pub(super) fn attach(&self) {
        if let Some(control) = &self.control {
            control.attach(self.deadline);
        }
    }

    pub(super) fn check(&self) -> vela_vm::error::VmResult<()> {
        let status = if self.control.as_ref().is_some_and(CallControl::cancelled) {
            Some(CallStatus::Cancelled)
        } else if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            Some(CallStatus::DeadlineExceeded)
        } else {
            None
        };
        if let Some(status) = status {
            if let Some(control) = &self.control {
                control.finish(status);
            }
            return Err(vela_vm::error::VmError::new(match status {
                CallStatus::Cancelled => vela_vm::error::VmErrorKind::CallCancelled,
                CallStatus::DeadlineExceeded => vela_vm::error::VmErrorKind::DeadlineExceeded,
                _ => unreachable!("only abort states create control errors"),
            }));
        }
        Ok(())
    }

    pub(super) fn finish<T>(&self, result: &vela_vm::error::VmResult<T>) {
        if let Some(control) = &self.control {
            control.finish(if result.is_ok() {
                CallStatus::Completed
            } else {
                CallStatus::Failed
            });
        }
    }
}
