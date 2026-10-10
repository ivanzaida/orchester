//! One stop of a process tree: close requests, a bounded wait, then an end for survivors. Before the first close
//! request a stop can still be refused as a whole; after it nothing aborts the stop, and every process found gets an
//! outcome in the report.

use std::{
  collections::BTreeSet,
  thread,
  time::{Duration, Instant},
};

use super::{Entry, ProcessError, Refusal, RunProcesses, platform};

const GRACE: Duration = Duration::from_secs(2);
const END_WAIT: Duration = Duration::from_secs(1);
const POLL: Duration = Duration::from_millis(50);
const MAX_TREE: usize = 1024;

/// The tree reached [`MAX_TREE`] processes; the rest of it is not followed.
pub(super) struct TreeBound;

enum State {
  /// Inside the worktree; no close request sent yet.
  Pending,
  /// It had exited by itself before its close request.
  Gone,
  Closing,
  Ending,
  /// Never closed or ended: outside the worktree or protected.
  Left(Refusal),
  Failed(String),
}

struct Member {
  process: platform::Process,
  state: State,
  /// A close request that could not be delivered; the process was still ended after the wait.
  close_error: Option<String>,
}

impl Member {
  fn new(process: platform::Process, state: State) -> Self {
    Self {
      process,
      state,
      close_error: None,
    }
  }

  fn acting(&self) -> bool {
    matches!(self.state, State::Pending | State::Closing | State::Ending)
  }

  /// Whether the process still runs. An identity that can no longer be checked ends this member's stop as failed.
  fn running(&mut self) -> bool {
    match self.process.alive() {
      Ok(alive) => alive,
      Err(error) => {
        self.state = State::Failed(format!("cannot check the process: {error}"));
        false
      }
    }
  }

  fn request_close(&mut self) {
    if !matches!(self.state, State::Pending) {
      return;
    }
    if !self.running() {
      if matches!(self.state, State::Pending) {
        self.state = State::Gone;
      }
      return;
    }
    if let Err(error) = self.process.close() {
      self.close_error = Some(error.to_string());
    }
    self.state = State::Closing;
  }

  fn end(&mut self) {
    if !matches!(self.state, State::Closing) || !self.running() {
      return;
    }
    self.state = match self.process.end() {
      Ok(()) => State::Ending,
      Err(error) => State::Failed(format!("cannot end: {error}")),
    };
  }

  /// The report line, and whether this process needs nothing more.
  fn outcome(&mut self) -> (String, bool) {
    if self.acting() && self.running() {
      self.state = State::Failed("still running after the bounded wait".to_owned());
    }
    let (text, complete) = match &self.state {
      State::Pending | State::Gone => ("already gone".to_owned(), true),
      State::Closing => ("closed".to_owned(), true),
      State::Ending => ("ended".to_owned(), true),
      State::Failed(reason) => (format!("failed: {reason}"), false),
      State::Left(refusal) => (format!("not stopped, {refusal}; {}", self.left_status()), true),
    };
    let info = &self.process.info;
    let mut line = format!("pid {}: {text} ({})", info.pid, info.name());
    if let Some(error) = &self.close_error {
      line.push_str(&format!("; close request failed: {error}"));
    }
    (line, complete)
  }

  fn left_status(&self) -> &'static str {
    match self.process.alive() {
      Ok(true) => "still running",
      Ok(false) => "exited by itself",
      Err(_) => "its state cannot be read",
    }
  }
}

pub(super) struct Stop<'a> {
  owner: &'a RunProcesses,
  protected: BTreeSet<u32>,
  /// The root first, then descendants in the order found: parents before their children.
  members: Vec<Member>,
  /// Descendants that could not be opened, by pid: they are left alone.
  uninspectable: BTreeSet<u32>,
  /// Problems that are not one process's outcome, each once.
  notes: BTreeSet<String>,
}

