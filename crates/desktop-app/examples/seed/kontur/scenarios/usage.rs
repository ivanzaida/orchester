//! `usage`: what the Usage page (`et35n`) charts: runs of Kontur's tasks over the last two weeks, a few
//! today, on Feature delivery and Quick fix, with Explorer and Builder on Codex and Planner and Reviewer on
//! Claude Code, so tokens split by CLI, model, profile · step and pipeline; runs that succeeded, merged,
//! failed, paused and still run; and the plan limits the CLIs reported: Codex's weekly window resetting
//! midweek and its 5-hour windows, Claude's weekly window known only from four days ago (the hatched start).
//! `usage-empty` is the same project with no run today, which the page shows as *No runs in Kontur today*
//! on *Today*.

use orchester_core::{
  code_index::CodeIndexUse,
  connection::{
    AgentProvider,
    plan_limits::{LimitReading, LimitState, LimitWindow, PlanLimits},
  },
  id::RunId,
  resource::StepKey,
  run::{EditProgress, FirstEdit, Run, TokenUsage},
  store::{ActivityStore, RunStore},
  task::organization::{Priority, WorkflowStatus},
  time::Timestamp,
};

use crate::kontur::{
  Kontur, SeedError,
  catalogue::{Catalogue, Published},
  shapes::{self, Shape},
};

const HOUR: i64 = 60;
const DAY: i64 = 24 * HOUR;
/// A week's and a 5-hour window's minutes.
const WEEKLY: u64 = 10_080;
const FIVE_HOURS: u64 = 300;

#[derive(Clone, Copy)]
enum Pipe {
  Feature,
  QuickFix,
}

/// A task, its run's shape, and when the run started: `days` before today at `minute` of that day; today's
/// runs start `minute`s before now.
struct Row {
  title: &'static str,
  tags: &'static [&'static str],
  pipe: Pipe,
  shape: Shape,
  days: i64,
  minute: i64,
}

const fn row(
  title: &'static str,
  tags: &'static [&'static str],
  pipe: Pipe,
  shape: Shape,
  (days, minute): (i64, i64),
) -> Row {
  Row {
    title,
    tags,
    pipe,
    shape,
    days,
    minute,
  }
}

const ROWS: &[Row] = &[
  row(
    "Add retry to webhook sender",
    &["webhooks"],
    Pipe::Feature,
    Shape::Running("implement"),
    (0, 70),
  ),
  row(
    "Typed errors per module",
    &["errors"],
    Pipe::Feature,
    Shape::Running("review"),
    (0, 150),
  ),
  row(
    "Explain an expired GitHub token in errors",
    &["errors"],
    Pipe::Feature,
    Shape::Paused("implement"),
    (1, 15 * HOUR),
  ),
  row(
    "Export invoices as PDF",
    &["billing"],
    Pipe::Feature,
    Shape::Failed("review"),
    (2, 9 * HOUR),
  ),
  row(
    "Show the signed-in account",
    &["auth"],
    Pipe::QuickFix,
    Shape::Succeeded,
    (2, 14 * HOUR),
  ),
  row(
    "Rate-limit outgoing GitHub calls",
    &["webhooks"],
    Pipe::Feature,
    Shape::Merged,
    (3, 10 * HOUR),
  ),
  row(
    "Cache the pipeline catalogue",
    &[],
    Pipe::QuickFix,
    Shape::Merged,
    (3, 16 * HOUR),
  ),
  row(
    "Sync contacts from HubSpot",
    &["billing"],
    Pipe::Feature,
    Shape::Succeeded,
    (4, 11 * HOUR),
  ),
  row(
    "Retry failed invoice emails",
    &["billing"],
    Pipe::QuickFix,
    Shape::Failed("review"),
    (5, 13 * HOUR),
  ),
  row(
    "Link runs to their pull requests",
    &[],
    Pipe::Feature,
    Shape::Merged,
    (6, 9 * HOUR),
  ),
  row(
    "Trim long tool output",
    &["errors"],
    Pipe::QuickFix,
    Shape::Merged,
    (8, 10 * HOUR),
  ),
  row("Group tasks by tag", &[], Pipe::Feature, Shape::Merged, (9, 14 * HOUR)),
  row(
    "Fix the timezone of reset times",
    &["auth"],
    Pipe::QuickFix,
    Shape::Succeeded,
    (11, 15 * HOUR),
  ),
];

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  runs(kontur, ROWS.iter().collect())
}

/// The same project, but nothing ran today.
pub fn no_runs_today(kontur: &mut Kontur) -> Result<(), SeedError> {
  runs(kontur, ROWS.iter().filter(|row| row.days > 0).collect())
}

fn runs(kontur: &mut Kontur, rows: Vec<&Row>) -> Result<(), SeedError> {
  let catalogue = kontur.usage_catalogue()?;
  let mut sources = Vec::new();
  for row in rows {
    let pipeline = pipeline(&catalogue, row.pipe);
    let at = started(kontur, row);
    let created = Timestamp::from_unix_millis(at.unix_millis() - HOUR * 60_000);
    let mut task = kontur.task(
      (
        row.title,
        "What the task changes, as its brief says.",
        &["It works as asked."],
      ),
      Some(pipeline),
      created,
    )?;
    shapes::apply(kontur, &mut task, Some(pipeline), row.shape, at)?;
    kontur.organize(
      task.id(),
      (Priority::Medium, WorkflowStatus::InProgress),
      row.tags,
      None,
    )?;
    if let Some(Ok(run)) = kontur.db.runs_of_task(task.id())?.last() {
      first_edits(kontur, run, matches!(row.pipe, Pipe::Feature))?;
      sources.push((run.id(), at));
    }
  }
  plan_limits(kontur, &sources)
}

