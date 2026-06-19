use std::{any::Any, thread, time::Instant};

use crossbeam_channel::{Receiver, RecvError, select};
use lsp_server::{Connection, Message};

use crate::{
    LaunchConfiguration,
    global_state::GlobalState,
    task::TaskResult,
    tracing::TraceSink,
    transport::{MessageMetadata, RequestProfiler, serialize_json_rpc_message},
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
                let result = state.handle_message(&message, &input);
                let handle_ms = elapsed_ms(handle_start);

                let write_start = Instant::now();
                let summary = state.send_task_result(TaskResult::response(result))?;
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
                let _summary = state.send_task_result(task)?;
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
    use crossbeam_channel::unbounded;

    use crate::{JsonRpcResult, LaunchConfiguration, global_state::GlobalState, task::TaskLane};

    use super::{MAIN_LOOP_THREAD_NAME, MainLoopEvent, next_event, spawn_latency_main_loop_thread};

    #[test]
    fn next_event_receives_background_lane_task_results() {
        let (response_sender, _response_receiver) = unbounded();
        let (_message_sender, message_receiver) = unbounded();
        let state = GlobalState::new(response_sender, LaunchConfiguration::default());

        state.task_scheduler().spawn(TaskLane::Worker, || {
            JsonRpcResult::Response("worker".to_owned())
        });

        let event = next_event(&message_receiver, &state).expect("task event should be selected");

        let MainLoopEvent::Task(task) = event else {
            panic!("expected task event");
        };
        assert_eq!(task.lane(), TaskLane::Worker);
        assert_eq!(
            task.into_result(),
            JsonRpcResult::Response("worker".to_owned())
        );
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
}
