//! `tasks`: the design's Kontur project (`Tasks · full page · dark` `C2E5Po`): 24 tasks across the
//! workflow tabs (Backlog 3, Todo 8, In progress 8, In review 2, Done 2, one Canceled) and run phases, four
//! of them waiting for a person. The rows the design shows keep their titles, tags, pipelines and states,
//! and where their key fits in 24 tasks their key too (KON-9, 11, 12, 16, 21, 23, 24). KON-12, Add retry to
//! webhook sender, is the task the design's task pages show: its third run works on Implement again after
//! Review returned the work, waits for a command approval, and has a checkpoint and uncommitted work.

use orchester_core::{
  id::IdempotencyKey,
  run::{RunLimit, StepFailure},
  store::TaskStore,
  task::{
    Task,
    organization::{Priority, WorkflowStatus},
  },
  task_group::{GroupHold, ReviewedPipeline, Scheduling, TaskGroup},
  tokens::TokenBudget,
};

use crate::kontur::{
  Kontur, SeedError,
  catalogue::{Catalogue, Published},
  files,
  run::Drive,
  shapes::{self, Shape},
};

#[derive(Clone, Copy)]
enum Pipe {
  Feature,
  QuickFix,
  None,
}

/// One task of the list, in key order.
struct Row {
  title: &'static str,
  brief: &'static str,
  tags: &'static [&'static str],
  pipe: Pipe,
  priority: Priority,
  status: WorkflowStatus,
  shape: Shape,
}

const fn row(
  (title, brief): (&'static str, &'static str),
  tags: &'static [&'static str],
  pipe: Pipe,
  (priority, status): (Priority, WorkflowStatus),
  shape: Shape,
) -> Row {
  Row {
    title,
    brief,
    tags,
    pipe,
    priority,
    status,
    shape,
  }
}

use Priority::{High, Low, Medium, None as NoPriority};
use WorkflowStatus::{Backlog, Canceled, Done, InProgress, InReview, Todo};

/// KON-12 is [`retry_webhooks`]; its row's shape is not used.
const KON_12: usize = 11;
/// KON-11 and KON-17 were queued together; KON-11 failed, so the group holds KON-17.
const GROUPED: [usize; 2] = [10, 16];
/// KON-19 waits for KON-18 to be merged.
const BLOCKED: (usize, usize) = (18, 17);

