use crate::JsonRpcResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskResult {
    Response(JsonRpcResult),
}

impl TaskResult {
    pub(crate) const fn response(result: JsonRpcResult) -> Self {
        Self::Response(result)
    }

    pub(crate) fn into_result(self) -> JsonRpcResult {
        match self {
            Self::Response(result) => result,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_result_preserves_json_rpc_result() {
        let result = JsonRpcResult::Response("{\"jsonrpc\":\"2.0\"}".to_owned());

        let task_result = TaskResult::response(result.clone());

        assert_eq!(task_result.into_result(), result);
    }
}
