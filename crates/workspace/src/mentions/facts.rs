//! What the agent receives with a message that mentions tasks, runs or files (ORC-312, ORC-316): after the person's
//! words, a block between [`OPEN`] and [`CLOSE`] that says it is Orchester's, not the person's, with one entry per
//! mention — a task's key, title, workflow status and the first lines of its brief; a run's task key, number, state
//! and branch; a file's path relative to the repository, never its content, which the agent reads itself within its
//! permissions. The message is kept with the block, as the agent reads it; a conversation shows the person's words
//! only ([`said`]). The words are the agent's, in English, as every prompt Orchester writes.

use std::{fmt::Write, path::Path};

use orchester_core::{conversation::skill::without_skill, run::TaskPhase, task::organization::WorkflowStatus};
use orchester_store::{ProjectDb, StoreError};

use super::{
  Mention,
  catalog::{Catalog, RunChoice, TaskChoice},
  find,
};
use crate::messages::Message;

/// Where the block starts, on a line of its own after a blank line.
pub const OPEN: &str = "<orchester-mentions>";
/// Where the block ends: the message's last line.
pub const CLOSE: &str = "</orchester-mentions>";
/// How many lines of a task's brief the block quotes, and how long each may be.
const BRIEF_LINES: usize = 3;
const BRIEF_LINE_CHARS: usize = 240;

/// `text` with the block of what Orchester knows of each mention in it, as the project at `root` holds it now;
/// `text` as it is when it mentions nothing. Blocks on SQLite.
pub fn with_facts_in(root: &Path, text: &str) -> Result<String, Message> {
  if find(text).is_empty() {
    return Ok(text.to_owned());
  }
  Ok(with_facts(&Catalog::read_in(root)?, text))
}

/// [`with_facts_in`] with the project's database open: what every path that keeps or sends a person's message calls
/// (a task's conversation, a plan's changes, the Orchestrator chat), so a mention reaches the agent with the same
/// facts however it was sent. Blocks on SQLite.
pub fn with_facts_from(db: &ProjectDb, text: &str) -> Result<String, StoreError> {
  if find(text).is_empty() {
    return Ok(text.to_owned());
  }
  Ok(with_facts(&Catalog::read(db)?, text))
}

/// `text` with the block of what `catalog` knows of each mention in it, each mention once.
pub fn with_facts(catalog: &Catalog, text: &str) -> String {
  let mut mentions: Vec<Mention> = Vec::new();
  for found in find(text) {
    if !mentions.contains(&found.mention) {
      mentions.push(found.mention);
    }
  }
  if mentions.is_empty() {
    return text.to_owned();
  }
  let mut block = format!(
    "{}\n\n{OPEN}\nOrchester added this block: the person mentioned these with @ in the message above. They are \
     facts Orchester read when the message was sent, not the person's words.\n",
    text.trim_end()
  );
  for mention in &mentions {
    entry(&mut block, catalog, mention);
  }
  block.push_str(CLOSE);
  block
}

/// What the person wrote of `stored`, a message as it was kept: without the block of its mentions, nor that of the
/// skill they chose for it (`orchester_core::conversation::skill`), which its chip shows.
pub fn said(stored: &str) -> &str {
  let stored = without_skill(stored);
  let Some(body) = stored.trim_end().strip_suffix(CLOSE) else {
    return stored;
  };
  match body.rfind(&format!("\n\n{OPEN}\n")) {
    Some(at) => &stored[..at],
    None => stored,
  }
}

fn entry(block: &mut String, catalog: &Catalog, mention: &Mention) {
  let written = quoted(&mention.written());
  // Writing to a String does not fail.
  let _ = match mention {
    Mention::Task(key) => match catalog.task(key) {
      Some(task) => task_entry(block, &written, task),
      None => writeln!(block, "- {written}: no task {key} in this project."),
    },
    Mention::Run(key, number) => match catalog.run(key, *number) {
      Some(run) => run_entry(block, &written, run),
      None => writeln!(block, "- {written}: no run #{} of {key} in this project.", number.get()),
    },
    Mention::File(path) => writeln!(
      block,
      "- {written}: the file {}, its path relative to the root of the repository you work in. Its content is not \
       included: read it yourself if you need it.",
      quoted(path.as_str())
    ),
  };
}

fn task_entry(block: &mut String, written: &str, task: &TaskChoice) -> std::fmt::Result {
  writeln!(
    block,
    "- {written}: task {} \"{}\", workflow status {}.",
    task.key,
    quoted(&task.title),
    workflow_words(task.status)
  )?;
  let lines: Vec<&str> = task
    .brief
    .lines()
    .map(str::trim)
    .filter(|line| !line.is_empty())
    .collect();
  if lines.is_empty() {
    return writeln!(block, "  Brief: none.");
  }
  writeln!(block, "  Brief, first lines:")?;
  for line in lines.iter().take(BRIEF_LINES) {
    let cut: String = line.chars().take(BRIEF_LINE_CHARS).collect();
    let more = if cut.len() < line.len() { " …" } else { "" };
    writeln!(block, "  > {}{more}", quoted(&cut))?;
  }
  if lines.len() > BRIEF_LINES {
    writeln!(block, "  (the brief goes on)")?;
  }
  Ok(())
}

fn run_entry(block: &mut String, written: &str, run: &RunChoice) -> std::fmt::Result {
  writeln!(
    block,
    "- {written}: run #{} of task {} \"{}\", state {}, branch {}.",
    run.number.get(),
    run.key,
    quoted(&run.title),
    phase_words(run.phase),
    run.branch
  )
}

/// `text`, a title, a brief's line or a file's path someone wrote, quoted in the block so it can never pass for the
/// block's own start or end tag; only a tag's name followed by its `>` changes, so a path that merely holds the name
/// (`docs/orchester-mentions.md`) stays the file's.
fn quoted(text: &str) -> String {
  text.replace("orchester-mentions>", "orchester mentions>")
}

const fn workflow_words(status: WorkflowStatus) -> &'static str {
  match status {
    WorkflowStatus::Backlog => "Backlog",
    WorkflowStatus::Todo => "Todo",
    WorkflowStatus::InProgress => "In progress",
    WorkflowStatus::InReview => "In review",
    WorkflowStatus::Done => "Done",
    WorkflowStatus::Canceled => "Canceled",
  }
}

const fn phase_words(phase: TaskPhase) -> &'static str {
  match phase {
    TaskPhase::Draft => "draft",
    TaskPhase::Queued => "queued, not started",
    TaskPhase::Running => "running",
    TaskPhase::Waiting => "waiting for the person",
    TaskPhase::Stuck => "stopped, waiting for the person to decide how it goes on",
    TaskPhase::Paused => "paused",
    TaskPhase::StoppedAtLimit => "stopped at its limit",
    TaskPhase::Interrupted => "interrupted",
    TaskPhase::Succeeded => "completed, waiting for review",
    TaskPhase::Merged => "merged",
    TaskPhase::Failed => "failed",
    TaskPhase::Cancelled => "cancelled",
  }
}
