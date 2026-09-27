//! The session's background operations as the status bar shows them (`pmOpt`, `E1msJS`):
//! what runs now, what finished, and which failures still wait for the user. Plain data with
//! explicit times, so the rules can be tested without a clock.

use std::{
  path::PathBuf,
  sync::Arc,
  time::{Instant, SystemTime},
};

use orchester_core::id::ResourceId;

use crate::messages::Message;

/// Identifies one operation and the event it ends with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OperationId(u64);

/// Where in a project an operation's object is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
  Overview,
  Tasks,
  PullRequests,
  Pipeline(ResourceId),
  AgentProfile(ResourceId),
  Tool(ResourceId),
  Connections,
  Settings,
}

/// What opening an operation or its event shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
  /// Nothing to open, e.g. a folder dialog.
  Nothing,
  /// The runner log of the open project.
  RunnerLog,
  Project {
    repo_root: PathBuf,
    place: Place,
  },
}

/// How an operation is described before it starts: while it runs, once it finished, and
/// the heading of its failure (the failure's reason is added when it fails).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationSpec {
  project: Option<Arc<str>>,
  running: Message,
  done: Message,
  failed: Message,
  target: Target,
  quiet: bool,
}

impl lurq::app::component::DevtoolsInspectable for OperationSpec {}

impl OperationSpec {
  pub fn new(running: Message, done: Message, failed: Message) -> Self {
    Self {
      project: None,
      running,
      done,
      failed,
      target: Target::Nothing,
      quiet: false,
    }
  }

  /// The project the operation belongs to, by name.
  pub fn project(mut self, name: &str) -> Self {
    self.project = Some(name.into());
    self
  }

  pub fn target(mut self, target: Target) -> Self {
    self.target = target;
    self
  }

  /// Not listed while it runs, only its outcome is recorded: local draft saves are too fast
  /// to follow and appear only as finished events.
  pub fn quiet(mut self) -> Self {
    self.quiet = true;
    self
  }
}

/// An operation in progress.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Operation {
  id: OperationId,
  spec: OperationSpec,
  started: Instant,
}

impl Operation {
  pub fn id(&self) -> OperationId {
    self.id
  }

  pub fn title(&self) -> &Message {
    &self.spec.running
  }

  pub fn project(&self) -> Option<&str> {
    self.spec.project.as_deref()
  }

  pub fn target(&self) -> &Target {
    &self.spec.target
  }

  pub fn started(&self) -> Instant {
    self.started
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
  Finished,
  /// Ended without doing what it set out to, and says so without a failure to dismiss.
  NotDone,
  /// `resolved` once the user opened the failure's object or dismissed it.
  Failed {
    reason: Message,
    resolved: bool,
  },
}

/// A finished or failed operation, or something that happened on its own (the runner
/// reconnected).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
  id: OperationId,
  project: Option<Arc<str>>,
  title: Message,
  target: Target,
  at: SystemTime,
  outcome: Outcome,
}

impl Event {
  pub fn id(&self) -> OperationId {
    self.id
  }

  pub fn project(&self) -> Option<&str> {
    self.project.as_deref()
  }

  pub fn title(&self) -> &Message {
    &self.title
  }

  pub fn target(&self) -> &Target {
    &self.target
  }

  pub fn at(&self) -> SystemTime {
    self.at
  }

  pub fn outcome(&self) -> &Outcome {
    &self.outcome
  }

  /// A failure the user has neither opened nor dismissed.
  pub fn is_unresolved(&self) -> bool {
    matches!(self.outcome, Outcome::Failed { resolved: false, .. })
  }

  /// The same thing happened again, e.g. another save of the same draft.
  fn repeats(&self, other: &Self) -> bool {
    !matches!(self.outcome, Outcome::Failed { .. })
      && self.outcome == other.outcome
      && self.title == other.title
      && self.target == other.target
      && self.project == other.project
  }
}

/// What the activity item shows, by the priority of `Status bar` in the design doc (the
/// runner problem, which outranks all of these, is decided with the runner state in `bar`).
#[derive(Debug, PartialEq, Eq)]
pub enum Slot<'a> {
  /// The newest unresolved failure.
  Failure(&'a Event),
  /// Operations in progress: the newest and how many.
  Running {
    newest: &'a Operation,
    count: usize,
  },
  /// The latest event, until the next one replaces it.
  Latest(&'a Event),
  Empty,
}

/// Operations and events of this session. Nothing is kept after the application closes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ActivityLog {
  next: u64,
  /// Oldest first.
  running: Vec<Operation>,
  /// Newest first.
  events: Vec<Event>,
}

