//! How a finished command reads in its row and in its tool group's summary: its exit code when the CLI reported one,
//! otherwise *failed* or *no exit code* (Claude Code reports none), never an empty exit code.

use lurq::app::App;
use orchester_core::{
  connection::stream::{AgentEvent, CommandStatus},
  id::StepExecutionId,
};
use orchester_desktop::{
  i18n,
  messages::Message,
  tasks::{
    timeline::ToolGroup,
    transcript::{Entry, EntryKind},
  },
};

use crate::transcript;

/// `command` started, and ended a second later in `status` with `exit_code`.
fn ran(command: &str, status: CommandStatus, exit_code: Option<i64>) -> [AgentEvent; 2] {
  [
    AgentEvent::CommandStarted {
      item: command.into(),
      command: command.into(),
    },
    AgentEvent::CommandFinished {
      item: command.into(),
      command: command.into(),
      output: String::new(),
      exit_code,
      status,
    },
  ]
}

fn english() -> App {
  let app = App::new();
  i18n::install(&app).expect("bundled locales load");
  app
}

fn text(app: &App, message: &Message) -> String {
  app
    .i18n()
    .t_args(
      message.key,
      message.args.iter().map(|(name, value)| (*name, value.as_str())),
    )
    .to_string()
}

/// What the row of the only command of `events` says of its end.
fn row(app: &App, events: [AgentEvent; 2]) -> String {
  let entries = transcript(StepExecutionId::generate(), events.to_vec());
  let [
    Entry {
      kind: EntryKind::Command(command),
      ..
    },
  ] = entries.as_slice()
  else {
    panic!("one command entry: {entries:?}");
  };
  text(app, &command.ended().expect("the command ended"))
}

/// What the summary of a group of `commands` says of its first failed one.
fn group_failure(app: &App, commands: &[[AgentEvent; 2]]) -> String {
  let entries = transcript(StepExecutionId::generate(), commands.concat());
  let failure = ToolGroup::new(entries).summary.failure().expect("a command failed");
  text(app, &failure)
}

#[test]
fn timeline_command_end_reads_failed_without_an_exit_code() {
  let app = english();
  assert_eq!(
    row(&app, ran("cargo test", CommandStatus::Failed, None)),
    "failed · 1.0 s"
  );
  assert_eq!(
    row(&app, ran("cargo test", CommandStatus::Completed, None)),
    "no exit code · 1.0 s"
  );
  assert_eq!(
    row(&app, ran("cargo test", CommandStatus::Failed, Some(101))),
    "exit 101 · 1.0 s"
  );
  assert_eq!(
    row(&app, ran("cargo test", CommandStatus::Completed, Some(0))),
    "exit 0 · 1.0 s"
  );
}

#[test]
fn timeline_command_end_in_a_group_summary_names_a_failure_without_an_exit_code() {
  let app = english();
  let failed = ran("cargo test", CommandStatus::Failed, None);
  assert_eq!(group_failure(&app, &[failed.clone()]), "failed");
  let lint = ran("pnpm lint", CommandStatus::Completed, None);
  let build = ran("pnpm build", CommandStatus::Completed, None);
  assert_eq!(
    group_failure(&app, &[lint.clone(), build.clone(), failed]),
    "cargo test · failed"
  );
  assert_eq!(
    group_failure(&app, &[ran("cargo test", CommandStatus::Failed, Some(101))]),
    "exit 101"
  );
  assert_eq!(
    group_failure(
      &app,
      &[lint, build, ran("cargo test", CommandStatus::Failed, Some(101))]
    ),
    "cargo test · exit 101"
  );
}
