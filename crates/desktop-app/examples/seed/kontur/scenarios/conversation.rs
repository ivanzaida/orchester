//! `conversation`: the `tasks` project with a long transcript in KON-12's run (`Task · Conversation · dark`
//! `kazPZ`): its Builder, back on Implement after Review returned the work, reads files, edits, runs tests
//! that fail, loses its stream once and goes on reading for a long while — more than the timeline shows at
//! first — while a person's message reached it mid-turn. The command it asked to run in `tasks` was approved
//! by the person, so nothing waits for them here. It still works: its last command has not ended. Near the
//! end it writes a message with links (`Orchestrator · reply with links` `w262B`): a labelled link list,
//! in-app links, a link into Atlas — a second project registered beside Kontur — and addresses it does not
//! follow.

use std::fs;

use orchester_core::{
  connection::stream::{AgentEvent, CommandStatus, FileChange, FileChangeKind},
  conversation::{Arrival, Message, MessageBody, Recipient, Timing},
  id::{IdempotencyKey, ProjectId},
  name::Name,
  project::{Project, ProjectKey},
  resource::StepKey,
  run::{DecidedBy, Invocation, Run, StepExecutor, ToolApprovalDecision},
  store::{ActivityStore, ConversationStore, RunStore},
  time::Timestamp,
};
use orchester_store::AppDb;

use super::tasks;
use crate::kontur::{Kontur, SeedError};

/// How far apart the transcript's events are.
const EVENT_MILLIS: i64 = 20_000;
/// Files the Builder reads in the long middle of its turn.
const READ: [&str; 12] = [
  "src/webhooks/log.rs",
  "src/webhooks/mod.rs",
  "src/lib.rs",
  "tests/webhooks/retry.rs",
  "migrations/0041_endpoints.sql",
  "Cargo.toml",
  "README.md",
  "src/webhooks/settings.rs",
  "src/webhooks/sender.rs",
  "migrations/0042_delivery_attempts.sql",
  "tests/webhooks/jitter.rs",
  "src/webhooks/log.rs",
];
const ASKED: &str = "Please keep the retry budget configurable per endpoint, default 5 attempts.";

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  tasks::project(kontur)?;
  let task = kontur.find("KON-12")?;
  let run = kontur
    .db
    .runs_of_task(task.id())?
    .into_iter()
    .filter_map(Result::ok)
    .next_back()
    .ok_or(SeedError::NotAtWork)?;
  let invocation = kontur
    .db
    .invocations(run.id())?
    .into_iter()
    .rev()
    .find(|invocation| invocation.ended_at.is_none())
    .ok_or(SeedError::NotAtWork)?;
  // The person approved the command the tasks scenario left waiting; the Builder ran it and went on.
  let approval = kontur
    .db
    .pending_tool_approvals(run.id())?
    .into_iter()
    .next()
    .ok_or(SeedError::NotAtWork)?;
  let decided = Timestamp::from_unix_millis(approval.requested_at.unix_millis() + EVENT_MILLIS);
  kontur
    .db
    .claim_tool_approval(run.id(), &approval.id, DecidedBy::Person, decided)?;
  kontur
    .db
    .delivered_tool_approval(run.id(), &approval.id, ToolApprovalDecision::Approved)?;
  let atlas = register_atlas(kontur)?;
  let links = links(
    (kontur.db.project().id(), atlas),
    (&task.key().to_string(), task.details().title.as_str()),
  );
  let events = transcript(decided, &approval.action, &links);
  kontur.db.append_activity(&invocation, &events)?;
  let asked_at = events[10].0;
  let body = MessageBody::new(ASKED).map_err(|error| SeedError::Invalid(error.to_string()))?;
  let message = Message::from_person(
    task.id(),
    Some(run.id()),
    Recipient::Agent(builder(&run)?),
    body,
    asked_at,
  )
  .sent(Timing::Now)
  .map_err(|error| SeedError::Invalid(error.to_string()))?;
  let mut message = kontur.db.send_message(IdempotencyKey::generate(), &message)?;
  deliver(&mut message, &run, &invocation, events[11].0)?;
  kontur.db.save_deliveries(&[message])?;
  Ok(())
}

/// The Builder's profile, which Implement runs.
fn builder(run: &Run) -> Result<orchester_core::id::ResourceId, SeedError> {
  let implement = StepKey::new("implement").expect("a valid step key");
  match run.snapshot().step(&implement).map(|step| step.executor()) {
    Some(StepExecutor::Agent { profile, .. }) => Ok(profile.resource),
    _ => Err(SeedError::NoSuchStep("implement".to_owned())),
  }
}

fn deliver(message: &mut Message, run: &Run, invocation: &Invocation, at: Timestamp) -> Result<(), SeedError> {
  message
    .deliver(run.id(), (invocation.execution, invocation.id), at, Arrival::MidTurn)
    .map_err(|error| SeedError::Invalid(error.to_string()))
}

/// Atlas, a second project registered in the app's database beside Kontur (its folder is empty: only its name
/// is read), which the Builder's links point into.
fn register_atlas(kontur: &Kontur) -> Result<ProjectId, SeedError> {
  let atlas = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ATL").expect("a valid project key"),
    Name::new("Atlas").map_err(|error| SeedError::Invalid(error.to_string()))?,
  );
  let folder = kontur.app_database.with_file_name("repo-atlas");
  fs::create_dir_all(&folder).map_err(|source| SeedError::Io {
    path: folder.clone(),
    source,
  })?;
  let mut db = AppDb::open(&kontur.app_database)?;
  // The signed-in account's project, or the next one's to sign in.
  let owner = db.cloud_account()?.map(|account| account.account);
  // Opened an hour before Kontur, which stays the project the app opens.
  db.register_project(&atlas, &folder, kontur.ago(60), owner.as_ref())?;
  Ok(atlas.id())
}

