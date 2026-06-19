use std::time::Instant;

use lsp_server::Connection;

use crate::{
    LaunchConfiguration,
    global_state::GlobalState,
    transport::{
        MessageMetadata, RequestProfiler, ResultSummary, messages_from_result,
        serialize_json_rpc_message,
    },
};

pub fn run(connection: Connection, configuration: LaunchConfiguration) -> anyhow::Result<()> {
    let mut state = GlobalState::new(configuration);
    let mut profiler = RequestProfiler::from_configuration(state.launch_configuration())?;
    let mut sequence = 0_u64;

    while let Ok(message) = connection.receiver.recv() {
        sequence = sequence.saturating_add(1);
        let metadata = MessageMetadata::from_message(&message);
        let input = serialize_json_rpc_message(&message)?;
        let input_bytes = input.len();
        profiler.begin(sequence, &metadata, input_bytes)?;

        let handle_start = Instant::now();
        let result = state.handle_message(&message, &input);
        let handle_ms = elapsed_ms(handle_start);
        let summary = ResultSummary::from_result(&result);

        let write_start = Instant::now();
        for message in messages_from_result(result)? {
            connection.sender.send(message)?;
        }
        let write_ms = elapsed_ms(write_start);
        profiler.end(
            sequence,
            &metadata,
            input_bytes,
            handle_ms,
            write_ms,
            &summary,
        )?;

        if state.is_exited() {
            break;
        }
    }

    Ok(())
}

fn elapsed_ms(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}
