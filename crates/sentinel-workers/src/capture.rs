//! Reads a worker's stdout and stderr line by line, keeps a bounded tail, and drains the
//! lines that arrive after the child stops.

use std::collections::VecDeque;
use std::time::Duration;

use sentinel_core::TaskId;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time;

use crate::worker::{OutputLine, RunSpec};

pub(crate) const MAX_LINE_CHARS: usize = 4096;
pub(crate) const TAIL_LINES: usize = 200;

#[derive(Clone)]
struct LineSink {
    task: TaskId,
    agent: sentinel_core::ToolId,
    lines: mpsc::UnboundedSender<OutputLine>,
}

pub(crate) struct Capture {
    tail: VecDeque<String>,
    pub(crate) raw: mpsc::UnboundedReceiver<String>,
    readers: Vec<JoinHandle<()>>,
}

pub(crate) fn capture_streams(
    child: &mut Child,
    spec: &RunSpec,
    lines: mpsc::UnboundedSender<OutputLine>,
) -> Capture {
    let (raw_tx, raw) = mpsc::unbounded_channel();
    let sink = LineSink {
        task: spec.task.clone(),
        agent: spec.agent.clone(),
        lines,
    };
    let mut readers = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        readers.push(spawn_reader(stdout, sink.clone(), raw_tx.clone()));
    }
    if let Some(stderr) = child.stderr.take() {
        readers.push(spawn_reader(stderr, sink, raw_tx.clone()));
    }
    drop(raw_tx);
    Capture {
        tail: VecDeque::new(),
        raw,
        readers,
    }
}

fn spawn_reader<R>(reader: R, sink: LineSink, raw: mpsc::UnboundedSender<String>) -> JoinHandle<()>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(read_lines(reader, sink, raw))
}

async fn read_lines<R>(reader: R, sink: LineSink, raw: mpsc::UnboundedSender<String>)
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let text: String = line.chars().take(MAX_LINE_CHARS).collect();
        let message = OutputLine {
            task: sink.task.clone(),
            agent: sink.agent.clone(),
            text: text.clone(),
        };
        let _ = sink.lines.send(message);
        if raw.send(text).is_err() {
            break;
        }
    }
}

impl Capture {
    pub(crate) fn push(&mut self, line: String) {
        if self.tail.len() == TAIL_LINES {
            self.tail.pop_front();
        }
        self.tail.push_back(line);
    }

    /// Reads the lines still in flight, passing each to `on_line`, then stops the readers.
    pub(crate) async fn drain(&mut self, limit: Duration, mut on_line: impl FnMut(&str)) {
        let read_rest = async {
            while let Some(line) = self.raw.recv().await {
                self.push(line.clone());
                on_line(&line);
            }
        };
        let _ = time::timeout(limit, read_rest).await;
        for reader in &self.readers {
            reader.abort();
        }
    }

    pub(crate) fn into_tail(self) -> Vec<String> {
        self.tail.into_iter().collect()
    }
}
