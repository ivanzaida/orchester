//! What the Orchestrator's agent changes itself and what waits for the person (`docs/design/orchestrator-toolset.md`,
//! cards `b55NqO`, `UKgrR`, `Ee2JE`): it reads another project and changes only its own; publishing a draft waits
//! for the person's Publish, with the draft's facts and diff, and reads as *Published by you* once applied, even what
//! widens what agents may do, which the person decides; a draft saved again since is refused; with *Let the
//! Orchestrator publish drafts* on the chat publishes itself (*Confirmed*), but proposes what widens while *Let
//! automation change agent permissions* is off; canceling a run waits for the person; in auto mode a run starts at
//! once, kept with its *Auto mode* chip.

use std::time::{Duration, Instant};

use lurq::app::events::with_synthetic_input;
use orchester_app::{
  routes::{OrchestratorChat, ProjectPage, ProjectSettingsTab, Route},
  ui::tooltip::TOOLTIP_DELAY,
};
use orchester_core::{
  id::ResourceId,
  orchestrator::{ActionOutcome, ChatAction, EntryKind, change::Change},
  permission::{AgentPermissions, CommandPermission, NetworkAccess},
  resource::{AgentProfile, Draft, RepositoryAccess},
  run::RunStatus,
  store::{ResourceStore, RunStore, TaskStore},
  task::organization::{BlockingTask, ChangeActor, DependencyGate, Priority, TaskOrganization, WorkflowStatus},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::workspace::OrchestratorConsent;

use super::{
  harness::{Orchestrator, Setup},
  proposal::{outcomes, proposal_card, runs},
};

/// The conversation's actions, in order.
pub(super) fn actions(screen: &Orchestrator) -> Vec<ChatAction> {
  let db = screen.db();
  let Some(listing) = db.chats().expect("chats").pop() else {
    return Vec::new();
  };
  db.chat_entries(listing.chat.id())
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Action(action) => Some(action),
      _ => None,
    })
    .collect()
}

/// Sends `call` and waits until its turn kept `count` actions and ended.
pub(super) fn call(screen: &mut Orchestrator, call: &str, count: usize) {
  screen.send(call);
  screen.until("the call's action", |screen| outcomes(screen).len() == count);
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));
}

/// Kontur with a published Delivery pipeline and its instruction *Review checklist* v1 with a draft that changes a
/// line and adds one; the instruction's id.
fn with_instruction_draft(screen: &Orchestrator) -> orchester_core::id::ResourceId {
  let delivery = screen.delivery();
  let instruction = screen.instruction(
    delivery,
    (
      "Review checklist",
      "## Tests\n- Ask for tests where behaviour changes.\n- Run the suite once.",
    ),
  );
  screen.instruction_draft(
    (instruction, delivery),
    "## Tests\n- Every changed module has a test named after it.\n- Name a missing test in the review.\n- Run the suite once.",
  );
  instruction
}

const PUBLISH_INSTRUCTION: &str = r#"Call publish_instruction {"project":"Kontur","instruction":"Review checklist"}"#;
const PUBLISH_PIPELINE: &str = r#"Call publish_pipeline {"project":"Kontur","pipeline":"Delivery"}"#;

