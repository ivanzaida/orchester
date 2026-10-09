//! The status bar as its facts say (`shell::status_bar::StatusBarView`), headless at the design's 1440 ×
//! 900 and at 960: what each item says in each state, what its actions ask the window to do, which of the
//! activity's candidates the slot shows, and the order in which the items give way on a narrow window.

use std::{
  num::{NonZeroU16, NonZeroU32},
  path::PathBuf,
  sync::Arc,
  time::{Duration, Instant, SystemTime},
};

use lurq::{
  app::{App, Tree, events::MouseButton},
  node::element::ElementRef,
};
use orchester_app::{
  i18n,
  mcp::McpStatus,
  routes::{OverviewTab, ProjectPage, Route, TaskView},
  shell::status_bar::{BarAction, BarFacts, BarProps, ProjectFacts, ReadFailure, RunnerFacts, StatusBarView, Stopping},
  theme::{self, Mode},
};
use orchester_core::{
  id::{InvocationId, ProjectId, RunId, StepExecutionId, TaskId},
  limits::ProjectLimits,
  run::{PendingToolApproval, RunNumber, SimilarUnavailable},
  time::Timestamp,
  tokens::TokensUsed,
};
use orchester_ipc::WorkSummary;
use orchester_workspace::{
  activity::{ActivityLog, OperationSpec, Target},
  messages::Message,
  runner::state::{RunnerFailure, RunnerState},
  tasks::{
    approvals::{AlwaysBlocked, ApprovalChoices, ShownApproval},
    data::{AutoApprovingRun, ProjectTasks},
    usage::ProjectUsage,
  },
};
use parking_lot::Mutex;

mod activity;
mod give_way;
mod items;
#[path = "../../examples/seed/kontur/mod.rs"]
#[allow(
  dead_code,
  reason = "the seed reports what it wrote; these tests read the window instead"
)]
mod kontur;
#[path = "../support/mod.rs"]
mod support;
mod window;

const ROOT: &str = "H:/projects/kontur";

/// The bar mounted alone, and what its actions asked for.
struct Bar {
  app: App,
  tree: Tree,
  asked: Arc<Mutex<Vec<BarAction>>>,
}

impl Bar {
  fn at(width: u32, facts: BarFacts) -> Self {
    let mut app = App::new();
    theme::install(&mut app, Mode::Dark);
    i18n::install(&app).expect("bundled locales load");
    let mut tree = Tree::new();
    tree.resize(width, 900);
    let asked = Arc::new(Mutex::new(Vec::new()));
    let record = asked.clone();
    let dispatch = Arc::new(move |action| record.lock().push(action));
    tree.mount_root::<StatusBarView>(&mut app, BarProps { facts, dispatch });
    tree.pass_headless(&mut app);
    Self { app, tree, asked }
  }

  fn of(facts: BarFacts) -> Self {
    Self::at(1440, facts)
  }

  fn texts(&mut self, id: &str) -> Option<Vec<String>> {
    let element = self.tree.get_element_by_id(id)?;
    let mut all = Vec::new();
    collect(element, &mut all);
    all.retain(|text| !text.chars().all(|glyph| ('\u{e000}'..='\u{f8ff}').contains(&glyph)));
    Some(all)
  }

  /// The width the element `id` takes; 0 once it gave way.
  fn width(&mut self, id: &str) -> f32 {
    self
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
      .width
  }

  fn press(&mut self, id: &str) {
    let bounds = self
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"));
    let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
    self.tree.mouse_move(x, y);
    self.tree.mouse_down(x, y, MouseButton::Left);
    self.tree.mouse_up(x, y, MouseButton::Left);
    self.tree.pass_headless(&mut self.app);
  }

  fn asked(&self) -> Vec<BarAction> {
    self.asked.lock().clone()
  }
}

fn collect(element: ElementRef<'_>, into: &mut Vec<String>) {
  into.extend(element.text_content().map(str::to_owned));
  for child in element.children() {
    collect(child, into);
  }
}

fn project_id() -> ProjectId {
  "3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b".parse().expect("a project id")
}

fn listening() -> McpStatus {
  McpStatus::Listening {
    port: NonZeroU16::new(52341).expect("a port"),
    window_control: true,
  }
}

fn runner(state: RunnerState) -> RunnerFacts {
  RunnerFacts {
    state,
    ..RunnerFacts::starting()
  }
}

/// Kontur with `runner`, its tasks read as `tasks` and today's usage read.
fn kontur(runner: RunnerFacts, tasks: ProjectTasks) -> ProjectFacts {
  ProjectFacts {
    id: project_id(),
    name: "Kontur".to_owned(),
    repo_root: PathBuf::from(ROOT),
    runner,
    tasks: Some(tasks),
    usage: Some(ProjectUsage {
      limits: ProjectLimits::default(),
      today: TokensUsed::new(3_800_000),
    }),
    read_failure: None,
  }
}

fn facts(project: Option<ProjectFacts>) -> BarFacts {
  BarFacts {
    project,
    log: ActivityLog::default(),
    mcp: listening(),
    stopping: Stopping::Idle,
  }
}

/// A request of Builder in KON-42 to run `cargo add serde`, waiting since now.
fn approval() -> ShownApproval {
  ShownApproval {
    approval: PendingToolApproval {
      id: "request-1".to_owned(),
      run: RunId::generate(),
      execution: StepExecutionId::generate(),
      invocation: InvocationId::generate(),
      agent: "Builder".to_owned(),
      command: true,
      scope: None,
      server: String::new(),
      action: "cargo add serde".to_owned(),
      target: None,
      requested_at: Timestamp::now(),
      native: None,
    },
    provider: None,
    vendor: None,
    model: None,
    read_only: false,
    run: None,
    task: Some("KON-42".into()),
    task_id: Some(TaskId::generate()),
    step: None,
    step_key: None,
    cwd: None,
    choices: ApprovalChoices {
      similar: Err(SimilarUnavailable::Unknown),
      always: Err(AlwaysBlocked::Missing),
      told: "Denied.".into(),
      codex: false,
      category: None,
      secrets: false,
      reason: None,
      auto_asks: true,
    },
    asking: None,
    auto_approving: false,
    review: None,
  }
}

fn auto_approving() -> AutoApprovingRun {
  AutoApprovingRun {
    run: RunId::generate(),
    number: RunNumber::new(NonZeroU32::new(142).expect("a run number")),
    task: TaskId::generate(),
    key: "KON-42".into(),
  }
}

fn operation(name: &str) -> OperationSpec {
  let message = |key: &'static str| Message::new(key).with("name", name.to_owned());
  OperationSpec::new(
    message("statusBar.operations.pipelineDraft.running"),
    message("statusBar.operations.pipelineDraft.done"),
    message("statusBar.operations.pipelineDraft.failed"),
  )
}
