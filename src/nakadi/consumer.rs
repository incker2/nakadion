use nakadi::client::{ConnectError, LineResult, RawLine};
use nakadi::Lifecycle;
use std::thread;
use std::time::{Duration, Instant};
use std::sync::Arc;

use nakadi::CommitStrategy;
use nakadi::handler::HandlerFactory;
use nakadi::client::StreamingClient;
use nakadi::model::*;
use nakadi::committer::Committer;
use nakadi::dispatcher::Dispatcher;
use nakadi::batch::{Batch, BatchLine};

const CONNECT_RETRY_BACKOFF_MS: &'static [u64] = &[
    10, 50, 100, 500, 1000, 1000, 1000, 3000, 3000, 3000, 5000, 5000, 5000, 10_000, 10_000, 10_000,
    15_000, 15_000, 15_000,
];

/// The consumer connects to the stream and sends batch lines to the processor.
///
/// This is the top level component used by an application that wants to consume a
/// Nakadi stream
#[derive(Clone)]
pub struct Consumer {
    lifecycle: Lifecycle,
}

impl Consumer {
    pub fn start<C, HF>(client: C, handler_factory: HF, commit_strategy: CommitStrategy) -> Consumer
    where
        C: StreamingClient + Clone + Send + 'static,
        HF: HandlerFactory + Send + Sync + 'static,
    {
        let lifecycle = Lifecycle::default();

        let consumer = Consumer {
            lifecycle: lifecycle.clone(),
        };

        start_consumer_loop(client, handler_factory, commit_strategy, lifecycle);

        consumer
    }

    pub fn running(&self) -> bool {
        self.lifecycle.running()
    }

    pub fn stop(&self) {
        self.lifecycle.request_abort()
    }
}

fn start_consumer_loop<C, HF>(
    client: C,
    handler_factory: HF,
    commit_strategy: CommitStrategy,
    lifecycle: Lifecycle,
) where
    C: StreamingClient + Clone + Send + 'static,
    HF: HandlerFactory + Send + Sync + 'static,
{
    thread::spawn(move || consumer_loop(client, handler_factory, commit_strategy, lifecycle));
}

fn consumer_loop<C, HF>(
    client: C,
    handler_factory: HF,
    commit_strategy: CommitStrategy,
    lifecycle: Lifecycle,
) where
    C: StreamingClient + Clone + Send + 'static,
    HF: HandlerFactory + Send + Sync + 'static,
{
    let handler_factory = Arc::new(handler_factory);

    loop {
        if lifecycle.abort_requested() {
            info!("Abort requested");
            break;
        }

        info!("Connecting to stream");
        let (stream_id, line_iterator) =
            match connect(&client, Duration::from_secs(300), &lifecycle) {
                Ok(v) => v,
                Err(err) => {
                    if err.is_permanent() {
                        error!("Permanent connection error: {}", err);
                        break;
                    } else {
                        warn!("Temporary connection error: {}", err);
                        continue;
                    }
                }
            };

        info!("Connected to stream {}", stream_id);

        let committer = Committer::start(client.clone(), commit_strategy, stream_id.clone());

        let dispatcher = Dispatcher::start(handler_factory.clone(), committer.clone());

        consume(line_iterator, dispatcher, committer, lifecycle.clone());
    }

    lifecycle.stopped();

    info!("Nakadi consumer stopped");
}

fn consume<I>(line_iterator: I, dispatcher: Dispatcher, committer: Committer, lifecycle: Lifecycle)
where
    I: Iterator<Item = LineResult>,
{
    for line_result in line_iterator {
        if lifecycle.abort_requested() {
            break;
        }
        match line_result {
            Ok(raw_line) => if let Err(err) = send_line(&dispatcher, raw_line) {
                error!("Could not process batch: {}", err);
                break;
            },
            Err(err) => {
                error!("The connection broke: {}", err);
                break;
            }
        }
    }

    info!("Stopping dispatcher");
    dispatcher.stop();

    while dispatcher.is_running() {
        thread::sleep(Duration::from_millis(10));
    }

    info!("Stopping commiter");
    committer.stop();

    while committer.running() {
        thread::sleep(Duration::from_millis(10));
    }
}

fn send_line(dispatcher: &Dispatcher, line: RawLine) -> Result<(), String> {
    let batch_line = BatchLine::new(line.bytes)?;

    if let Some(info) = batch_line.info() {
        match ::std::str::from_utf8(info) {
            Ok(info) => info!("Received info: {}", info),
            Err(err) => warn!("Received info line which is not UTF-8: {}", err),
        };
    }

    if batch_line.is_keep_alive_line() {
        info!("Keep alive!");
        Ok(())
    } else {
        dispatcher.process(Batch {
            batch_line: batch_line,
            received_at: line.received_at,
        })
    }
}

fn connect<C: StreamingClient>(
    client: &C,
    max_dur: Duration,
    lifecycle: &Lifecycle,
) -> Result<(StreamId, C::LineIterator), ConnectError> {
    let deadline = Instant::now() + max_dur;
    let mut attempt = 0;
    loop {
        attempt += 1;
        let flow_id = FlowId::default();
        match client.connect(flow_id.clone()) {
            Ok(it) => return Ok(it),
            Err(err) => {
                let sleep_dur_ms = *CONNECT_RETRY_BACKOFF_MS.get(attempt).unwrap_or(&30_000);
                if Instant::now() >= deadline {
                    return Err(ConnectError::Other(
                        format!("Failed to connect to Nakadi after {} attempts.", attempt),
                        flow_id,
                    ));
                } else if lifecycle.abort_requested() {
                    return Err(ConnectError::Other(
                        format!(
                            "Failed to connect to Nakadi after {} attempts. Abort requested",
                            attempt
                        ),
                        flow_id,
                    ));
                } else {
                    warn!(
                        "Failed to connect(attempt {}) to Nakadi(retry in {}ms): {}",
                        attempt, sleep_dur_ms, err
                    );
                    thread::sleep(Duration::from_millis(sleep_dur_ms));
                }
            }
        }
    }
}
