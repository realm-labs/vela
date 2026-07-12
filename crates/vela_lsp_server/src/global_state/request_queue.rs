use std::collections::{BTreeMap, BTreeSet};

use lsp_server::{Message, RequestId};
use vela_language_service::CancellationHandle;

#[derive(Debug, Default)]
pub(super) struct RequestQueue {
    pub(super) incoming: BTreeSet<RequestId>,
    cancelled: BTreeSet<RequestId>,
    pub(super) in_flight: BTreeMap<RequestId, CancellationHandle>,
}

impl RequestQueue {
    pub(super) fn request_id(message: &Message) -> Option<RequestId> {
        match message {
            Message::Request(request) => Some(request.id.clone()),
            Message::Response(_) | Message::Notification(_) => None,
        }
    }

    pub(super) fn start(&mut self, id: RequestId) {
        self.incoming.insert(id);
    }

    pub(super) fn finish(&mut self, id: &RequestId) {
        self.incoming.remove(id);
    }

    pub(super) fn start_in_flight(&mut self, id: RequestId, handle: CancellationHandle) {
        self.in_flight.insert(id, handle);
    }

    pub(super) fn finish_in_flight(&mut self, id: &RequestId) -> Option<CancellationHandle> {
        self.in_flight.remove(id)
    }

    pub(super) fn cancel(&mut self, id: RequestId) {
        if let Some(handle) = self.in_flight.get(&id) {
            handle.cancel();
        } else if self.incoming.contains(&id) {
            self.cancelled.insert(id);
        }
    }

    pub(super) fn take_cancelled(&mut self, id: &RequestId) -> bool {
        self.cancelled.remove(id)
    }
}