impl<'a> Stop<'a> {
  /// A stop of the validated `root`; `protected` holds the runner and its ancestors.
  pub(super) fn new(owner: &'a RunProcesses, protected: BTreeSet<u32>, root: platform::Process) -> Self {
    Self {
      owner,
      protected,
      members: vec![Member::new(root, State::Pending)],
      uninspectable: BTreeSet::new(),
      notes: BTreeSet::new(),
    }
  }

  /// Adds the descendants in `entries` not seen yet. Exited parents stay members: a close handler can spawn a child
  /// and exit between snapshots. Descendants outside the worktree are followed too, so their children inside it are
  /// stopped.
  pub(super) fn discover(&mut self, entries: &[Entry]) -> Result<(), TreeBound> {
    let mut index = 0;
    while index < self.members.len() {
      let parent_started = self.members[index].process.info.started;
      for entry in entries {
        if self.members[index].process.is_parent_of(*entry) && !self.known(*entry) {
          self.adopt(*entry, parent_started)?;
        }
      }
      index += 1;
    }
    Ok(())
  }

  pub(super) fn run(mut self, pid: u32) -> Result<String, ProcessError> {
    self.close_from(0);
    self.settle(GRACE, false);
    self.settle(END_WAIT, true);
    let (report, complete) = self.report();
    if complete {
      Ok(report)
    } else {
      Err(ProcessError::Incomplete { pid, report })
    }
  }

  fn known(&self, entry: Entry) -> bool {
    self.uninspectable.contains(&entry.pid) || self.members.iter().any(|member| member.process.is_process(entry))
  }

  fn adopt(&mut self, entry: Entry, parent_started: u64) -> Result<(), TreeBound> {
    let child = match platform::Process::open(entry, true) {
      Ok(Some(child)) => child,
      Ok(None) => return Ok(()),
      Err(_) => {
        self.uninspectable.insert(entry.pid);
        return Ok(());
      }
    };
    // A stale parent PID in a snapshot does not establish descent from a newer process.
    if child.info.started < parent_started {
      return Ok(());
    }
    if self.members.len() >= MAX_TREE {
      return Err(TreeBound);
    }
    let state = match self.owner.validate(&child, &self.protected) {
      Ok(()) => State::Pending,
      Err(refusal) => State::Left(refusal),
    };
    self.members.push(Member::new(child, state));
    Ok(())
  }

  /// Children are asked before their parents.
  fn close_from(&mut self, start: usize) {
    for member in self.members[start..].iter_mut().rev() {
      member.request_close();
    }
  }

  /// Waits up to `wait` for every process inside the worktree to exit, closing any found late, and ends survivors
  /// first when `end` is set. A child first seen while ending still gets its close request before its end.
  fn settle(&mut self, wait: Duration, end: bool) {
    let deadline = Instant::now() + wait;
    loop {
      if end {
        for member in self.members.iter_mut().rev() {
          member.end();
        }
      }
      let settled = self
        .members
        .iter_mut()
        .all(|member| !member.acting() || !member.running());
      let known = self.members.len();
      // Scan after checking exits, so the final snapshot includes children born before those exits.
      self.rediscover();
      self.close_from(known);
      if (settled && known == self.members.len()) || Instant::now() >= deadline {
        return;
      }
      thread::sleep(POLL);
    }
  }

  fn rediscover(&mut self) {
    match platform::snapshot() {
      Ok(entries) => {
        if self.discover(&entries).is_err() {
          self
            .notes
            .insert(format!("{}; processes beyond it were not followed", Refusal::TreeBound));
        }
      }
      Err(error) => {
        self.notes.insert(format!(
          "listing processes failed ({error}); children started late may have been missed"
        ));
      }
    }
  }

  fn report(&mut self) -> (String, bool) {
    let mut complete = self.notes.is_empty();
    let mut lines = Vec::new();
    for member in &mut self.members {
      let (line, done) = member.outcome();
      complete &= done;
      lines.push(line);
    }
    lines.extend(
      self
        .uninspectable
        .iter()
        .map(|pid| format!("pid {pid}: not stopped, {}", Refusal::Unreadable)),
    );
    lines.extend(self.notes.iter().cloned());
    (lines.join("\n"), complete)
  }
}
