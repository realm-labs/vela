use std::thread;

use crossbeam_channel::{Receiver, Sender, unbounded};
use vela_language_service::GenerationToken;

use crate::{JsonRpcResult, RequestId};

type TaskJob = Box<dyn FnOnce() -> TaskResult + Send + 'static>;

#[derive(Debug, Clone)]
pub(crate) enum TaskResult {
    Response {
        lane: TaskLane,
        method: Option<String>,
        request_id: Option<RequestId>,
        generation: Option<GenerationToken>,
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

pub(crate) struct TaskScheduler {
    latency_jobs: Sender<TaskJob>,
    formatting_jobs: Sender<TaskJob>,
    worker_jobs: Sender<TaskJob>,
    latency_results: Receiver<TaskResult>,
    formatting_results: Receiver<TaskResult>,
    worker_results: Receiver<TaskResult>,
}

impl TaskScheduler {
    pub(crate) fn new() -> Self {
        let (latency_jobs, latency_results) = spawn_lane_worker(TaskLane::Latency);
        let (formatting_jobs, formatting_results) = spawn_lane_worker(TaskLane::Formatting);
        let (worker_jobs, worker_results) = spawn_lane_worker(TaskLane::Worker);
        Self {
            latency_jobs,
            formatting_jobs,
            worker_jobs,
            latency_results,
            formatting_results,
            worker_results,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn spawn(
        &self,
        lane: TaskLane,
        job: impl FnOnce() -> JsonRpcResult + Send + 'static,
    ) {
        self.spawn_labeled(lane, None, None, None, job);
    }

    #[allow(dead_code)]
    pub(crate) fn spawn_for_method(
        &self,
        lane: TaskLane,
        method: impl Into<String>,
        job: impl FnOnce() -> JsonRpcResult + Send + 'static,
    ) {
        self.spawn_labeled(lane, Some(method.into()), None, None, job);
    }

    #[allow(dead_code)]
    pub(crate) fn spawn_for_request(
        &self,
        lane: TaskLane,
        method: impl Into<String>,
        request_id: RequestId,
        generation: GenerationToken,
        job: impl FnOnce() -> JsonRpcResult + Send + 'static,
    ) {
        self.spawn_labeled(
            lane,
            Some(method.into()),
            Some(request_id),
            Some(generation),
            job,
        );
    }

    fn spawn_labeled(
        &self,
        lane: TaskLane,
        method: Option<String>,
        request_id: Option<RequestId>,
        generation: Option<GenerationToken>,
        job: impl FnOnce() -> JsonRpcResult + Send + 'static,
    ) {
        let task = Box::new(move || {
            TaskResult::lane_method_request_generation_response(
                lane,
                method,
                request_id,
                generation,
                job(),
            )
        });
        match lane {
            TaskLane::Latency => self.latency_jobs.send(task),
            TaskLane::Formatting => self.formatting_jobs.send(task),
            TaskLane::Worker => self.worker_jobs.send(task),
            TaskLane::Main => unreachable!("main-thread work should not be scheduled as a task"),
        }
        .expect("task lane worker should be alive");
    }

    pub(crate) const fn latency_results(&self) -> &Receiver<TaskResult> {
        &self.latency_results
    }

    pub(crate) const fn formatting_results(&self) -> &Receiver<TaskResult> {
        &self.formatting_results
    }

    pub(crate) const fn worker_results(&self) -> &Receiver<TaskResult> {
        &self.worker_results
    }
}

impl Default for TaskScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskResult {
    pub(crate) fn response(result: JsonRpcResult) -> Self {
        Self::lane_response(TaskLane::Main, result)
    }

    pub(crate) fn lane_response(lane: TaskLane, result: JsonRpcResult) -> Self {
        Self::lane_method_response(lane, None, result)
    }

    pub(crate) fn lane_method_response(
        lane: TaskLane,
        method: Option<String>,
        result: JsonRpcResult,
    ) -> Self {
        Self::lane_method_request_generation_response(lane, method, None, None, result)
    }

    pub(crate) fn lane_method_request_generation_response(
        lane: TaskLane,
        method: Option<String>,
        request_id: Option<RequestId>,
        generation: Option<GenerationToken>,
        result: JsonRpcResult,
    ) -> Self {
        Self::Response {
            lane,
            method,
            request_id,
            generation,
            result,
        }
    }

    pub(crate) const fn lane(&self) -> TaskLane {
        match self {
            Self::Response { lane, .. } => *lane,
        }
    }

    pub(crate) fn method(&self) -> Option<&str> {
        match self {
            Self::Response { method, .. } => method.as_deref(),
        }
    }

    pub(crate) fn request_id(&self) -> Option<&RequestId> {
        match self {
            Self::Response { request_id, .. } => request_id.as_ref(),
        }
    }

    pub(crate) fn generation_token(&self) -> Option<&GenerationToken> {
        match self {
            Self::Response { generation, .. } => generation.as_ref(),
        }
    }

    pub(crate) fn into_result(self) -> JsonRpcResult {
        match self {
            Self::Response { result, .. } => result,
        }
    }
}

fn spawn_lane_worker(lane: TaskLane) -> (Sender<TaskJob>, Receiver<TaskResult>) {
    let (job_sender, job_receiver) = unbounded::<TaskJob>();
    let (result_sender, result_receiver) = unbounded::<TaskResult>();
    thread::Builder::new()
        .name(format!("VelaLsp{lane:?}Task"))
        .spawn(move || {
            while let Ok(job) = job_receiver.recv() {
                if result_sender.send(job()).is_err() {
                    break;
                }
            }
        })
        .expect("task lane worker should spawn");
    (job_sender, result_receiver)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use vela_language_service::LanguageServiceDatabases;

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
        assert_eq!(
            TaskResult::lane_method_response(
                TaskLane::Worker,
                Some("textDocument/hover".to_owned()),
                result.clone(),
            )
            .method(),
            Some("textDocument/hover")
        );
        assert!(task_result.request_id().is_none());
        assert!(task_result.generation_token().is_none());
        assert_eq!(task_result.into_result(), result);
    }

    #[test]
    fn task_scheduler_executes_lane_jobs_on_background_workers() {
        let scheduler = TaskScheduler::new();

        scheduler.spawn(TaskLane::Latency, || {
            JsonRpcResult::Response("latency".to_owned())
        });
        scheduler.spawn(TaskLane::Formatting, || {
            JsonRpcResult::Response("formatting".to_owned())
        });
        scheduler.spawn(TaskLane::Worker, || {
            JsonRpcResult::Response("worker".to_owned())
        });

        let latency = scheduler
            .latency_results()
            .recv_timeout(Duration::from_secs(1))
            .expect("latency lane should respond");
        let formatting = scheduler
            .formatting_results()
            .recv_timeout(Duration::from_secs(1))
            .expect("formatting lane should respond");
        let worker = scheduler
            .worker_results()
            .recv_timeout(Duration::from_secs(1))
            .expect("worker lane should respond");

        assert_eq!(latency.lane(), TaskLane::Latency);
        assert_eq!(formatting.lane(), TaskLane::Formatting);
        assert_eq!(worker.lane(), TaskLane::Worker);
        assert_eq!(
            latency.into_result(),
            JsonRpcResult::Response("latency".to_owned())
        );
        assert_eq!(
            formatting.into_result(),
            JsonRpcResult::Response("formatting".to_owned())
        );
        assert_eq!(
            worker.into_result(),
            JsonRpcResult::Response("worker".to_owned())
        );
    }

    #[test]
    fn task_scheduler_preserves_method_names_for_profiled_tasks() {
        let scheduler = TaskScheduler::new();

        scheduler.spawn_for_method(TaskLane::Worker, "textDocument/references", || {
            assert_eq!(
                thread::current().name(),
                Some("VelaLspWorkerTask"),
                "worker thread should still identify the lane"
            );
            JsonRpcResult::Response("references".to_owned())
        });

        let task = scheduler
            .worker_results()
            .recv_timeout(Duration::from_secs(1))
            .expect("worker lane should respond");

        assert_eq!(task.lane(), TaskLane::Worker);
        assert_eq!(task.method(), Some("textDocument/references"));
        assert_eq!(
            task.into_result(),
            JsonRpcResult::Response("references".to_owned())
        );
    }

    #[test]
    fn task_scheduler_preserves_request_id_and_generation_token() {
        let scheduler = TaskScheduler::new();
        let databases = LanguageServiceDatabases::new();
        let (token, _handle) = databases.begin_cancellable_background_request();
        let request_id = RequestId::String("fmt-1".to_owned());

        scheduler.spawn_for_request(
            TaskLane::Formatting,
            "textDocument/formatting",
            request_id.clone(),
            token.clone(),
            || JsonRpcResult::Response("formatted".to_owned()),
        );

        let task = scheduler
            .formatting_results()
            .recv_timeout(Duration::from_secs(1))
            .expect("formatting lane should respond");

        assert_eq!(task.lane(), TaskLane::Formatting);
        assert_eq!(task.method(), Some("textDocument/formatting"));
        assert_eq!(task.request_id(), Some(&request_id));
        let generation = task
            .generation_token()
            .expect("request task should carry generation token");
        assert_eq!(generation.generation(), token.generation());
        assert!(!generation.is_cancelled());
        assert_eq!(
            task.into_result(),
            JsonRpcResult::Response("formatted".to_owned())
        );
    }
}