/// What the run's agents used before their first edit: Implement edits, with the code index on Feature
/// delivery (after reading a third of what it read) and without it on Quick fix (two thirds); the other
/// steps change no file.
fn first_edits(kontur: &mut Kontur, run: &Run, indexed: bool) -> Result<(), SeedError> {
  let implement = run
    .latest_execution(&StepKey::new("implement").expect("a valid step key"))
    .map(|execution| execution.id());
  for invocation in kontur.db.invocations(run.id())? {
    if invocation.ended_at.is_none() || invocation.usage == TokenUsage::default() {
      continue;
    }
    let used = if indexed {
      CodeIndexUse::Given {
        revision: kontur.base.clone(),
        ripgrep: None,
      }
    } else {
      CodeIndexUse::Off
    };
    kontur.db.save_invocation_code_index(invocation.id, &used)?;
    let edits = Some(invocation.execution) == implement;
    let thirds = if indexed { 1 } else { 2 };
    let part = |count: u64| count * thirds / 3;
    let before = TokenUsage {
      input: part(invocation.usage.input),
      cached_input: part(invocation.usage.cached_input),
      output: part(invocation.usage.output),
    };
    let progress = EditProgress {
      before,
      first_edit: edits.then(|| FirstEdit {
        at: invocation.started_at,
        path: "src/lib.rs".to_owned(),
      }),
      at_most: false,
    };
    kontur.db.save_edit_progress(invocation.id, &progress)?;
  }
  Ok(())
}

fn pipeline(catalogue: &Catalogue, pipe: Pipe) -> &Published {
  match pipe {
    Pipe::Feature => &catalogue.feature_delivery,
    Pipe::QuickFix => &catalogue.quick_fix,
  }
}

fn started(kontur: &Kontur, row: &Row) -> Timestamp {
  if row.days == 0 {
    return kontur.today_ago(row.minute);
  }
  let day = kontur.now.local_day_start().unix_millis() - row.days * DAY * 60_000;
  Timestamp::from_unix_millis(day + row.minute * 60_000)
}

/// What the CLIs reported while the runs worked: every half hour of the last week's working hours (08:00 to
/// 20:00) from the run nearest before, so each day's line is solid and the nights are dotted gaps; Codex's
/// weekly window reset three days ago at 09:00, and Claude was first heard of four days ago.
fn plan_limits(kontur: &mut Kontur, sources: &[(RunId, Timestamp)]) -> Result<(), SeedError> {
  let reset = kontur.now.local_day_start().unix_millis() - 3 * DAY * 60_000 + 9 * HOUR * 60_000;
  let next_reset = Timestamp::from_unix_millis(reset + 7 * DAY * 60_000);
  let claude_reset = kontur.ago(-2 * DAY - 3 * HOUR);
  let mut minutes = 7 * DAY;
  while minutes > STEP {
    let at = kontur.ago(minutes);
    minutes -= STEP;
    let hour = (at.unix_millis() - at.local_day_start().unix_millis()) / (HOUR * 60_000);
    let Some(run) = nearest(sources, at).filter(|_| (8..20).contains(&hour)) else {
      continue;
    };
    let since_reset = (at.unix_millis() - reset) as f64 / (DAY * 60_000) as f64;
    let (weekly, codex_resets) = if since_reset < 0.0 {
      (55.0 + (7.0 + since_reset) * 4.0, Timestamp::from_unix_millis(reset))
    } else {
      (5.0 + since_reset * 18.0, next_reset)
    };
    // Where the half hour falls in its 5-hour window, which fills up and then resets.
    let slot = (minutes / STEP) % 10;
    let five_hour_resets = Timestamp::from_unix_millis(at.unix_millis() + (10 - slot) * STEP * 60_000);
    let codex = vec![
      window(WEEKLY, weekly.min(95.0), codex_resets),
      window(FIVE_HOURS, slot as f64 * 9.0 + 5.0, five_hour_resets),
    ];
    kontur
      .db
      .save_limit_snapshot(run, &reading(AgentProvider::CodexCli, at, codex))?;
    if minutes <= 4 * DAY {
      let claude_used = 20.0 + (4 * DAY - minutes) as f64 / DAY as f64 * 5.0;
      let claude = vec![window(WEEKLY, claude_used, claude_reset)];
      kontur
        .db
        .save_limit_snapshot(run, &reading(AgentProvider::ClaudeCodeCli, at, claude))?;
    }
  }
  Ok(())
}

/// Between two reports of a CLI's limits.
const STEP: i64 = 30;

/// The run that started last before `at`.
fn nearest(sources: &[(RunId, Timestamp)], at: Timestamp) -> Option<RunId> {
  sources
    .iter()
    .filter(|(_, started)| *started <= at)
    .max_by_key(|(_, started)| *started)
    .map(|(run, _)| *run)
}

fn window(minutes: u64, percent: f64, resets: Timestamp) -> LimitWindow {
  LimitWindow::new(minutes, Some(percent), Some(resets), LimitState::Normal, false)
    .expect("the scenario's windows are valid")
}

fn reading(provider: AgentProvider, at: Timestamp, windows: Vec<LimitWindow>) -> LimitReading {
  LimitReading {
    provider,
    account: None,
    at,
    limits: PlanLimits {
      windows,
      plan: None,
      credits: None,
      blocked: None,
    },
  }
}
