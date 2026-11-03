//! A profile's *Set by the project* (S14 a `o9GsNS`): the seven settings the project decides, read-only, each opening
//! its Project settings tab; the enforcement column has no Repository agent files row any more.
//! `cargo test -p orchester-app --test agents project_sets`.
use orchester_app::routes::{ProjectPage, ProjectSettingsTab, Route, SecretsView};
use orchester_core::{limits::ProjectLimits, store::SettingsStore, tokens::TokenBudget};
use orchester_store::ProjectDb;

use super::{harness::Agents, support::project_id};

const ROWS: [&str; 7] = [
  "autopilot",
  "sources",
  "agentFiles",
  "codeIndex",
  "memory",
  "environment",
  "limits",
];

fn hint(screen: &mut Agents, row: &str) -> String {
  screen.text(&format!("agent-project-sets-{row}-hint"))
}

/// Opens the profile `id` and waits until every row has read its setting.
fn opened(screen: &mut Agents, id: orchester_core::id::ResourceId) {
  screen.open(id);
  screen.until("the project's settings read", |s| {
    ROWS
      .iter()
      .all(|row| s.exists(&format!("agent-project-sets-{row}-hint")) && hint(s, row) != "Reading…")
  });
}

#[test]
fn project_sets_reads_what_the_project_decides() {
  let mut screen = Agents::new(true);
  let limits = ProjectLimits::new(
    Some(TokenBudget::new(200_000).expect("budget")),
    ProjectLimits::DEFAULT.daily_budget(),
    None,
  )
  .expect("limits");
  ProjectDb::open(&screen.root)
    .expect("database")
    .save_project_limits(&limits)
    .expect("limits saved");
  let id = screen.profile(true);
  opened(&mut screen, id);
  assert_eq!(screen.text("agent-project-sets-group-title"), "Set by the project");
  let said: Vec<String> = ROWS.iter().map(|row| hint(&mut screen, row)).collect();
  assert_eq!(
    said,
    [
      "Not allowed in Kontur · Runs › Approvals",
      "None · Context",
      "Read into every prompt · Context",
      "Off · Context",
      "8 records, 3k tokens an invocation · Context",
      "No variables · secrets go only to their tools",
      "200k tokens a run · no time limit · Runs › Limits",
    ]
  );
  assert_eq!(
    screen.text("agent-project-sets-note-text"),
    "Read here, changed only in Project settings. Each row opens its tab."
  );
  assert!(
    !screen.exists("agent-enforcement-agentFiles"),
    "the enforcement column no longer shows the repository's agent files"
  );
}

#[test]
fn project_sets_rows_open_their_tabs() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  let settings = |tab| ProjectPage::Settings(tab);
  for (row, page) in [
    ("autopilot", settings(ProjectSettingsTab::Runs)),
    ("sources", settings(ProjectSettingsTab::Context)),
    ("agentFiles", settings(ProjectSettingsTab::Context)),
    ("codeIndex", settings(ProjectSettingsTab::Context)),
    ("memory", settings(ProjectSettingsTab::Context)),
    ("environment", ProjectPage::Secrets(SecretsView::default())),
    ("limits", settings(ProjectSettingsTab::Runs)),
  ] {
    opened(&mut screen, id);
    screen.click(&format!("agent-project-sets-{row}"));
    assert_eq!(
      screen.window.navigation().route_now(),
      Route::Project(project_id(), page),
      "{row} opens its tab"
    );
  }
}
