use std::{any::Any, thread, time::Instant};

use crossbeam_channel::{Receiver, RecvError, select};
use lsp_server::{Connection, Message};

use crate::{
    LaunchConfiguration,
    global_state::GlobalState,
    profile::RequestProfiler,
    task::TaskResult,
    tracing::TraceSink,
    transport::{MessageMetadata, serialize_json_rpc_message},
};

const MAIN_LOOP_THREAD_NAME: &str = "VelaLspMainLoop";

pub fn run_on_latency_thread(
    connection: Connection,
    configuration: LaunchConfiguration,
) -> anyhow::Result<()> {
    join_latency_main_loop(spawn_latency_main_loop_thread(move || {
        run(connection, configuration)
    })?)
}

pub fn run(connection: Connection, configuration: LaunchConfiguration) -> anyhow::Result<()> {
    let Connection { sender, receiver } = connection;
    let mut state = GlobalState::new(sender, configuration);
    let mut profiler = RequestProfiler::from_configuration(state.launch_configuration())?;
    let mut trace = TraceSink::from_configuration(state.launch_configuration())?;
    let mut sequence = 0_u64;

    while let Ok(event) = next_event(&receiver, &state) {
        match event {
            MainLoopEvent::Message(message) => {
                sequence = sequence.saturating_add(1);
                let metadata = MessageMetadata::from_message(&message);
                let input = serialize_json_rpc_message(&message)?;
                let input_bytes = input.len();
                profiler.begin(sequence, &metadata, input_bytes)?;
                trace.message_received(sequence, &metadata, input_bytes)?;

                let handle_start = Instant::now();
                let messages = state.handle_message(&message, &input)?;
                let handle_ms = elapsed_ms(handle_start);

                let write_start = Instant::now();
                let summary = state.send_messages(messages)?;
                let write_ms = elapsed_ms(write_start);
                trace.response_sent(sequence, &metadata, &summary)?;
                profiler.end(
                    sequence,
                    &metadata,
                    input_bytes,
                    handle_ms,
                    write_ms,
                    &summary,
                )?;
            }
            MainLoopEvent::Task(task) => {
                let task_metadata = crate::tracing::TaskTraceMetadata::from_task(&task);
                trace.task_lifecycle(&task)?;
                let task_summary = state.send_task_result(task)?;
                trace.task_result(
                    &task_metadata,
                    task_summary.outcome(),
                    task_summary.summary(),
                )?;
            }
        }

        if state.is_exited() {
            break;
        }
    }

    Ok(())
}

enum MainLoopEvent {
    Message(Message),
    Task(TaskResult),
}

fn next_event(
    receiver: &Receiver<Message>,
    state: &GlobalState,
) -> Result<MainLoopEvent, RecvError> {
    if let Ok(task) = state.task_scheduler().formatting_results().try_recv() {
        return Ok(MainLoopEvent::Task(task));
    }

    select! {
        recv(receiver) -> message => message.map(MainLoopEvent::Message),
        recv(state.task_scheduler().latency_results()) -> task => task.map(MainLoopEvent::Task),
        recv(state.task_scheduler().formatting_results()) -> task => task.map(MainLoopEvent::Task),
        recv(state.task_scheduler().worker_results()) -> task => task.map(MainLoopEvent::Task),
    }
}

