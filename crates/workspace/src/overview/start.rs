//! A new project's getting-started list (03 `ArvFR`): connect an agent CLI, publish an agent profile, publish a
//! pipeline and create the first task, each done by the project's real state ([`ProjectReadiness`] and its tasks) and
//! saying what was found.

use crate::{
  messages::Message,
  tasks::ready_to_run::{CheckKind, ProjectReadiness},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartStep {
  Connect,
  Profile,
  Pipeline,
  Task,
}

impl StartStep {
  pub const ALL: [Self; 4] = [Self::Connect, Self::Profile, Self::Pipeline, Self::Task];

  pub fn title(self) -> Message {
    Message::new(match self {
      Self::Connect => "overview.start.connect.title",
      Self::Profile => "overview.start.profile.title",
      Self::Pipeline => "overview.start.pipeline.title",
      Self::Task => "overview.start.task.title",
    })
  }

  /// The step's name for tools and element ids.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Connect => "connect",
      Self::Profile => "profile",
      Self::Pipeline => "pipeline",
      Self::Task => "task",
    }
  }
}

/// A step, whether it is done and what was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartLine {
  pub step: StartStep,
  pub done: bool,
  pub found: Message,
}

/// The four steps of a project with `readiness` and `tasks` tasks; names in a line joined by `separator`.
pub fn steps(readiness: &ProjectReadiness, tasks: usize, separator: &str) -> [StartLine; 4] {
  let checks = readiness.checks(separator);
  let check = |kind: CheckKind| {
    checks
      .iter()
      .find(|check| check.kind == kind)
      .expect("the readiness checks every kind")
      .clone()
  };
  StartStep::ALL.map(|step| {
    let (done, found) = match step {
      StartStep::Connect => {
        let check = check(CheckKind::Connection);
        (check.ready, check.detail)
      }
      StartStep::Profile => found(
        check(CheckKind::Agents).ready,
        check(CheckKind::Agents).detail,
        "overview.start.profile.none",
      ),
      StartStep::Pipeline => found(
        check(CheckKind::Pipeline).ready,
        check(CheckKind::Pipeline).detail,
        "overview.start.pipeline.none",
      ),
      StartStep::Task if tasks > 0 => (
        true,
        Message::new("overview.start.task.done").with("count", tasks.to_string()),
      ),
      StartStep::Task => (false, Message::new("overview.start.task.none")),
    };
    StartLine { step, done, found }
  })
}

/// What a step found: the check's own words once ready, `none` until then.
fn found(ready: bool, detail: Message, none: &'static str) -> (bool, Message) {
  if ready {
    (true, detail)
  } else {
    (false, Message::new(none))
  }
}

/// How many of `lines` are done.
pub fn done(lines: &[StartLine]) -> usize {
  lines.iter().filter(|line| line.done).count()
}