#[test]
fn orchestrator_reads_another_project_and_changes_only_its_own() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let atlas = screen.another("Atlas");
  screen.go(OrchestratorChat::New);
  screen.send(r#"Call list_tasks {"project":"Atlas"}"#);
  screen.until("the read's answer", |screen| {
    screen.text("orchestrator-conversation").contains("Tool said") && !screen.exists("orchestrator-thinking")
  });
  let answer = screen.text("orchestrator-conversation");
  assert!(
    answer.contains("\"tasks\"") && !answer.contains("other_project"),
    "another project reads: {answer}"
  );
  assert!(outcomes(&screen).is_empty(), "a read is no action");

  screen.send(r#"Call create_task {"project":"Atlas","title":"Not here"}"#);
  screen.until("the other project's refusal", |screen| {
    screen.text("orchestrator-conversation").contains("other_project") && !screen.exists("orchestrator-thinking")
  });
  let theirs = ProjectDb::open(&atlas).expect("Atlas").tasks().expect("tasks");
  assert!(theirs.is_empty(), "nothing lands in another project: {theirs:?}");
}

#[test]
fn orchestrator_publication_waits_for_the_person_with_its_facts_and_diff() {
  let mut screen = Orchestrator::new(Setup::Ready);
  with_instruction_draft(&screen);
  screen.go(OrchestratorChat::New);
  call(&mut screen, PUBLISH_INSTRUCTION, 1);
  let action = actions(&screen).remove(0);
  assert_eq!(
    action.outcome,
    ActionOutcome::Waits,
    "nothing is published on the agent's word"
  );
  assert!(matches!(
    action.change.map(|change| change.change),
    Some(Change::Publish { version: 2, .. })
  ));
  assert!(
    screen
      .text("orchestrator-conversation")
      .contains("waits for the person"),
    "the agent was told it waits"
  );
  let card = proposal_card(&screen);
  screen.until("the card's facts", |screen| screen.exists(&format!("{card}-diff")));
  assert!(
    screen
      .text(&format!("{card}-title"))
      .contains("Publish Review checklist v2?")
  );
  let facts = screen.text(&format!("{card}-facts"));
  for shown in [
    "Instruction",
    "Review checklist",
    "v1 published",
    "v2 draft, saved",
    "Applies to",
    "Delivery › Implement",
  ] {
    assert!(facts.contains(shown), "the facts name {shown}: {facts}");
  }
  let diff = screen.text(&format!("{card}-diff"));
  assert!(
    diff.contains("Ask for tests where behaviour changes.") && diff.contains("Name a missing test in the review."),
    "the diff shows the removed and added lines: {diff}"
  );
  assert!(screen.text(&card).contains("1 line removed, 2 added"));

  let publish = format!("{card}-start");
  with_synthetic_input(|| screen.click(&publish));
  screen.until("the refusal", |screen| screen.exists(&format!("{card}-person-only")));
  assert_eq!(
    actions(&screen)[0].outcome,
    ActionOutcome::Waits,
    "automation's input publishes nothing"
  );

  screen.click(&publish);
  screen.until("the publication", |screen| {
    actions(screen)[0].outcome == ActionOutcome::Applied
  });
  // Decided, the card folds into its line.
  screen.until("the published line", |screen| {
    screen
      .text(&format!("{card}-title"))
      .contains("Published Review checklist v2")
      && screen.text(&format!("{card}-target")) == "by you"
  });
  assert!(
    !screen.exists(&format!("{card}-diff")),
    "a published draft shows no diff"
  );
  assert!(!screen.exists(&format!("{card}-start")), "nothing waits any more");
}

#[test]
fn orchestrator_person_publishes_what_widens_and_dismisses() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let builder = screen.builder();
  let delivery = screen.pipeline(builder, ("delivery", "Delivery"));
  // New instructions are not provably as strict: automation could not publish it, the person may.
  screen.pipeline_draft((delivery, builder), "Read README.md twice.");
  screen.go(OrchestratorChat::New);
  call(&mut screen, PUBLISH_PIPELINE, 1);
  let card = proposal_card(&screen);
  screen.until("the card's facts", |screen| {
    screen.text(&format!("{card}-facts")).contains("Widens")
  });
  screen.click(&format!("{card}-start"));
  screen.until("the publication", |screen| {
    actions(screen)[0].outcome == ActionOutcome::Applied
  });
  assert_eq!(
    screen.pipeline_version(delivery),
    2,
    "the person's Publish published what widens"
  );

  screen.pipeline_draft((delivery, builder), "Read README.md three times.");
  call(&mut screen, PUBLISH_PIPELINE, 2);
  let card = proposal_card(&screen);
  screen.until("its buttons", |screen| screen.exists(&format!("{card}-dismiss")));
  screen.click(&format!("{card}-dismiss"));
  screen.until("the dismissal", |screen| {
    actions(screen)[1].outcome == ActionOutcome::Dismissed
  });
  screen.until("the dismissed card", |screen| {
    screen
      .text(&format!("{card}-title"))
      .contains("Publish Delivery v3 · dismissed")
  });
  assert_eq!(screen.pipeline_version(delivery), 2, "nothing was published");
  assert!(
    screen.exists(&format!("{card}-open")),
    "the draft opens from the dismissed line"
  );
}

