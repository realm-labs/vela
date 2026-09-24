use lsp_server::{Message, RequestId};
use lsp_types::RenameParams;

use super::{GlobalStateSnapshot, documents::snapshot_document_text, reference_projection};
use crate::{ErrorCode, lsp::from_proto};

impl GlobalStateSnapshot {
    pub(crate) fn rename(self, id: RequestId, params: RenameParams) -> Vec<Message> {
        let document_id = from_proto::document_id(&params.text_document_position.text_document.uri);
        let text = snapshot_document_text(&self, &document_id);
        let input = match from_proto::rename_params(&text, &params) {
            Ok(input) => input,
            Err(error) => {
                return super::responses::error(
                    id,
                    ErrorCode::InvalidRequest,
                    format!("invalid rename position: {error}"),
                );
            }
        };
        let edit = self
            .databases
            .rename(&input.document_id, input.position, &params.new_name);
        if edit.is_none()
            && self
                .databases
                .prepare_rename(&input.document_id, input.position)
                .is_some()
        {
            if let Some(error) = self.databases.rename_name_error(
                &input.document_id,
                input.position,
                &params.new_name,
            ) {
                return super::responses::error(id, ErrorCode::InvalidParams, error);
            }
            return super::responses::error(
                id,
                ErrorCode::InvalidRequest,
                format!(
                    "rename to `{}` was rejected due to a conflicting declaration or unsafe reference resolution",
                    params.new_name
                ),
            );
        }

        reference_projection::respond(
            id,
            edit.as_ref()
                .map(|edit| reference_projection::edit(&self, edit))
                .transpose(),
            "typed rename response",
        )
    }
}