const ROWS: [Row; 24] = [
  row(
    (
      "Set up CI caching",
      "Cache cargo's registry and target folder between CI runs.",
    ),
    &["ci"],
    Pipe::Feature,
    (Medium, Done),
    Shape::Merged,
  ),
  row(
    (
      "Webhook endpoint settings page",
      "Let each endpoint set its URL, secret and timeout in the app.",
    ),
    &["webhooks", "ui"],
    Pipe::Feature,
    (Medium, Done),
    Shape::Merged,
  ),
  row(
    ("Audit log retention", ""),
    &["backend"],
    Pipe::QuickFix,
    (Low, Backlog),
    Shape::Draft,
  ),
  row(
    ("Billing alerts by email", ""),
    &["billing"],
    Pipe::None,
    (NoPriority, Backlog),
    Shape::Draft,
  ),
  row(
    (
      "Rotate API keys from settings",
      "Let an admin rotate the API key without downtime: both keys work for an hour.",
    ),
    &["auth"],
    Pipe::Feature,
    (High, Todo),
    Shape::ReadyDraft,
  ),
  row(
    (
      "Paginate the events API",
      "Page the events endpoint with a cursor; keep the first page as it is.",
    ),
    &["backend"],
    Pipe::QuickFix,
    (Medium, Todo),
    Shape::ReadyDraft,
  ),
  row(
    (
      "Retry Slack notifications",
      "Retry Slack notifications the way webhooks are retried.",
    ),
    &["webhooks"],
    Pipe::QuickFix,
    (Low, Canceled),
    Shape::ReadyDraft,
  ),
  row(
    (
      "Localize the invoice emails",
      "Send invoice emails in the customer's language, German and French first.",
    ),
    &["exports"],
    Pipe::Feature,
    (Medium, Todo),
    Shape::Queued,
  ),
  row(
    (
      "Show the signed-in account",
      "Show who is signed in, with their avatar, at the top of the sidebar.",
    ),
    &["ui"],
    Pipe::QuickFix,
    (Low, InReview),
    Shape::Succeeded,
  ),
  row(
    (
      "Plan the GitHub app permissions",
      "Work out the smallest set of GitHub app permissions the sync needs.",
    ),
    &["github", "auth"],
    Pipe::Feature,
    (High, InProgress),
    Shape::PlanReview,
  ),
  row(
    (
      "Export invoices as PDF",
      "Export a month of invoices as one PDF per customer.",
    ),
    &["exports"],
    Pipe::Feature,
    (Medium, InProgress),
    Shape::Failed("run-checks"),
  ),
  row(
    ("Add retry to webhook sender", RETRY_BRIEF),
    &["webhooks", "backend"],
    Pipe::Feature,
    (High, InProgress),
    Shape::Running("implement"),
  ),
  row(
    (
      "Sync contacts with HubSpot",
      "Sync contacts both ways with HubSpot every fifteen minutes.",
    ),
    &["import"],
    Pipe::Feature,
    (Medium, InProgress),
    Shape::Question("implement"),
  ),
  row(
    (
      "Cache GitHub API responses",
      "Cache GitHub API responses with their ETags and revalidate them.",
    ),
    &["github"],
    Pipe::QuickFix,
    (Medium, InReview),
    Shape::Stuck("review"),
  ),
  row(
    (
      "Drop the legacy sync worker",
      "Remove the old sync worker now that the queue runs every sync.",
    ),
    &["backend"],
    Pipe::QuickFix,
    (Low, Todo),
    Shape::ReadyDraft,
  ),
  row(
    (
      "Rate-limit messages from GitHub",
      "Respect GitHub's rate limits when the sync sends messages.",
    ),
    &["github"],
    Pipe::QuickFix,
    (Medium, InProgress),
    Shape::Running("plan"),
  ),
  row(
    (
      "Retry failed exports nightly",
      "Retry exports that failed during the day once at night.",
    ),
    &["exports"],
    Pipe::Feature,
    (Medium, Todo),
    Shape::Queued,
  ),
  row(
    (
      "Webhook signing secrets per endpoint",
      "Give each endpoint its own signing secret, rotated from settings.",
    ),
    &["webhooks", "auth"],
    Pipe::Feature,
    (High, Todo),
    Shape::Queued,
  ),
  row(
    (
      "Verify webhook signatures on receive",
      "Reject incoming webhooks whose signature does not match.",
    ),
    &["webhooks"],
    Pipe::Feature,
    (Medium, Todo),
    Shape::ReadyDraft,
  ),
  row(
    (
      "Dark mode for the settings page",
      "Make the settings page follow the dark theme like the rest of the app.",
    ),
    &["ui"],
    Pipe::QuickFix,
    (Low, Todo),
    Shape::ReadyDraft,
  ),
  row(
    (
      "Typed errors per module",
      "Give every module its own error enum instead of strings.",
    ),
    &[],
    Pipe::Feature,
    (Medium, InProgress),
    Shape::Running("review"),
  ),
  row(
    ("Docs: connecting GitHub", ""),
    &["docs"],
    Pipe::None,
    (NoPriority, Backlog),
    Shape::Draft,
  ),
  row(
    (
      "Explain an expired GitHub token in the status bar",
      "Say in the status bar when the GitHub token expired and how to renew it.",
    ),
    &["github", "auth", "ui", "docs"],
    Pipe::Feature,
    (Medium, InProgress),
    Shape::AtLimit("implement", RunLimit::Tokens),
  ),
  row(
    (
      "Import contacts from CSV",
      "Import contacts from a CSV file with a preview of the first rows.",
    ),
    &["import"],
    Pipe::QuickFix,
    (Low, InProgress),
    Shape::Unreadable("plan"),
  ),
];

const RETRY_BRIEF: &str = "Webhook deliveries fail for good after a single network error. Retry failed deliveries \
                           with exponential backoff and jitter, keep every attempt visible in the endpoint's \
                           delivery log, and let each endpoint set how many attempts it gets.";
const RETRY_CRITERIA: [&str; 5] = [
  "Failed deliveries retry with exponential backoff and full jitter.",
  "429 and 503 responses honor Retry-After, capped at 60 s.",
  "Each endpoint has a retry budget; the default is 5 attempts.",
  "Every attempt is recorded with its status code and delay.",
  "Tests cover the backoff schedule and the jitter bound.",
];
const CRITERIA: [&str; 2] = ["The change is covered by tests.", "Nothing changes for existing users."];

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  project(kontur).map(drop)
}

