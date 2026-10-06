//! Auto mode in the desktop, from runs and requests seeded into the project database: the run's switch in
//! its Approvals panel and its permissions popover, fixed with the reason while the project has no Claude
//! Code connection; Project settings' *New runs use auto mode* and reviewer model; why auto mode left a
//! request to the person, in the approval dialog; and the run's switch through Orchester's MCP server,
//! only while automation may answer approvals.
//! `cargo test -p orchester-desktop --test form_keyboard auto_mode`.

use std::path::Path;

use orchester_core::{
  connection::{AgentConnection, AgentProvider, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn},
  id::ConnectionId,
  name::Name,
  run::{
    PendingToolApproval,
    review::{Review, ReviewHold},
  },
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::{AppDb, ProjectDb};
use serde_json::json;

use crate::{
  app_mcp::{agent, ok, refused},
  approval_choices::open_task,
  github::shows,
  harness::Desktop,
  native_run::native_run,
  task_run::seed_claude,
  unreadable_run::at,
};

const SWITCH: &str = "task-auto-mode";

/// A seeded run with a waiting MCP request that auto mode left to the person with `review`, in a project
/// with a Claude Code connection when `reviewer`; the run's id.
fn seeded(reviewer: bool, review: Option<&Review>) -> (Desktop, orchester_core::id::RunId) {
  let mut seeded = None;
  let desktop = Desktop::with_seeded_project_and_app_mcp(|repo| {
    if reviewer {
      claude_connection(repo);
    }
    let (run, invocation) = native_run(repo);
    let request = PendingToolApproval {
      id: format!("{}:call_1", invocation.id),
      run: run.id(),
      execution: invocation.execution,
      invocation: invocation.id,
      agent: "Designer".to_owned(),
      command: false,
      scope: None,
      server: "pencil".to_owned(),
      action: "batch_design".to_owned(),
      target: None,
      requested_at: at(20),
      native: None,
    };
    let mut db = ProjectDb::open(repo).expect("open");
    db.request_tool_approval(&request).expect("request");
    if let Some(review) = review {
      db.record_review(run.id(), &request.id, review).expect("review");
    }
    seeded = Some(run.id());
  });
  (desktop, seeded.expect("seeded"))
}

/// A signed-in Claude Code connection, which auto mode reviews with.
fn claude_connection(repo: &Path) {
  let now = Timestamp::now();
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Claude · Kontur").expect("name"),
    AgentProvider::ClaudeCodeCli,
    repo.join("orchester-fake-claude"),
    now,
  )
  .expect("absolute path");
  connection
    .record_check(ConnectionCheck {
      at: now,
      result: CheckResult::Available {
        version: "2.1.89".into(),
        sign_in: SignIn::SignedIn,
        catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
      },
    })
    .expect("an agent check fits a CLI connection");
  ProjectDb::open(repo)
    .expect("open")
    .create_connection(&connection.into_connection())
    .expect("connection");
}

fn run_auto_mode(repo: &Path, run: orchester_core::id::RunId) -> bool {
  ProjectDb::open(repo).expect("open").run_auto_mode(run).expect("read")
}

#[test]
fn auto_mode_dialog_says_why_auto_mode_left_the_request_to_the_person() {
  let asked = Review::Asked {
    reason: "Redesigning the switcher is not part of the task.".to_owned(),
    model: "claude-haiku-4-5-20251001".to_owned(),
  };
  let (mut desktop, _) = seeded(true, Some(&asked));
  desktop.wait_for("tool-approval-dialog");
  desktop.wait_for("tool-approval-review");
  assert!(shows(
    &mut desktop,
    "tool-approval-review",
    "Auto mode asked you: Redesigning the switcher is not part of the task."
  ));

  let held = Review::Held {
    hold: ReviewHold::Secrets,
  };
  let (mut desktop, _) = seeded(true, Some(&held));
  desktop.wait_for("tool-approval-review");
  assert!(shows(
    &mut desktop,
    "tool-approval-review",
    "Auto mode left this to you: it reads a credential location."
  ));
}