impl lurq::app::component::DevtoolsInspectable for ActivityLog {}

impl ActivityLog {
  pub fn start(&mut self, spec: OperationSpec, now: Instant) -> OperationId {
    let id = self.next_id();
    self.running.push(Operation { id, spec, started: now });
    id
  }

  pub fn finish(&mut self, id: OperationId, at: SystemTime) {
    self.finish_at(id, None, at);
  }

  /// Finishes with `target` instead of the one given at the start, for work whose object
  /// exists only once it finished (a created pipeline).
  pub fn finish_at(&mut self, id: OperationId, target: Option<Target>, at: SystemTime) {
    self.finish_event(id, None, target, at);
  }

  /// Finishes as not done, with `title` instead of the spec's done title, for work that ended
  /// without doing what it set out to and says so without a failure the user must dismiss (a pull request sync
  /// GitHub refused, repeated every minute while the screen is open).
  pub fn finish_as(&mut self, id: OperationId, title: Message, at: SystemTime) {
    self.finish_event(id, Some(title), None, at);
  }

  /// A `title` replacing the done title means the work was not done.
  fn finish_event(&mut self, id: OperationId, title: Option<Message>, target: Option<Target>, at: SystemTime) {
    if let Some(operation) = self.take(id) {
      let OperationSpec {
        project,
        done,
        target: planned,
        ..
      } = operation.spec;
      let target = target.unwrap_or(planned);
      let outcome = if title.is_some() {
        Outcome::NotDone
      } else {
        Outcome::Finished
      };
      self.push(Event {
        id,
        project,
        title: title.unwrap_or(done),
        target,
        at,
        outcome,
      });
    }
  }

  pub fn fail(&mut self, id: OperationId, reason: Message, at: SystemTime) {
    if let Some(operation) = self.take(id) {
      let OperationSpec {
        project,
        failed,
        target,
        ..
      } = operation.spec;
      self.push(Event {
        id,
        project,
        title: failed,
        target,
        at,
        outcome: Outcome::Failed {
          reason,
          resolved: false,
        },
      });
    }
  }

  /// The work was abandoned before it ended (its screen closed); it leaves no event.
  pub fn cancel(&mut self, id: OperationId) {
    self.take(id);
  }

  /// Something that finished without being started here, e.g. the runner reconnecting.
  pub fn record(&mut self, title: Message, target: Target, at: SystemTime) {
    let id = self.next_id();
    self.push(Event {
      id,
      project: None,
      title,
      target,
      at,
      outcome: Outcome::Finished,
    });
  }

  /// The user opened the failure's object or dismissed it; it stays in the recent list.
  pub fn resolve(&mut self, id: OperationId) {
    for event in &mut self.events {
      if event.id == id
        && let Outcome::Failed { resolved, .. } = &mut event.outcome
      {
        *resolved = true;
      }
    }
  }

  /// Listed operations in progress, newest first.
  pub fn running(&self) -> impl Iterator<Item = &Operation> {
    self.running.iter().rev().filter(|operation| !operation.spec.quiet)
  }

  /// Events for the popover's RECENT list: unresolved failures pinned first, then everything
  /// else newest first.
  pub fn recent(&self) -> Vec<&Event> {
    let (mut pinned, rest): (Vec<&Event>, Vec<&Event>) = self.events.iter().partition(|event| event.is_unresolved());
    pinned.extend(rest);
    pinned
  }

  pub fn slot(&self) -> Slot<'_> {
    if let Some(failure) = self.events.iter().find(|event| event.is_unresolved()) {
      return Slot::Failure(failure);
    }
    let mut running = self.running();
    if let Some(newest) = running.next() {
      return Slot::Running {
        newest,
        count: 1 + running.count(),
      };
    }
    self.events.first().map_or(Slot::Empty, Slot::Latest)
  }

  fn next_id(&mut self) -> OperationId {
    self.next += 1;
    OperationId(self.next)
  }

  fn take(&mut self, id: OperationId) -> Option<Operation> {
    let index = self.running.iter().position(|operation| operation.id == id)?;
    Some(self.running.remove(index))
  }

  /// Newest first; a repeat of the newest finished event replaces it, so autosaves of one
  /// draft do not fill the list.
  fn push(&mut self, event: Event) {
    if self.events.first().is_some_and(|newest| newest.repeats(&event)) {
      self.events.remove(0);
    }
    self.events.insert(0, event);
  }
}
