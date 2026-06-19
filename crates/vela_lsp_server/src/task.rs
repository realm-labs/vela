use crate::JsonRpcResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskResult {
    Response {
        lane: TaskLane,
        result: JsonRpcResult,
    },
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskLane {
    Main,
    Latency,
    Formatting,
    Worker,
}

impl TaskResult {
    pub(crate) const fn response(result: JsonRpcResult) -> Self {
        Self::lane_response(TaskLane::Main, result)
    }

    pub(crate) const fn lane_response(lane: TaskLane, result: JsonRpcResult) -> Self {
        Self::Response { lane, result }
    }

    pub(crate) const fn lane(&self) -> TaskLane {
        match self {
            Self::Response { lane, .. } => *lane,
        }
    }

    pub(crate) fn into_result(self) -> JsonRpcResult {
        match self {
            Self::Response { result, .. } => result,
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

        assert_eq!(task_result.lane(), TaskLane::Main);
        assert_eq!(
            TaskResult::lane_response(TaskLane::Latency, result.clone()).lane(),
            TaskLane::Latency
        );
        assert_eq!(
            TaskResult::lane_response(TaskLane::Formatting, result.clone()).lane(),
            TaskLane::Formatting
        );
        assert_eq!(
            TaskResult::lane_response(TaskLane::Worker, result.clone()).lane(),
            TaskLane::Worker
        );
        assert_eq!(task_result.into_result(), result);
    }
}