#[test]
fn auto_mode_run_switch_saves_at_once_and_says_when_it_is_unavailable() {
  let (mut desktop, run) = seeded(true, None);
  desktop.key("Escape", "Escape");
  open_task(&mut desktop);
  desktop.wait_for(SWITCH);
  assert!(
    !run_auto_mode(desktop.repo(), run),
    "off for a run created before the setting"
  );
  let repo = desktop.repo().to_path_buf();
  desktop.wait_until("auto mode is on for the run", |desktop| {
    if run_auto_mode(&repo, run) {
      return true;
    }
    desktop.click(SWITCH);
    false
  });

  let (mut desktop, _) = seeded(false, None);
  desktop.key("Escape", "Escape");
  open_task(&mut desktop);
  desktop.wait_for("task-auto-mode-unavailable");
  assert!(shows(
    &mut desktop,
    "task-auto-mode-unavailable",
    "Auto mode needs a Claude Code connection"
  ));
}

#[test]
fn auto_mode_project_setting_turns_new_runs_on() {
  let mut desktop = Desktop::with_seeded_project(|repo| seed_claude(repo, &repo.join("orchester-fake-codex")));
  desktop.wait_for("nav-settings");
  desktop.click("nav-settings");
  desktop.wait_for("settings-auto-mode-new-runs");
  assert!(shows(
    &mut desktop,
    "settings-auto-mode-section",
    "claude-haiku-4-5-20251001"
  ));
  assert!(shows(
    &mut desktop,
    "settings-auto-mode-section",
    "Reviews with Claude · Kontur"
  ));
  let repo = desktop.repo().to_path_buf();
  let new_runs = |repo: &Path| {
    ProjectDb::open(repo)
      .expect("open")
      .auto_mode_settings()
      .expect("read")
      .new_runs
  };
  assert!(!new_runs(&repo), "off until a person turns it on");
  desktop.wait_until("new runs use auto mode", |desktop| {
    if new_runs(&repo) {
      return true;
    }
    desktop.click("settings-auto-mode-new-runs");
    false
  });

  let mut without = Desktop::with_project();
  without.wait_for("nav-settings");
  without.click("nav-settings");
  without.wait_for("settings-auto-mode-unavailable");
}

#[test]
fn auto_mode_run_switch_through_mcp_only_while_automation_may_answer() {
  let (desktop, run) = seeded(true, None);
  let mut client = agent(&desktop);
  let arguments = json!({"project": "Kontur", "run": run.to_string(), "enabled": true});
  let off = refused(&mut client, "set_run_auto_mode", &arguments);
  assert_eq!(off["code"], "automation_not_allowed");
  let database = desktop.repo().parent().expect("the test folder").join("orchester.db");
  AppDb::open(&database)
    .expect("open")
    .set_automation_answers_approvals(true)
    .expect("a person lets automation answer");
  let on = ok(&mut client, "set_run_auto_mode", &arguments);
  assert_eq!(on["run"]["auto_mode"], true);
  assert!(run_auto_mode(desktop.repo(), run));
  let read = ok(
    &mut client,
    "get_run",
    &json!({"project": "Kontur", "run": run.to_string()}),
  );
  assert_eq!(read["auto_mode"], true);

  let (desktop, run) = seeded(false, None);
  AppDb::open(&desktop.repo().parent().expect("the test folder").join("orchester.db"))
    .expect("open")
    .set_automation_answers_approvals(true)
    .expect("a person lets automation answer");
  let mut client = agent(&desktop);
  let unavailable = refused(
    &mut client,
    "set_run_auto_mode",
    &json!({"project": "Kontur", "run": run.to_string(), "enabled": true}),
  );
  assert_eq!(unavailable["code"], "auto_mode_unavailable");
}
