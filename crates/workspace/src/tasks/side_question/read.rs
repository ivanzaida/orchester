//! What the side agent reads of a run, as text: its transcript, entry by entry with the person's messages to the run
//! between them in time order, and its changes as a unified diff, the checkpointed ones and what its worktree holds
//! beyond them. Every text here was kept redacted; the runner masks the project's secret values in it once more.
//! Blocks on SQLite and git.

use std::path::Path;

use orchester_core::{
  connection::stream::{CommandStatus, FileChangeKind},
  conversation::Sender,
  diff::{Diff, LineKind},
  id::{RunId, TaskId},
  run::side_question::{Exchange, SideAbout, SideInput, SideQuestionText, SideState},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;

use crate::{
  display::clock,
  messages::{self, Message},
  tasks::{
    changes::{Checkpointed, NotCheckpointed, RunRange, RunStage, not_checkpointed_in, run_changes_in},
    detail::{RunView, TaskDetail},
    timeline::load::transcript_in,
    transcript::{Entry, EntryKind},
  },
};

/// The most characters of a command's output one entry keeps: its end, where an error is.
const OUTPUT_TAIL_CHARS: usize = 1_500;

/// What the side agent reads to answer `question` about `run` of `task` in the project at `root`: the run's
/// transcript and changes, and the questions asked about it before with their answers.
pub fn side_input_in(
  root: &Path,
  programs: &Programs,
  (task, run): (TaskId, RunId),
  question: &SideQuestionText,
) -> Result<SideInput, Message> {
  let detail = TaskDetail::load_in(root, task).map_err(|error| messages::store_error(&error))?;
  let view = detail
    .run
    .as_ref()
    .filter(|view| view.id() == run)
    .ok_or_else(|| Message::new("sideQuestion.errors.notLatestRun"))?;
  let entries = transcript_in(root, run).map_err(|error| messages::store_error(&error))?;
  let mut lines = transcript_lines(view, &entries);
  lines.extend(person_lines(&detail, run));
  lines.sort_by_key(|(at, _)| *at);
  let lines: Vec<String> = lines.into_iter().map(|(_, line)| line).collect();
  let diff = run_diff(root, programs, view);
  let earlier = earlier_in(root, (task, run))?;
  let about = SideAbout {
    task_key: detail.task.key().to_string(),
    title: detail.task.details().title.as_str().to_owned(),
    run: view.run.number().get(),
    agent: view
      .current_step()
      .and_then(|step| step.agent.as_deref().map(str::to_owned)),
  };
  Ok(SideInput::new(about, question, (&lines, &diff), &earlier))
}

/// Whether `run` has anything for the side agent to read yet: a transcript entry.
pub fn has_transcript_in(root: &Path, run: RunId) -> Result<bool, Message> {
  let entries = transcript_in(root, run).map_err(|error| messages::store_error(&error))?;
  Ok(!entries.is_empty())
}

/// The questions about `run` answered before, with their answers.
fn earlier_in(root: &Path, (task, run): (TaskId, RunId)) -> Result<Vec<Exchange>, Message> {
  let db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
  let asked = db.side_questions(task).map_err(|error| messages::store_error(&error))?;
  Ok(
    asked
      .into_iter()
      .filter(|asked| asked.run == run)
      .filter_map(|asked| match asked.state {
        SideState::Answered { reply, .. } => Some(Exchange {
          question: asked.question.as_str().to_owned(),
          answer: reply.text,
        }),
        SideState::Answering | SideState::Failed { .. } => None,
      })
      .collect(),
  )
}

/// `entries` of `view`'s transcript as lines, each with when it happened.
pub fn transcript_lines(view: &RunView, entries: &[Entry]) -> Vec<(Timestamp, String)> {
  entries
    .iter()
    .filter_map(|entry| {
      let author = view
        .step_of(entry.execution)
        .and_then(|step| step.agent.as_deref().map(str::to_owned).or(Some(step.name.to_string())))
        .unwrap_or_else(|| "agent".to_owned());
      let said = entry_text(&entry.kind)?;
      Some((entry.at, format!("[{}] {author}: {said}", clock(entry.at))))
    })
    .collect()
}

/// What one entry says, as a line of text; none for what says nothing about the work (usage, raw output).
fn entry_text(kind: &EntryKind) -> Option<String> {
  Some(match kind {
    EntryKind::Message(text) => format!("message: {text}"),
    EntryKind::Reasoning(text) => format!("reasoning: {text}"),
    EntryKind::Command(command) => {
      let status = match command.status {
        None => "running",
        Some(CommandStatus::Completed) => "completed",
        Some(CommandStatus::Failed) => "failed",
        Some(CommandStatus::Declined) => "declined",
      };
      let exit = command
        .exit_code
        .map(|code| format!(", exit code {code}"))
        .unwrap_or_default();
      format!(
        "ran `{}` ({status}{exit}); output ends: {}",
        command.command,
        tail(&command.output)
      )
    }
    EntryKind::ToolCommand(command) => {
      format!("tool ran `{}`; output ends: {}", command.command, tail(&command.output))
    }
    EntryKind::FileChange(change) => {
      let how = match change.kind {
        FileChangeKind::Added => "added",
        FileChangeKind::Modified => "changed",
        FileChangeKind::Deleted => "deleted",
      };
      format!("{how} file {}", change.path)
    }
    EntryKind::McpTool(call) => format!("called tool {} of {}", call.tool, call.server),
    EntryKind::Error(text) | EntryKind::NotSignedIn(text) => format!("error: {text}"),
    EntryKind::TurnStopped => "the person stopped its turn".to_owned(),
    EntryKind::Usage(_)
    | EntryKind::Raw(_)
    | EntryKind::Problem(_)
    | EntryKind::McpServer(..)
    | EntryKind::ExemptionNotApplied { .. }
    | EntryKind::Approval(_) => return None,
  })
}

/// The person's messages to `run` as lines, each with when it was written.
fn person_lines(detail: &TaskDetail, run: RunId) -> Vec<(Timestamp, String)> {
  detail
    .messages
    .iter()
    .filter(|message| message.run() == Some(run) && matches!(message.sender(), Sender::Person { .. }))
    .map(|message| {
      let at = message.created_at();
      (
        at,
        format!("[{}] person: message: {}", clock(at), message.body().as_str()),
      )
    })
    .collect()
}

/// The end of `output`, at most [`OUTPUT_TAIL_CHARS`].
fn tail(output: &str) -> String {
  let count = output.chars().count();
  output.chars().skip(count.saturating_sub(OUTPUT_TAIL_CHARS)).collect()
}

/// `view`'s changes as a unified diff: from where its work starts to its latest checkpoint, then what its worktree
/// holds beyond that. What cannot be read is said in the text instead.
fn run_diff(root: &Path, programs: &Programs, view: &RunView) -> String {
  let mut text = String::new();
  match run_changes_in(root, programs, &RunRange::of(&view.run)) {
    Ok(Checkpointed::Read { changes, .. }) => text.push_str(&unified_text(&changes.diff)),
    Ok(Checkpointed::NoCheckpoint) => {}
    Ok(Checkpointed::TooLarge { size, .. }) => line(
      &mut text,
      &format!("(the checkpointed changes are too large to read: {} files)", size.files),
    ),
    Err(error) => {
      tracing::warn!(%error, run = %view.id(), "cannot read the run's changes for a side question");
      line(&mut text, "(the checkpointed changes could not be read)");
    }
  }
  match not_checkpointed_in(root, programs, (view.id(), RunStage::of(view.run.status()))) {
    Ok(NotCheckpointed::Read(changes)) => {
      line(&mut text, "(not checkpointed yet, in the worktree:)");
      text.push_str(&unified_text(&changes.diff));
    }
    Ok(NotCheckpointed::Clean | NotCheckpointed::Removed) => {}
    Ok(NotCheckpointed::TooLarge { size, .. }) => line(
      &mut text,
      &format!("(the worktree's changes are too large to read: {} files)", size.files),
    ),
    Err(error) => {
      tracing::warn!(%error, run = %view.id(), "cannot read the run's worktree for a side question");
      line(&mut text, "(the worktree's changes could not be read)");
    }
  }
  text
}

/// `diff` written out as a unified diff again.
pub fn unified_text(diff: &Diff) -> String {
  let mut text = String::new();
  for file in diff.files() {
    line(&mut text, &format!("--- {}", file.old_path().unwrap_or("/dev/null")));
    line(&mut text, &format!("+++ {}", file.new_path().unwrap_or("/dev/null")));
    if file.is_binary() {
      line(&mut text, "(binary file)");
    }
    for hunk in file.hunks() {
      let range = format!(
        "@@ -{},{} +{},{} @@ {}",
        hunk.old_start(),
        hunk.old_count(),
        hunk.new_start(),
        hunk.new_count(),
        hunk.section()
      );
      line(&mut text, &range);
      for changed in hunk.lines() {
        let mark = match changed.kind() {
          LineKind::Context => ' ',
          LineKind::Added => '+',
          LineKind::Removed => '-',
        };
        text.push(mark);
        line(&mut text, changed.text());
      }
    }
    if file.omitted_lines() > 0 {
      line(
        &mut text,
        &format!("({} more lines of this file left out)", file.omitted_lines()),
      );
    }
  }
  text
}

/// Adds `said` and a line break to `text`.
fn line(text: &mut String, said: &str) {
  text.push_str(said);
  text.push('\n');
}
