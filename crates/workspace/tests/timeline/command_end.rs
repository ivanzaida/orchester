//! How a finished command reads in its row and in its tool group's summary: its exit code when the CLI reported one;
//! otherwise an agent's command reads *failed*, or only how long it took once it completed (Claude Code reports no
//! exit codes), and a tool step's *no exit code*. Never an empty exit code or duration.

use lurq::app::App;
use orchester_core::{
  connection::stream::{AgentEvent, CommandStatus, ToolCommandResult},
  id::StepExecutionId,
};
use orchester_desktop::{
  i18n,
  messages::Message,
  tasks::{
    timeline::ToolGroup,
    transcript::{CommandEntry, Entry, EntryKind, ToolCommandEntry},
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
fn row(app: &App, events: &[AgentEvent; 2]) -> String {
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
    row(&app, &ran("cargo test", CommandStatus::Failed, None)),
    "failed · 1.0 s"
  );
  assert_eq!(row(&app, &ran("cargo test", CommandStatus::Completed, None)), "1.0 s");
  assert_eq!(
    row(&app, &ran("cargo test", CommandStatus::Failed, Some(101))),
    "exit 101 · 1.0 s"
  );
  assert_eq!(
    row(&app, &ran("cargo test", CommandStatus::Completed, Some(0))),
    "exit 0 · 1.0 s"
  );
}

#[test]
fn timeline_command_end_in_a_group_summary_names_a_failure_without_an_exit_code() {
  let app = english();
  let failed = ran("cargo test", CommandStatus::Failed, None);
  assert_eq!(group_failure(&app, std::slice::from_ref(&failed)), "failed");
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

/// An agent's command without a start seen has no duration: a failed one reads *failed*, a completed one nothing.
#[test]
fn timeline_command_end_without_a_duration_shows_no_empty_one() {
  let app = english();
  let ended = |status, exit_code| CommandEntry {
    item: "c".into(),
    command: "cargo test".into(),
    output: "".into(),
    status: Some(status),
    exit_code,
    duration: None,
    mark: None,
    native: None,
  };
  let failed = ended(CommandStatus::Failed, None).ended().expect("it failed");
  assert_eq!(text(&app, &failed), "failed");
  assert_eq!(ended(CommandStatus::Completed, None).ended(), None);
  let exit = ended(CommandStatus::Completed, Some(0))
    .ended()
    .expect("it has an exit code");
  assert_eq!(text(&app, &exit), "exit 0");
}

/// A tool step's command reads its exit code, *no exit code*, *timed out* or *stopped*, with how long it took only
/// when that is known.
#[test]
fn timeline_command_end_of_a_tool_step_shows_no_empty_duration() {
  let app = english();
  let ended = |result, exit_code, duration| {
    let entry = ToolCommandEntry {
      command: "pnpm test".into(),
      result: Some(result),
      exit_code,
      output: "".into(),
      omitted_bytes: 0,
      duration,
    };
    text(&app, &entry.ended().expect("it ended"))
  };
  assert_eq!(ended(ToolCommandResult::Failed, Some(3), Some(4_213)), "exit 3 · 4.2 s");
  assert_eq!(ended(ToolCommandResult::Failed, Some(3), None), "exit 3");
  assert_eq!(
    ended(ToolCommandResult::Passed, None, Some(4_213)),
    "no exit code · 4.2 s"
  );
  assert_eq!(ended(ToolCommandResult::Passed, None, None), "no exit code");
  assert_eq!(
    ended(ToolCommandResult::TimedOut, None, Some(30_000)),
    "timed out · 30.0 s"
  );
  assert_eq!(ended(ToolCommandResult::TimedOut, None, None), "timed out");
  assert_eq!(ended(ToolCommandResult::Stopped, None, None), "stopped");
}