/// Seeds the project; the catalogue its tasks use, for the scenarios that add to it.
pub fn project(kontur: &mut Kontur) -> Result<Catalogue, SeedError> {
  let catalogue = kontur.catalogue()?;
  let mut tasks = Vec::with_capacity(ROWS.len());
  for (index, row) in ROWS.iter().enumerate() {
    let created = kontur.ago(minutes(24 - index) * 10);
    let criteria: &[&str] = match (row.brief.is_empty(), index == KON_12) {
      (true, _) => &[],
      (false, true) => &RETRY_CRITERIA,
      (false, false) => &CRITERIA,
    };
    tasks.push(kontur.task(
      (row.title, row.brief, criteria),
      pipeline(&catalogue, row.pipe),
      created,
    )?);
  }
  let group = queue_group(kontur, &tasks, &catalogue)?;
  for (index, row) in ROWS.iter().enumerate() {
    let mut task = kontur.db.task(tasks[index].id())?.ok_or(SeedError::TaskChanged)?;
    let started = kontur.ago(minutes(24 - index) * 4);
    if index == KON_12 {
      retry_webhooks(kontur, &mut task, &catalogue.feature_delivery)?;
    } else {
      shapes::apply(kontur, &mut task, pipeline(&catalogue, row.pipe), row.shape, started)?;
    }
  }
  // After the runs, which move a task's workflow status as they start and end, as people do.
  for (index, row) in ROWS.iter().enumerate() {
    let blocked_by = (index == BLOCKED.0).then(|| tasks[BLOCKED.1].id());
    kontur.organize(tasks[index].id(), (row.priority, row.status), row.tags, blocked_by)?;
  }
  kontur.db.set_task_group_hold(group, Some(&GroupHold::FailedTask))?;
  Ok(catalogue)
}

fn minutes(steps: usize) -> i64 {
  i64::try_from(steps).expect("a small number of steps") * 30
}

fn pipeline(catalogue: &Catalogue, pipe: Pipe) -> Option<&Published> {
  match pipe {
    Pipe::Feature => Some(&catalogue.feature_delivery),
    Pipe::QuickFix => Some(&catalogue.quick_fix),
    Pipe::None => None,
  }
}

/// Queues KON-11 and KON-17 as one reviewed group, one after the other; its key.
fn queue_group(kontur: &mut Kontur, tasks: &[Task], catalogue: &Catalogue) -> Result<IdempotencyKey, SeedError> {
  let pinned = catalogue.feature_delivery.snapshot().pipeline();
  let members: Vec<_> = GROUPED.iter().map(|index| tasks[*index].id()).collect();
  let reviewed: Vec<_> = members
    .iter()
    .map(|task| ReviewedPipeline {
      task: *task,
      pipeline: pinned.resource,
      version: pinned.number,
    })
    .collect();
  let cap = TokenBudget::new(2_000_000).expect("above zero");
  let group = TaskGroup::new(IdempotencyKey::generate(), members, Scheduling::Sequential, cap)
    .map_err(|error| SeedError::Invalid(error.to_string()))?;
  Ok(kontur.db.queue_task_group(&group, None, &[reviewed].concat())?.key())
}

/// KON-12's three runs, as the design's Summary lists them: the first failed at Run checks (its worktree
/// removed since), the second was cancelled at Implement with two files not committed, and the third works
/// on Implement again after Review returned the work, waiting for a person to approve a command.
fn retry_webhooks(kontur: &mut Kontur, task: &mut Task, feature: &Published) -> Result<(), SeedError> {
  kontur.queue(task, feature)?;
  let (four_days, two_days, earlier_today) = (kontur.ago(4 * 24 * 60), kontur.ago(2 * 24 * 60), kontur.ago(60));
  Drive::start(kontur, task, feature, four_days)?
    .fail_until_stuck("run-checks")?
    .stop()?
    .remove_worktree()?;
  Drive::start(kontur, task, feature, two_days)?
    .pass_before("implement")?
    .begin("implement")?
    .uncommitted(&files::retry_in_progress())?
    .cancel()?;
  Drive::start(kontur, task, feature, earlier_today)?
    .pass_before("implement")?
    .pass_with("implement", &files::retry_checkpoint())?
    .pass("run-checks")?
    .fail("review", StepFailure::ReportedFailed)?
    .begin("implement")?
    .approval("implement", "cargo add rand", "Builder")?
    .uncommitted(&files::retry_in_progress())?;
  kontur.seeded(task, "running, approval waiting");
  Ok(())
}