/// The Builder's message with links (as in `Orchestrator · reply with links` `w262B`): a labelled list of links
/// to `task` (`(key, title)`) of `project`, an in-app link, a link into `atlas`, a pull request on the web, and
/// addresses the chat does not follow.
fn links((project, atlas): (ProjectId, ProjectId), (key, title): (&str, &str)) -> String {
  let task = format!("orchester:/project/{project}/tasks/{key}/summary");
  format!(
    "The retry budget is in; here is where {key} stands.\n\n\
     - Task: [{key} · {title}]({task}) — returned by Review, back on Implement\n\
     - Run: [Run #2 · Feature delivery](orchester:/project/{project}/tasks/{key}/conversation) — 03 Implement · \
     2 of 5 steps\n\
     - Usage: [Usage · 1–15 Oct · {key} · by step](orchester:/project/{project}/usage?from=2026-10-01&to=2026-10-15&group=step) \
     — 1.2M tokens over 2 runs\n\
     - Pull request: [#412 Local integration](https://github.com/kontur/kontur/pull/412) — 2 of 5 checks running\n\n\
     Review's notes are in [{key} › Conversation](orchester:/project/{project}/tasks/{key}/conversation); Atlas \
     reads the same log in [ATL-9 · Import schema](orchester:/project/{atlas}/tasks/ATL-9/summary). \
     [See the runbook](file:///H:/projects/kontur/docs/runbook.md) or the \
     [Status page](http://status.kontur.local)."
  )
}

/// The turn's events after the person approved `approved` at `start`, [`EVENT_MILLIS`] apart; `links` is the
/// Builder's message with links, near the end: before its last tool group, which it would split.
fn transcript(start: Timestamp, approved: &str, links: &str) -> Vec<(Timestamp, AgentEvent)> {
  let mut events = Vec::new();
  events.extend(command("approved-1", approved, "Adding rand v0.9 to dependencies", 0));
  events.push(reasoning(
    "Review said a 503 is retried without waiting. Reading how the sender handles failures.",
  ));
  events.push(message(
    "I'll start with the sender: a retry loop with exponential backoff and jitter, then the attempt log.",
  ));
  events.extend(command(
    "read-1",
    "rg -n \"fn send\" src/webhooks",
    "src/webhooks/sender.rs:9:  pub async fn send",
    0,
  ));
  events.extend(command(
    "read-2",
    "sed -n 1,80p src/webhooks/sender.rs",
    "use crate::webhooks::{log::DeliveryLog, …}",
    0,
  ));
  events.extend(command(
    "read-3",
    "sed -n 1,40p src/webhooks/settings.rs",
    "pub struct Settings {",
    0,
  ));
  events.extend(command(
    "read-4",
    "rg -n retry tests",
    "tests/webhooks/retry.rs:4:fn the_default_budget…",
    0,
  ));
  events.push(edit("edit-1", &[("src/webhooks/sender.rs", FileChangeKind::Modified)]));
  events.push(edit(
    "edit-2",
    &[("src/webhooks/settings.rs", FileChangeKind::Modified)],
  ));
  events.extend(command(
    "test-1",
    "cargo test -p webhooks",
    "test webhooks::retry::backoff_stays_under_the_jitter_bound ... FAILED\ntest result: FAILED. 41 passed; 1 failed",
    101,
  ));
  events.push(message(
    "Added retry_budget to the endpoint settings (default 5). One test fails on the jitter bound; fixing it.",
  ));
  events.push(AgentEvent::Error {
    message: "stream disconnected before completion; reconnecting (1/5)".to_owned(),
  });
  events.push(message(links));
  for (index, path) in READ.into_iter().enumerate() {
    events.push(reasoning(&format!("Checking how {path} records an attempt.")));
    events.extend(command(
      &format!("read-{}", index + 5),
      &format!("sed -n 1,60p {path}"),
      "…",
      0,
    ));
  }
  events.push(edit(
    "edit-3",
    &[
      ("src/webhooks/log.rs", FileChangeKind::Modified),
      ("tests/webhooks/jitter.rs", FileChangeKind::Added),
    ],
  ));
  events.push(AgentEvent::CommandStarted {
    item: "test-2".to_owned(),
    command: "cargo test -p webhooks jitter".to_owned(),
  });
  events
    .into_iter()
    .zip(1..)
    .map(|(event, index)| {
      (
        Timestamp::from_unix_millis(start.unix_millis() + index * EVENT_MILLIS),
        event,
      )
    })
    .collect()
}

fn message(text: &str) -> AgentEvent {
  AgentEvent::Message { text: text.to_owned() }
}

fn reasoning(text: &str) -> AgentEvent {
  AgentEvent::Reasoning { text: text.to_owned() }
}

fn command(item: &str, line: &str, output: &str, exit: i64) -> [AgentEvent; 2] {
  [
    AgentEvent::CommandStarted {
      item: item.to_owned(),
      command: line.to_owned(),
    },
    AgentEvent::CommandFinished {
      item: item.to_owned(),
      command: line.to_owned(),
      output: output.to_owned(),
      exit_code: Some(exit),
      status: if exit == 0 {
        CommandStatus::Completed
      } else {
        CommandStatus::Failed
      },
    },
  ]
}

fn edit(item: &str, files: &[(&str, FileChangeKind)]) -> AgentEvent {
  AgentEvent::FileChanged {
    item: item.to_owned(),
    changes: files
      .iter()
      .map(|(path, kind)| FileChange {
        path: (*path).to_owned(),
        kind: *kind,
      })
      .collect(),
  }
}
