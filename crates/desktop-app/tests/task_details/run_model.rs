//! The run's model chip in the Conversation's composer (ORC-322; 08 `DPofN`, 08b `p7r89Q`, 08c `b3a6z`, 08d `ff1nD`,
//! 08e `X2Hmq`): it says what the step's next turn runs on; its picker lists the models of the step's connection with
//! their efforts and applies a change for this step or the rest of the run, which an event line records; a change
//! made while a turn runs waits for it, and *Cancel change* gives the step back what it ran on; a connection with
//! nothing to choose from shows a lock and no Apply. The profile never changes.
//!
//! `cargo test -p orchester-app --test task_details run_model`

use orchester_app::{routes::TaskTab, ui::icon::Icon};
use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
    stream::InvocationOutcome,
  },
  name::Name,
  resource::{AgentProfile, ModelId, StepKey},
  run::{ModelChoice, ModelEventKind, ModelScope, Run, StepExecutor},
  store::{ActivityStore, ConnectionStore, ResourceStore, RunStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::Page;

const CHIP: &str = "conversation-model";
const LABEL: &str = "conversation-model-label";
const PICKER: &str = "conversation-model-picker";
/// The seeded profiles' model.
const PROFILE_MODEL: &str = "gpt-5.5";
const OTHER_MODEL: &str = "gpt-5.5-codex";

fn model(id: &str) -> ModelId {
  ModelId::new(id).expect("model")
}

/// The running task's latest run and the key of its step at work.
fn running(page: &Page) -> (String, Run, StepKey) {
  let key = page.key("running");
  let task = page.task(&key);
  let db = ProjectDb::open(&page.repo).expect("the project");
  let run = db
    .runs_of_task(task.id())
    .expect("runs")
    .into_iter()
    .filter_map(Result::ok)
    .last()
    .expect("a run");
  let step = at_work(&run).step().clone();
  (key, run, step)
}

/// The execution of `run` whose agent works now.
fn at_work(run: &Run) -> &orchester_core::run::StepExecution {
  run
    .current_attempt()
    .and_then(|attempt| {
      attempt
        .executions()
        .iter()
        .find(|execution| execution.status() == orchester_core::run::StepStatus::Running)
    })
    .expect("a step at work")
}

/// Stores the connection the profile of `step` names — the seed names one it does not store — as a checked Codex
/// listing `models`, its program (never run) in the project at `root`.
fn connect(db: &mut ProjectDb, (run, step): (&Run, &StepKey), root: &std::path::Path, models: Vec<CatalogModel>) {
  let Some(StepExecutor::Agent { profile, .. }) = run.snapshot().step(step).map(|step| step.executor()) else {
    panic!("the step at work is an agent's");
  };
  let id = db
    .version::<AgentProfile>(profile.resource, profile.number)
    .expect("read")
    .expect("the pinned profile")
    .body()
    .connection();
  let mut connection = AgentConnection::new(
    id,
    Name::new("Codex · Work").expect("name"),
    AgentProvider::CodexCli,
    root.join("bin").join("codex.exe"),
    Timestamp::now(),
  )
  .expect("an absolute program");
  connection
    .record_check(ConnectionCheck {
      at: Timestamp::now(),
      result: CheckResult::Available {
        version: "0.159.0".into(),
        sign_in: SignIn::SignedIn,
        catalog: ModelCatalog::new(CatalogSource::Provider, models),
      },
    })
    .expect("a CLI check");
  db.create_connection(&connection.into_connection()).expect("stored");
}

/// The running task with its step's connection listing `models`, and every turn of the run ended unless `turn` keeps
/// one running.
fn prepared(page: &Page, models: Vec<CatalogModel>, turn: bool) -> (String, Run, StepKey) {
  let (key, run, step) = running(page);
  let mut db = ProjectDb::open(&page.repo).expect("the project");
  connect(&mut db, (&run, &step), &page.repo, models);
  for mut invocation in db.invocations(run.id()).expect("invocations") {
    if invocation.ended_at.is_none() {
      invocation.ended_at = Some(Timestamp::now());
      invocation.outcome = Some(InvocationOutcome::Completed);
      db.save_invocation(&invocation).expect("ended");
    }
  }
  if turn {
    let execution = at_work(&run).id();
    db.start_invocation(run.id(), execution, &model(PROFILE_MODEL), Timestamp::now())
      .expect("a turn runs");
  }
  (key, run, step)
}

/// The profile's model and another, each with its efforts.
fn two_models() -> Vec<CatalogModel> {
  let efforts = |levels: &[&str]| levels.iter().map(|level| (*level).to_owned()).collect();
  vec![
    CatalogModel::new(model(PROFILE_MODEL), efforts(&["low", "medium", "high"])),
    CatalogModel::new(model(OTHER_MODEL), efforts(&["low", "medium", "high", "xhigh"])),
  ]
}

fn chosen(page: &Page, run: &Run, step: &StepKey) -> Option<ModelChoice> {
  ProjectDb::open(&page.repo)
    .expect("the project")
    .step_model(run.id(), step)
    .expect("read")
}

fn events(page: &Page, run: &Run) -> Vec<ModelEventKind> {
  ProjectDb::open(&page.repo)
    .expect("the project")
    .model_events(run.id())
    .expect("events")
    .into_iter()
    .map(|event| event.kind)
    .collect()
}

impl Page {
  fn glyph(&mut self, id: &str) -> Option<String> {
    self
      .window
      .tree
      .get_element_by_id(id)
      .and_then(|icon| icon.text_content().map(str::to_owned))
  }

  /// Opens the picker, picks the other model at Extra high for `scope`, and applies it.
  fn apply_other(&mut self, scope: &str) {
    self.window.press(CHIP);
    self.window.until("the picker", |window| window.texts(PICKER).is_some());
    self.window.press("conversation-model-option-1");
    self.window.press("conversation-model-effort");
    self.window.until("the efforts", |window| {
      window.texts("conversation-model-effort-menu").is_some()
    });
    self.window.press("conversation-model-effort-option-4");
    self.window.press(scope);
    self.window.press("conversation-model-apply");
    self
      .window
      .until("the picker closes", |window| window.texts(PICKER).is_none());
  }
}

#[test]
fn run_model_chip_shows_the_profiles_model_and_the_picker_the_connections_models() {
  let mut page = Page::seeded("task-states");
  let (key, ..) = prepared(&page, two_models(), false);
  page.open(&key, TaskTab::Conversation, CHIP);
  assert_eq!(page.text(LABEL), PROFILE_MODEL, "the profile names no effort");
  assert_eq!(
    page.glyph("conversation-model-icon"),
    Some(Icon::Cpu.glyph().to_string())
  );
  page.window.press(CHIP);
  page.window.until("the picker", |window| window.texts(PICKER).is_some());
  assert_eq!(
    page.text("conversation-model-heading"),
    "Builder · Codex · Work · its models"
  );
  let profile = page.text("conversation-model-option-0");
  assert!(
    profile.starts_with(PROFILE_MODEL) && profile.contains("The profile’s model"),
    "{profile}"
  );
  assert!(page.text("conversation-model-option-1").starts_with(OTHER_MODEL));
  assert!(
    page
      .text("conversation-model-scope-step")
      .starts_with("From the next turn of this step")
  );
  assert!(
    page
      .text("conversation-model-scope-run")
      .starts_with("For this step and the rest of the run")
  );
  assert_eq!(
    page.text("conversation-model-foot-note"),
    format!("Builder’s profile stays on {PROFILE_MODEL}.")
  );
  page.window.press("conversation-model-cancel");
  page
    .window
    .until("the picker closes", |window| window.texts(PICKER).is_none());
}

#[test]
fn run_model_change_for_this_step_applies_from_the_next_turn_with_an_event_line() {
  let mut page = Page::seeded("task-states");
  let (key, run, step) = prepared(&page, two_models(), false);
  page.open(&key, TaskTab::Conversation, CHIP);
  page.apply_other("conversation-model-scope-step");
  let expected = format!("{OTHER_MODEL} · Extra high");
  page.window.until("the chip reads the new model", |window| {
    window.texts(LABEL) == Some(vec![expected.clone()])
  });
  assert_eq!(
    chosen(&page, &run, &step),
    Some(ModelChoice {
      model: model(OTHER_MODEL),
      effort: Some("xhigh".to_owned()),
    })
  );
  assert!(matches!(
    events(&page, &run).as_slice(),
    [ModelEventKind::Changed {
      scope: ModelScope::Step,
      ..
    }]
  ));
  let line = "conversation-model-model-0";
  page
    .window
    .until("the event line", |window| window.texts(line).is_some());
  assert_eq!(page.text(&format!("{line}-text")), "Model changed for this step");
  assert_eq!(page.text(&format!("{line}-value")), expected);
  let detail = page.text(&format!("{line}-detail"));
  assert!(
    detail.starts_with(&format!("· was {PROFILE_MODEL} · by you · ")),
    "{detail}"
  );
  assert!(!page.has(&format!("{line}-cancel")), "no turn runs: nothing waits");
}

#[test]
fn run_model_change_for_the_rest_of_the_run_is_recorded_so() {
  let mut page = Page::seeded("task-states");
  let (key, run, _) = prepared(&page, two_models(), false);
  page.open(&key, TaskTab::Conversation, CHIP);
  page.apply_other("conversation-model-scope-run");
  let line = "conversation-model-model-0";
  page
    .window
    .until("the event line", |window| window.texts(line).is_some());
  assert_eq!(
    page.text(&format!("{line}-text")),
    "Model changed for the rest of the run"
  );
  assert!(matches!(
    events(&page, &run).as_slice(),
    [ModelEventKind::Changed {
      scope: ModelScope::Run,
      ..
    }]
  ));
}

#[test]
fn run_model_change_during_a_turn_waits_and_cancel_change_gives_the_step_back_its_model() {
  let mut page = Page::seeded("task-states");
  let (key, run, step) = prepared(&page, two_models(), true);
  page.open(&key, TaskTab::Conversation, CHIP);
  page.apply_other("conversation-model-scope-step");
  page.window.until("the chip waits", |window| {
    window.texts("conversation-model-after").is_some()
  });
  assert_eq!(page.text("conversation-model-after"), "after this turn");
  assert_eq!(
    page.glyph("conversation-model-icon"),
    Some(Icon::Hourglass.glyph().to_string())
  );
  let line = "conversation-model-model-0";
  page
    .window
    .until("the waiting line", |window| window.texts(line).is_some());
  assert_eq!(
    page.text(&format!("{line}-text")),
    "Model change waits for this turn to end"
  );
  assert_eq!(page.text(&format!("{line}-cancel")), "Cancel change");
  page.window.press(&format!("{line}-cancel"));
  page.window.until("the change canceled", |window| {
    window.texts("conversation-model-model-1").is_some()
  });
  assert_eq!(chosen(&page, &run, &step), None, "back to the profile's");
  assert_eq!(page.text("conversation-model-model-1-text"), "Model change canceled");
  assert!(!page.has(&format!("{line}-cancel")), "nothing waits any more");
  assert!(!page.has("conversation-model-after"));
  assert_eq!(page.text(LABEL), PROFILE_MODEL);
}

#[test]
fn run_model_connection_with_one_model_is_fixed_and_offers_no_apply() {
  let mut page = Page::seeded("task-states");
  let (key, ..) = prepared(&page, vec![CatalogModel::new(model(PROFILE_MODEL), Vec::new())], false);
  page.open(&key, TaskTab::Conversation, CHIP);
  assert_eq!(
    page.glyph("conversation-model-icon"),
    Some(Icon::Lock.glyph().to_string())
  );
  page.window.press(CHIP);
  page.window.until("the picker", |window| window.texts(PICKER).is_some());
  assert!(page.text("conversation-model-locked").starts_with(PROFILE_MODEL));
  assert_eq!(
    page.text("conversation-model-effort-none"),
    "Not offered by this connection"
  );
  assert!(!page.has("conversation-model-apply"));
  assert!(page.text("conversation-model-connections").starts_with("Connections"));
}

#[test]
fn run_model_chip_is_gone_once_the_run_ended() {
  let mut page = Page::seeded("task-states");
  let key = page.key("failed");
  page.open(&key, TaskTab::Conversation, "conversation");
  assert!(!page.has(CHIP));
}

#[test]
fn run_model_seeded_scenarios_reach_every_chip_state_for_qa() {
  for (scenario, glyph, label) in [
    ("conversation-model", Icon::Cpu, PROFILE_MODEL.to_owned()),
    (
      "conversation-model-changed",
      Icon::Cpu,
      format!("{OTHER_MODEL} · Extra high"),
    ),
    (
      "conversation-model-waits",
      Icon::Hourglass,
      format!("{OTHER_MODEL} · Extra high"),
    ),
    ("conversation-model-fixed", Icon::Lock, PROFILE_MODEL.to_owned()),
  ] {
    let mut page = Page::seeded(scenario);
    page.open("KON-13", TaskTab::Conversation, CHIP);
    assert_eq!(page.text(LABEL), label, "{scenario}");
    assert_eq!(
      page.glyph("conversation-model-icon"),
      Some(glyph.glyph().to_string()),
      "{scenario}"
    );
  }
}

#[test]
fn run_model_next_step_after_a_change_for_one_step_says_it_starts_on_its_profile() {
  let mut page = Page::seeded("conversation-model-back");
  let task = page.task("KON-13");
  let run = ProjectDb::open(&page.repo)
    .expect("the project")
    .runs_of_task(task.id())
    .expect("runs")
    .into_iter()
    .filter_map(Result::ok)
    .last()
    .expect("a run");
  let implement = run
    .current_attempt()
    .and_then(|attempt| {
      attempt
        .executions()
        .iter()
        .find(|execution| matches!(execution.status(), orchester_core::run::StepStatus::Waiting(_)))
    })
    .expect("Implement waits")
    .id();
  let line = format!("conversation-model-model-back-{implement}");
  page.open("KON-13", TaskTab::Conversation, &line);
  assert_eq!(page.text(&format!("{line}-text")), "03 Implement starts on its profile");
  assert_eq!(page.text(&format!("{line}-value")), PROFILE_MODEL);
  assert_eq!(
    page.text(&format!("{line}-detail")),
    "· the change was for 02 Plan only"
  );
}

#[test]
fn run_model_roles_follow_the_design() {
  use lurq::{app::App, layout::text_style::FontWeight};
  use orchester_app::theme::{self, Mode, ModelText};
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  let theme = app.theme();
  for (role, size, weight, line_height) in [
    (ModelText::Chip, 11.5, FontWeight::Normal, 1.3),
    (ModelText::Heading, 11.0, FontWeight::Medium, 1.3),
    (ModelText::Name, 12.0, FontWeight::Normal, 1.3),
    (ModelText::Tag, 11.0, FontWeight::Normal, 1.3),
    (ModelText::Label, 12.0, FontWeight::Normal, 1.3),
    (ModelText::Value, 12.0, FontWeight::Normal, 1.3),
    (ModelText::Sub, 11.5, FontWeight::Normal, 1.4),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.weight, style.line_height),
      (size, weight, line_height),
      "{role:?}"
    );
  }
}