fn elapsed_ms(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn spawn_latency_main_loop_thread(
    job: impl FnOnce() -> anyhow::Result<()> + Send + 'static,
) -> std::io::Result<thread::JoinHandle<anyhow::Result<()>>> {
    thread::Builder::new()
        .name(MAIN_LOOP_THREAD_NAME.to_owned())
        .spawn(job)
}

fn join_latency_main_loop(handle: thread::JoinHandle<anyhow::Result<()>>) -> anyhow::Result<()> {
    match handle.join() {
        Ok(result) => result,
        Err(payload) => anyhow::bail!("main loop thread panicked: {}", panic_message(&payload)),
    }
}

fn panic_message(payload: &Box<dyn Any + Send>) -> &str {
    payload
        .downcast_ref::<&'static str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("unknown panic payload")
}

#[cfg(test)]
mod tests {
    use std::{
        thread,
        time::{Duration, Instant},
    };

    use crossbeam_channel::unbounded;
    use lsp_server::{Message, Notification, RequestId, Response};

    use crate::{LaunchConfiguration, global_state::GlobalState, task::TaskLane};

    use super::{MAIN_LOOP_THREAD_NAME, MainLoopEvent, next_event, spawn_latency_main_loop_thread};

    #[test]
    fn next_event_receives_background_lane_task_results() {
        let (response_sender, _response_receiver) = unbounded();
        let (_message_sender, message_receiver) = unbounded();
        let state = GlobalState::new(response_sender, LaunchConfiguration::default());

        state
            .task_scheduler()
            .spawn(TaskLane::Worker, || test_messages("worker"));

        let event = next_event(&message_receiver, &state).expect("task event should be selected");

        let MainLoopEvent::Task(task) = event else {
            panic!("expected task event");
        };
        assert_eq!(task.lane(), TaskLane::Worker);
        assert_response_messages(task.into_messages(), test_response("worker"));
    }

    #[test]
    fn next_event_receives_client_message_while_worker_task_is_pending() {
        let (response_sender, _response_receiver) = unbounded();
        let (message_sender, message_receiver) = unbounded();
        let state = GlobalState::new(response_sender, LaunchConfiguration::default());
        let (task_started_sender, task_started_receiver) = unbounded();
        let (release_task_sender, release_task_receiver) = unbounded::<()>();

        state.task_scheduler().spawn_for_method(
            TaskLane::Worker,
            "textDocument/references",
            move || {
                task_started_sender
                    .send(())
                    .expect("task start signal should send");
                release_task_receiver
                    .recv()
                    .expect("task release signal should be received");
                test_messages("worker")
            },
        );

        task_started_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("worker task should start");

        let cancel_method =
            <lsp_types::notification::Cancel as lsp_types::notification::Notification>::METHOD;
        let cancel_params = serde_json::json!({ "id": 7 });
        let cancel = Message::Notification(Notification {
            method: cancel_method.to_owned(),
            params: cancel_params.clone(),
        });
        message_sender
            .send(cancel.clone())
            .expect("cancel notification should send");

        let event =
            next_event(&message_receiver, &state).expect("message event should be selected");

        let MainLoopEvent::Message(Message::Notification(notification)) = event else {
            panic!("expected message event while worker task is pending");
        };
        assert_eq!(notification.method, cancel_method);
        assert_eq!(notification.params, cancel_params);

        release_task_sender
            .send(())
            .expect("task release signal should send");
        let task = state
            .task_scheduler()
            .worker_results()
            .recv_timeout(Duration::from_secs(1))
            .expect("worker task should finish after release");
        assert_eq!(task.lane(), TaskLane::Worker);
        assert_eq!(task.method(), Some("textDocument/references"));
        assert_response_messages(task.into_messages(), test_response("worker"));
    }

    #[test]
    fn next_event_prioritizes_ready_formatting_task_over_worker_task() {
        let (response_sender, _response_receiver) = unbounded();
        let (_message_sender, message_receiver) = unbounded();
        let state = GlobalState::new(response_sender, LaunchConfiguration::default());

        state.task_scheduler().spawn_for_method(
            TaskLane::Worker,
            "textDocument/references",
            || test_messages("worker"),
        );
        state.task_scheduler().spawn_for_method(
            TaskLane::Formatting,
            "textDocument/formatting",
            || test_messages("formatting"),
        );
        wait_for_ready_task_results(&state);

        let event = next_event(&message_receiver, &state)
            .expect("formatting task event should be selected");

        let MainLoopEvent::Task(task) = event else {
            panic!("expected task event");
        };
        assert_eq!(task.lane(), TaskLane::Formatting);
        assert_eq!(task.method(), Some("textDocument/formatting"));
        assert_response_messages(task.into_messages(), test_response("formatting"));

        let worker = state
            .task_scheduler()
            .worker_results()
            .recv_timeout(Duration::from_secs(1))
            .expect("worker task should remain queued");
        assert_eq!(worker.lane(), TaskLane::Worker);
        assert_eq!(worker.method(), Some("textDocument/references"));
    }

    #[test]
    fn latency_main_loop_thread_is_named_without_custom_stack() {
        let (name_sender, name_receiver) = unbounded();
        let handle = spawn_latency_main_loop_thread(move || {
            name_sender
                .send(std::thread::current().name().map(str::to_owned))
                .expect("thread name should send");
            Ok(())
        })
        .expect("main loop thread should spawn");

        handle
            .join()
            .expect("main loop thread should not panic")
            .expect("main loop thread job should succeed");

        assert_eq!(
            name_receiver
                .recv()
                .expect("thread name should be received")
                .as_deref(),
            Some(MAIN_LOOP_THREAD_NAME)
        );
    }

    fn wait_for_ready_task_results(state: &GlobalState) {
        let deadline = Instant::now() + Duration::from_secs(1);
        while Instant::now() < deadline {
            if !state.task_scheduler().formatting_results().is_empty()
                && !state.task_scheduler().worker_results().is_empty()
            {
                return;
            }
            thread::sleep(Duration::from_millis(1));
        }
        panic!("formatting and worker task results should be ready");
    }

    fn test_response(value: &str) -> Response {
        Response {
            id: RequestId::from(value.to_owned()),
            result: Some(serde_json::json!(value)),
            error: None,
        }
    }

    fn test_messages(value: &str) -> Vec<Message> {
        vec![Message::Response(test_response(value))]
    }

    fn assert_response_messages(messages: Vec<Message>, response: Response) {
        let expected = crate::rpc::serialize_message(&Message::Response(response));
        let actual = messages
            .iter()
            .map(crate::rpc::serialize_message)
            .collect::<Vec<_>>();
        assert_eq!(actual, vec![expected]);
    }
}
