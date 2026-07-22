use vela_common::ScalarValue;

use crate::error::{HostError, HostErrorKind, HostResult};
use crate::protocol::{HostCollectionMutation, HostCollectionQuery};
use crate::value::HostValue;

pub(super) fn collection_query_result(
    len: usize,
    query: HostCollectionQuery,
) -> HostResult<HostValue> {
    match query {
        HostCollectionQuery::Len => i64::try_from(len)
            .map(|len| HostValue::Scalar(ScalarValue::I64(len)))
            .map_err(|_| invalid_arg("collection length within i64 range")),
        HostCollectionQuery::IsEmpty => Ok(HostValue::Bool(len == 0)),
    }
}

pub(super) fn unsupported_collection_query(query: HostCollectionQuery) -> HostError {
    HostError {
        kind: HostErrorKind::UnsupportedCollectionQuery { query },
        source_span: None,
    }
}

pub(super) fn unsupported_collection_mutation(mutation: HostCollectionMutation) -> HostError {
    HostError {
        kind: HostErrorKind::UnsupportedCollectionMutation { mutation },
        source_span: None,
    }
}

fn invalid_arg(expected: &'static str) -> HostError {
    HostError {
        kind: HostErrorKind::InvalidArgument { expected },
        source_span: None,
    }
}
