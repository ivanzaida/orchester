//! Where and how a reviewer's CLI (Claude Code or Codex) runs. It runs in an empty folder the runner owns,
//! removed after the review ([`ReviewPlace`]), never in the project's repository, so nothing of the project (its
//! CLAUDE.md, its git status) reaches it. It is supervised like an agent ([`Supervised`]): contained with everything
//! it starts, stopped after the review's time, and stopped at once when the review is no longer wanted
//! (the person answered, the step or run ended, the runner exits), as the worker then drops the review's
//! [`StopReview`].

use std::{collections::VecDeque, io, time::Duration};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  connection::{reviewer::ReviewFolder, stream::OutputStream},
  run::review::{REVIEW_INSTRUCTIONS, REVIEW_SCHEMA},
};
use tempfile::TempDir;
use tokio::{runtime::Handle, sync::watch};

use crate::supervisor::{ProcessEvent, ProcessExit, Supervised, SupervisorError};

/// The most of each output stream of the reviewer that is kept: its last lines, as its answer ends it (Claude
/// Code's `result` line, Codex's final message and `turn.completed`), however many events came before.
const OUTPUT_MAX_BYTES: usize = 256 * 1024;

/// How long an ended reviewer may take to report its exit. Something it started that left its process group
/// (macOS) may hold its output open past its end; the review then stops waiting, so the step and the run are
/// never held up by it, and the review counts as stopped, its spend unknown.
const END_WAIT: Duration = Duration::from_secs(10);

/// Dropped, it stops the review it was made with.
pub(super) struct StopReview(#[expect(dead_code, reason = "held only to be dropped")] watch::Sender<()>);

/// What the review's process watches for being stopped.
pub(super) fn stop_review() -> (StopReview, watch::Receiver<()>) {
  let (sender, receiver) = watch::channel(());
  (StopReview(sender), receiver)
}

/// An empty folder for one review, with the reviewer's system prompt and the verdict's schema beside it;
/// removed when dropped.
pub(super) struct ReviewPlace {
  root: TempDir,
}

impl ReviewPlace {
  pub(super) fn new() -> io::Result<Self> {
    let root = tempfile::Builder::new().prefix("orchester-review-").tempdir()?;
    let place = Self { root };
    let folder = place.folder();
    std::fs::create_dir(&folder.work)?;
    std::fs::write(&folder.instructions, REVIEW_INSTRUCTIONS)?;
    std::fs::write(&folder.schema, REVIEW_SCHEMA)?;
    Ok(place)
  }

  pub(super) fn folder(&self) -> ReviewFolder {
    ReviewFolder {
      work: self.root.path().join("work"),
      instructions: self.root.path().join("instructions.md"),
      schema: self.root.path().join("schema.json"),
    }
  }
}

/// Runs the reviewer's CLI from the blocking pool on the runner's runtime, supervised.
pub(super) struct SupervisedRunner {
  pub(super) runtime: Handle,
  pub(super) timeout: Duration,
  pub(super) stop: watch::Receiver<()>,
}

impl CommandRunner for SupervisedRunner {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    self.runtime.block_on(async {
      let mut process = Supervised::start(command).map_err(|error| start_error(&error))?;
      let mut stop = self.stop.clone();
      let error = tokio::select! {
        output = tokio::time::timeout(self.timeout, collect(&mut process)) => match output {
          Ok(output) => return output,
          Err(_) => CommandError::TimedOut,
        },
        // The worker dropped the review.
        _ = stop.changed() => CommandError::Failed("the review is no longer wanted".to_owned()),
      };
      end(&mut process).await;
      Err(error)
    })
  }
}

/// Why the reviewer's CLI did not start. It ran nothing unless its process was created: a review's command
/// takes its input whole, so `Spawn` is never a created child left without its standard input. One that could
/// not be contained may have run.
fn start_error(error: &SupervisorError) -> CommandError {
  match error {
    SupervisorError::Launch { .. } | SupervisorError::Spawn { .. } => CommandError::NotStarted(error.to_string()),
    SupervisorError::Contain { .. } => CommandError::Failed(error.to_string()),
  }
}

/// Ends `process` and everything it started, and waits until it has exited, so its folder can be removed; at
/// most [`END_WAIT`].
async fn end(process: &mut Supervised) {
  process.terminate();
  let exited = async {
    while let Some(event) = process.next().await {
      if matches!(event, ProcessEvent::Exited(_)) {
        return;
      }
    }
  };
  if tokio::time::timeout(END_WAIT, exited).await.is_err() {
    tracing::warn!(
      wait = ?END_WAIT,
      "the ended reviewer did not report its exit; the review goes on without it"
    );
  }
}

/// The process's output and exit code, once it exits.
async fn collect(process: &mut Supervised) -> Result<CommandOutput, CommandError> {
  let mut stdout = Tail::default();
  let mut stderr = Tail::default();
  while let Some(event) = process.next().await {
    match event {
      ProcessEvent::Output { stream, line } => match stream {
        OutputStream::Stdout => stdout.push(line),
        OutputStream::Stderr => stderr.push(line),
      },
      ProcessEvent::Exited(ProcessExit::Exited { code }) => {
        return Ok(CommandOutput {
          code,
          stdout: stdout.text(),
          stderr: stderr.text(),
        });
      }
      ProcessEvent::Exited(ProcessExit::Terminated) => {
        return Err(CommandError::Failed("the reviewer was ended".to_owned()));
      }
      ProcessEvent::Exited(ProcessExit::WaitFailed { reason }) => return Err(CommandError::Failed(reason)),
    }
  }
  Err(CommandError::Failed("the reviewer's exit was not reported".to_owned()))
}

/// The last lines of an output stream, [`OUTPUT_MAX_BYTES`] at most; earlier lines give way to later ones.
#[derive(Default)]
struct Tail {
  lines: VecDeque<String>,
  bytes: usize,
}

impl Tail {
  fn push(&mut self, line: String) {
    // A line longer than the whole room is no reviewer's answer: it is left out.
    if line.len() >= OUTPUT_MAX_BYTES {
      return;
    }
    self.bytes += line.len() + 1;
    self.lines.push_back(line);
    while self.bytes > OUTPUT_MAX_BYTES {
      let dropped = self.lines.pop_front().expect("the kept lines take more than the room");
      self.bytes -= dropped.len() + 1;
    }
  }

  fn text(&self) -> String {
    let mut text = String::with_capacity(self.bytes);
    for line in &self.lines {
      text.push_str(line);
      text.push('\n');
    }
    text
  }
}
