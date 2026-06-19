use std::collections::BTreeSet;

use lsp_server::Message;

use crate::{JsonRpcResult, LaunchConfiguration, LspServer, RequestId, handlers::dispatch};

pub(crate) struct GlobalState {
    launch_configuration: LaunchConfiguration,
    request_queue: RequestQueue,
    server: LspServer,
}

impl GlobalState {
    pub(crate) fn new(launch_configuration: LaunchConfiguration) -> Self {
        let server = LspServer::with_launch_configuration(launch_configuration.clone());
        Self {
            launch_configuration,
            request_queue: RequestQueue::default(),
            server,
        }
    }

    pub(crate) const fn launch_configuration(&self) -> &LaunchConfiguration {
        &self.launch_configuration
    }

    pub(crate) fn handle_message(&mut self, message: &Message, input: &str) -> JsonRpcResult {
        let request_id = RequestQueue::request_id(message);
        if let Some(id) = request_id.as_ref() {
            self.request_queue.start(id.clone());
        }
        let result = dispatch::dispatch_message(self, message, input);
        if let Some(id) = request_id {
            self.request_queue.finish(&id);
        }
        result
    }

    pub(crate) const fn is_exited(&self) -> bool {
        self.server.is_exited()
    }

    pub(crate) const fn is_initialized(&self) -> bool {
        self.server.is_initialized()
    }

    pub(crate) const fn is_shutdown_requested(&self) -> bool {
        self.server.is_shutdown_requested()
    }

    pub(crate) fn take_cancelled_request(&mut self, id: &RequestId) -> bool {
        self.server.take_cancelled_request(id)
    }

    pub(crate) fn initialize(
        &mut self,
        id: lsp_server::RequestId,
        params: lsp_types::InitializeParams,
    ) -> JsonRpcResult {
        self.server.initialize_lsp(id, params)
    }

    pub(crate) fn handle_legacy_json(&mut self, input: &str) -> JsonRpcResult {
        self.server.handle_json(input)
    }
}

#[derive(Debug, Default)]
struct RequestQueue {
    incoming: BTreeSet<String>,
}

impl RequestQueue {
    fn request_id(message: &Message) -> Option<String> {
        match message {
            Message::Request(request) => Some(request.id.to_string()),
            Message::Response(_) | Message::Notification(_) => None,
        }
    }

    fn start(&mut self, id: String) {
        self.incoming.insert(id);
    }

    fn finish(&mut self, id: &str) {
        self.incoming.remove(id);
    }
}