#[test]
fn orchestrator_widening_card_shows_what_it_widens_without_details() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let builder = screen.builder();
  let delivery = screen.pipeline(builder, ("delivery", "Delivery"));
  screen.pipeline_draft((delivery, builder), "Read README.md twice.");
  // Let the Orchestrator publish drafts stays off: the card still warns that the draft widens.
  screen.go(OrchestratorChat::New);
  call(&mut screen, PUBLISH_PIPELINE, 1);
  let card = proposal_card(&screen);
  screen.until("the warning", |screen| screen.exists(&format!("{card}-widens")));
  assert!(
    screen
      .text(&format!("{card}-widens"))
      .contains("This draft widens what Delivery may do."),
    "{}",
    screen.text(&format!("{card}-widens"))
  );
  // Right under the draft's name, among the facts shown without Show details.
  let fact = screen.text(&format!("{card}-fact-1"));
  assert!(
    fact.starts_with("Widens"),
    "the second fact says what it widens: {fact}"
  );
}

#[test]
fn orchestrator_publication_refuses_a_draft_saved_after_it_was_proposed() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let builder = screen.builder();
  let delivery = screen.pipeline(builder, ("delivery", "Delivery"));
  screen.pipeline_draft((delivery, builder), "Read README.md twice.");
  screen.go(OrchestratorChat::New);
  call(&mut screen, PUBLISH_PIPELINE, 1);
  let card = proposal_card(&screen);
  // The draft changes after the person was asked: Publish must not publish what they did not read.
  screen.pipeline_draft((delivery, builder), "Delete the repository.");
  screen.until("its button", |screen| screen.exists(&format!("{card}-start")));
  screen.click(&format!("{card}-start"));
  screen.until("the refusal", |screen| {
    matches!(&actions(screen)[0].outcome, ActionOutcome::Refused { code, .. } if code == "changed_since_proposed")
  });
  assert_eq!(screen.pipeline_version(delivery), 1, "nothing was published");
  screen.until("the refused line", |screen| {
    screen.text(&format!("{card}-title")).contains("· not done")
  });
  // When the draft changed, as the person reads it under the refused line.
  let saved = screen
    .db()
    .draft::<orchester_core::resource::Pipeline>(delivery)
    .expect("read")
    .expect("the draft")
    .updated_at();
  let changed = format!(
    "The draft changed at {}, after this proposal.",
    orchester_workspace::display::clock(saved)
  );
  screen.click(&format!("{card}-line"));
  screen.until("the reason", |screen| {
    screen.text(&format!("{card}-reason")).contains(&changed)
  });
}

#[test]
fn orchestrator_publishes_itself_once_the_person_lets_it() {
  let mut screen = Orchestrator::new(Setup::Ready);
  with_instruction_draft(&screen);
  screen.let_orchestrator(OrchestratorConsent::PublishDrafts, true);
  screen.go(OrchestratorChat::New);
  call(&mut screen, PUBLISH_INSTRUCTION, 1);
  assert_eq!(actions(&screen)[0].outcome, ActionOutcome::Done);
  let card = proposal_card(&screen);
  screen.until("the line", |screen| {
    screen
      .text(&format!("{card}-title"))
      .contains("Published Review checklist v2")
  });
  // Done by the chat itself: its line says why on hover and once opened.
  screen.click(&format!("{card}-line"));
  screen.until("why", |screen| {
    screen
      .text(&format!("{card}-reason"))
      .contains("so the chat published it without asking")
  });
  // The switch that let it is the project's own: the line's other link opens its tab.
  let link = format!("{card}-more-0");
  screen.until("the switch's link", |screen| {
    screen.text(&link).contains("What the Orchestrator may do")
  });
  screen.click(&link);
  let orchestrator = Route::Project(screen.project, ProjectPage::Settings(ProjectSettingsTab::Orchestrator));
  screen.until("Project settings › Orchestrator", |screen| {
    screen.route() == orchestrator
  });
}

