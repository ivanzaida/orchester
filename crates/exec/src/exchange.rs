//! The process side of [`LineExchange`]: a program whose input stays open while it answers, read line by line until
//! the caller has its answer, then asked to end by closing its input.

use std::{
  io::{BufRead, BufReader, Write},
  sync::mpsc::{self, RecvTimeoutError},
  thread,
  time::{Duration, Instant},
};

use orchester_core::command::{Command, CommandError, LineExchange};

use super::{ProcessRunner, read_to_end, spawn, stop};

/// How long a program may take to end once its input is closed before it is stopped.
const CLOSE_GRACE: Duration = Duration::from_secs(5);

impl LineExchange for ProcessRunner {
  fn exchange(&self, command: &Command, done: &mut dyn FnMut(&str) -> bool) -> Result<Vec<String>, CommandError> {
    let mut child = spawn(command)?;
    let mut input = child.stdin.take();
    if let (Some(pipe), Some(text)) = (input.as_mut(), command.stdin()) {
      // A program that ended before reading its input says why in what it printed, which is read below.
      if let Err(error) = pipe.write_all(text.as_bytes()).and_then(|()| pipe.flush()) {
        tracing::debug!(%error, program = %command.program().display(), "a program ended before reading its input");
      }
    }
    // Standard error is drained so that a full pipe cannot stop the program; nothing of it is kept.
    let errors = read_to_end(child.stderr.take());
    let (sender, lines_read) = mpsc::channel();
    if let Some(output) = child.stdout.take() {
      thread::spawn(move || {
        for line in BufReader::new(output).lines() {
          if sender.send(line).is_err() {
            return;
          }
        }
      });
    }
    let deadline = Instant::now() + self.timeout;
    let mut lines = Vec::new();
    loop {
      match lines_read.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(Ok(line)) => {
          let answered = done(&line);
          lines.push(line);
          if answered {
            break;
          }
        }
        // The output ended: the program is gone.
        Err(RecvTimeoutError::Disconnected) => break,
        Ok(Err(error)) => {
          end(&mut child, command);
          return Err(CommandError::Failed(error.to_string()));
        }
        Err(RecvTimeoutError::Timeout) => {
          end(&mut child, command);
          return Err(CommandError::TimedOut);
        }
      }
    }
    // The end of its input asks the program to end; one that does not is stopped.
    drop(input);
    finish(&mut child, command);
    if errors.join().is_err() {
      tracing::warn!(program = %command.program().display(), "the reader of a program's errors panicked");
    }
    Ok(lines)
  }
}

/// Waits [`CLOSE_GRACE`] for the program to end, then stops it.
fn finish(child: &mut std::process::Child, command: &Command) {
  let deadline = Instant::now() + CLOSE_GRACE;
  while Instant::now() < deadline {
    match child.try_wait() {
      Ok(Some(_)) => return,
      Ok(None) => thread::sleep(super::POLL_INTERVAL),
      Err(error) => {
        tracing::warn!(%error, program = %command.program().display(), "cannot wait for a program");
        break;
      }
    }
  }
  end(child, command);
}

fn end(child: &mut std::process::Child, command: &Command) {
  if let Err(error) = stop(child) {
    tracing::warn!(%error, program = %command.program().display(), "cannot stop a program");
  }
}