#[test]
fn orchestrator_proposes_what_widens_even_when_it_may_publish() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let builder = screen.builder();
  let delivery = screen.pipeline(builder, ("delivery", "Delivery"));
  screen.pipeline_draft((delivery, builder), "Read README.md twice.");
  screen.let_orchestrator(OrchestratorConsent::PublishDrafts, true);
  screen.go(OrchestratorChat::New);
  call(&mut screen, PUBLISH_PIPELINE, 1);
  assert_eq!(
    actions(&screen)[0].outcome,
    ActionOutcome::Waits,
    "what widens waits for the person"
  );
  assert_eq!(screen.pipeline_version(delivery), 1);
  let card = proposal_card(&screen);
  screen.until("the note", |screen| screen.exists(&format!("{card}-widens")));
  assert!(
    screen
      .text(&format!("{card}-widens"))
      .contains("even though Let the Orchestrator publish drafts is on")
  );
}

/// A draft of profile `builder` that turns its network on: it widens what its agents may do.
fn network_draft(screen: &Orchestrator, builder: ResourceId) {
  let mut db = screen.db();
  let latest = db
    .latest_version::<AgentProfile>(builder)
    .expect("read")
    .expect("published");
  let body = latest.body().clone().with_permissions(AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::AskBefore,
    NetworkAccess::On,
  ));
  db.update_draft::<AgentProfile>(builder, |_, _| {
    Ok(Draft::new(body.clone(), Some(latest.number()), Timestamp::now()))
  })
  .expect("draft");
}

#[test]
fn orchestrator_profile_publication_says_what_the_profile_keeps_and_widens() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let builder = screen.builder();
  network_draft(&screen, builder);
  screen.go(OrchestratorChat::New);
  call(
    &mut screen,
    r#"Call publish_profile {"project":"Kontur","profile":"Builder"}"#,
    1,
  );
  let card = proposal_card(&screen);
  screen.until("the card's facts", |screen| {
    screen.text(&format!("{card}-facts")).contains("Widens")
  });
  let facts = screen.text(&format!("{card}-facts"));
  assert!(facts.contains("network: on"), "the short form of what widens: {facts}");
  // What publishing does is the question's tooltip, not a paragraph (`bxJZp`).
  assert!(!screen.exists(&format!("{card}-effect")), "{}", screen.text(&card));
  let title = screen.bounds(&format!("{card}-title"));
  screen
    .window
    .tree
    .mouse_move(title.x + title.width / 2.0, title.y + title.height / 2.0);
  screen.window.pass();
  screen
    .window
    .tree
    .tick_timers_at(Instant::now() + TOOLTIP_DELAY + Duration::from_millis(50));
  screen.window.pass();
  let tip = format!("{card}-why-tooltip");
  assert!(screen.exists(&tip), "the question's tooltip shows");
  assert!(
    screen
      .text(&tip)
      .contains("Nothing is published yet. Builder keeps v1 until you publish v2."),
    "{}",
    screen.text(&tip)
  );
}

#[test]
fn orchestrator_starts_in_auto_mode_and_cancel_waits_for_the_person() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let delivery = screen.delivery();
  let key = screen.draft_with(delivery, "Keep going", ("Keep working.", &["It keeps working."]));
  screen.allow_autopilot(true);
  screen.go(OrchestratorChat::New);
  call(
    &mut screen,
    &format!(r#"Call run_task {{"project":"Kontur","key":"{key}"}}"#),
    1,
  );
  let started = actions(&screen).remove(0);
  assert_eq!(
    started.outcome,
    ActionOutcome::Done,
    "Autopilot allowed starts it at once"
  );
  assert!(started.auto_mode && started.started.is_some(), "{started:?}");
  assert_eq!(runs(&screen, &key), 1);
  let card = proposal_card(&screen);
  screen.until("the started line", |screen| {
    screen.text(&format!("{card}-chip")) == "Autopilot allowed"
      && screen
        .text(&format!("{card}-title"))
        .contains(&format!("Started {key} on Delivery"))
  });

  let run = run_of(&screen, &key);
  call(
    &mut screen,
    &format!(r#"Call cancel_run {{"project":"Kontur","run":"{}"}}"#, run.id()),
    2,
  );
  let cancel = actions(&screen).remove(1);
  assert_eq!(cancel.outcome, ActionOutcome::Waits);
  assert!(matches!(
    cancel.change.map(|change| change.change),
    Some(Change::Cancel { .. })
  ));
  assert!(!run_of(&screen, &key).status().is_terminal(), "the run goes on");
  let card = proposal_card(&screen);
  screen.until("its button", |screen| screen.exists(&format!("{card}-start")));
  assert!(
    screen
      .text(&format!("{card}-title"))
      .contains(&format!("Cancel run #1 of {key}?"))
  );
  screen.click(&format!("{card}-start"));
  screen.until("the cancel", |screen| {
    actions(screen)[1].outcome == ActionOutcome::Applied
  });
  screen.until("the canceled run", |screen| {
    *run_of(screen, &key).status() == RunStatus::Cancelled
  });
  screen.until("the cancelled line", |screen| {
    screen.text(&format!("{card}-title")).starts_with("Cancelled run #1")
      && screen.text(&format!("{card}-target")).ends_with("by you")
  });
}

/// Task `key` waits for task `blocker` to be merged.
fn block(screen: &Orchestrator, key: &str, blocker: &str) {
  let mut db = screen.db();
  let id = |key: &str| {
    db.tasks()
      .expect("tasks")
      .into_iter()
      .find(|task| task.key().to_string() == key)
      .expect("the task")
      .id()
  };
  let (task, blocker) = (id(key), id(blocker));
  let linked = TaskOrganization::new(
    WorkflowStatus::Todo,
    Priority::High,
    None,
    vec![BlockingTask {
      task: blocker,
      gate: DependencyGate::Merge,
    }],
    vec![],
  )
  .expect("linked");
  db.set_task_organization(task, &linked, ChangeActor::Person, Timestamp::now())
    .expect("link");
}

#[test]
fn orchestrator_auto_mode_names_the_task_a_start_waits_for() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let delivery = screen.delivery();
  let first = screen.draft_with(delivery, "Lay the ground", ("Lay it.", &["It is laid."]));
  let second = screen.draft_with(delivery, "Build on it", ("Build.", &["It is built."]));
  block(&screen, &second, &first);
  screen.allow_autopilot(true);
  screen.go(OrchestratorChat::New);
  call(
    &mut screen,
    &format!(r#"Call run_task {{"project":"Kontur","key":"{second}"}}"#),
    1,
  );
  let refused = actions(&screen).remove(0);
  let named = format!("{second} waits for {first} to finish.");
  assert!(
    matches!(&refused.outcome, ActionOutcome::Refused { reason, .. } if reason.contains(&named)),
    "{refused:?}"
  );
  assert_eq!(runs(&screen, &second), 0, "nothing started");
}

/// The latest run of task `key`.
fn run_of(screen: &Orchestrator, key: &str) -> orchester_core::run::Run {
  let db = screen.db();
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)
    .expect("the task");
  db.runs_of_task(task.id())
    .expect("runs")
    .pop()
    .expect("a run")
    .expect("a readable run")
}
