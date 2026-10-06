//! Approvals through Orchester's MCP server: the waiting requests as the approval card shows them,
//! read while the approval dialog, a modal, covers the window; and answers, refused while *Let
//! automation answer approvals* is off and, once a person turns it on in the MCP item's popover,
//! carried out by the runner and recorded as automation's.

use std::path::Path;

use lurq::app::theme::PaletteColor;
use orchester_core::{
  connection::AgentProvider,
  run::PendingToolApproval,
  store::{RunStore, TaskStore},
};
use orchester_store::{AppDb, ProjectDb};
use serde_json::{Value, json};

use super::{agent, ok, refused};
use crate::{
  harness::Desktop,
  mcp_agent::McpAgent,
  native_run::native_run,
  task_run::{agent_step, fake_claude, has, init_repository, programs, seed_agent},
  unreadable_run::at,
};

const DIALOG: &str = "tool-approval-dialog";

/// The waiting requests of the project, oldest first.
pub(super) fn waiting(repo: &Path) -> Vec<PendingToolApproval> {
  ProjectDb::open(repo)
    .expect("open")
    .pending_tool_approvals_all()
    .expect("requests")
}

fn ids(listed: &Value) -> Vec<String> {
  listed["approvals"]
    .as_array()
    .expect("approvals")
    .iter()
    .filter_map(|approval| approval["id"].as_str().map(str::to_owned))
    .collect()
}

#[test]
fn app_mcp_answers_while_the_approval_dialog_is_open() {
  let mut seeded = None;
  let mut desktop = Desktop::with_seeded_project_and_app_mcp(|repo| {
    let (run, invocation) = native_run(repo);
    let request = PendingToolApproval {
      id: format!("{}:call_1", invocation.id),
      run: run.id(),
      execution: invocation.execution,
      invocation: invocation.id,
      agent: "Designer".to_owned(),
      command: false,
      scope: Some(r#"{"filePath":"design.pen"}"#.to_owned()),
      server: "pencil".to_owned(),
      action: "batch_design".to_owned(),
      target: None,
      requested_at: at(20),
      native: None,
    };
    ProjectDb::open(repo)
      .expect("open")
      .request_tool_approval(&request)
      .expect("pending approval");
    seeded = Some((run.id(), request.id));
  });
  let (run, request) = seeded.expect("seeded");
  desktop.wait_for(DIALOG);
  let mut agent = agent(&desktop);

  let listed = ok(&mut agent, "list_approvals", &json!({"project": "Kontur"}));
  let approval = &listed["approvals"][0];
  assert_eq!(approval["id"], request.as_str());
  assert_eq!(approval["kind"], "mcp_tool");
  assert_eq!(approval["server"], "pencil");
  assert_eq!(approval["action"], "batch_design");
  assert_eq!(approval["arguments"], r#"{"filePath":"design.pen"}"#);
  assert_eq!(approval["agent"], "Designer");
  assert_eq!(approval["task"], "KON-1");
  assert_eq!(approval["run"]["number"], 1);
  assert_eq!(approval["waiting_since"], 20);
  let of_run = ok(&mut agent, "list_approvals", &json!({"project": "Kontur", "run": "#1"}));
  assert_eq!(ids(&of_run), [request.as_str()]);
  let read = ok(
    &mut agent,
    "get_run",
    &json!({"project": "Kontur", "run": run.to_string()}),
  );
  assert_eq!(
    ids(&json!({"approvals": read["awaiting"]["approvals"]})),
    [request.as_str()]
  );
  let created = ok(
    &mut agent,
    "create_task",
    &json!({"project": "Kontur", "title": "Behind the dialog"}),
  );
  assert_eq!(created["task"]["key"], "KON-2");

  let off = refused(
    &mut agent,
    "decide_approval",
    &json!({"project": "Kontur", "approval": request, "decision": "approve_once"}),
  );
  assert_eq!(off["code"], "automation_not_allowed");
  desktop.pass();
  assert!(has(&mut desktop, DIALOG), "the dialog still waits for the person");
}

/// `decide_approval`'s arguments.
pub(crate) fn answer(approval: &str, decision: &str) -> Value {
  json!({"project": "Kontur", "approval": approval, "decision": decision})
}

/// Creates task `key` whose brief tells the agent what to run, runs it and waits for its first request.
pub(super) fn run_until_asked(
  desktop: &mut Desktop,
  agent: &mut McpAgent,
  (key, brief): (&str, &str),
) -> PendingToolApproval {
  let created = ok(
    agent,
    "create_task",
    &json!({"project": "Kontur", "title": key, "brief": brief, "criteria": ["It ran"], "pipeline": "Delivery"}),
  );
  assert_eq!(created["task"]["key"], key);
  ok(agent, "run_task", &json!({"project": "Kontur", "key": key}));
  let repo = desktop.repo().to_path_buf();
  desktop.wait_until("the agent asks", |_| !waiting(&repo).is_empty());
  desktop.wait_for(DIALOG);
  waiting(&repo).remove(0)
}

/// Waits until the latest run of task `key` ended and the dialog closed; that run's answers.
pub(super) fn ended(desktop: &mut Desktop, key: &str) -> orchester_store::DecisionCounts {
  let repo = desktop.repo().to_path_buf();
  let latest = |repo: &Path| {
    let db = ProjectDb::open(repo).expect("open");
    let task = db
      .tasks()
      .expect("tasks")
      .into_iter()
      .find(|task| task.key().to_string() == key)
      .expect("the task");
    db.runs_of_task(task.id())
      .expect("runs")
      .pop()
      .map(|run| run.expect("readable"))
  };
  desktop.wait_until("the run ends", |_| {
    latest(&repo).is_some_and(|run| run.status().is_terminal())
  });
  desktop.wait_until("the dialog closes", |desktop| !has(desktop, DIALOG));
  let run = latest(&repo).expect("the run");
  ProjectDb::open(&repo)
    .expect("open")
    .decision_counts(run.id())
    .expect("counts")
}

#[test]
fn app_mcp_decides_approvals_only_while_a_person_lets_it() {
  let (runner, fake) = programs();
  let fake = fake_claude(&fake);
  let mut desktop = Desktop::with_runner_and_app_mcp(runner, init_repository, |repo| {
    seed_agent(repo, &fake, AgentProvider::ClaudeCodeCli, |builder| {
      vec![agent_step(
        builder,
        "implement",
        "Implement",
        "Do what the task asks.",
        &[],
      )]
    });
  });
  let mut agent = agent(&desktop);
  let install = run_until_asked(&mut desktop, &mut agent, ("KON-1", "Run `cargo add serde`."));
  let listed = ok(&mut agent, "list_approvals", &json!({"project": "Kontur"}));
  assert_eq!(listed["approvals"][0]["kind"], "command");
  assert_eq!(listed["approvals"][0]["action"], "cargo add serde");
  let off = refused(&mut agent, "decide_approval", &answer(&install.id, "approve_once"));
  assert_eq!(off["code"], "automation_not_allowed");

  // A person lets automation answer, in the MCP item's popover, while the dialog is open.
  let_automation_answer(&mut desktop, true);
  // An install is left to a person, as auto-approval leaves it; automation may only deny it.
  let person_only = refused(&mut agent, "decide_approval", &answer(&install.id, "approve_once"));
  assert_eq!(person_only["code"], "person_only");
  assert!(
    person_only["message"]
      .as_str()
      .is_some_and(|message| message.starts_with("Only a person"))
  );
  ok(&mut agent, "decide_approval", &answer(&install.id, "deny"));
  let unknown = refused(&mut agent, "decide_approval", &answer(&install.id, "deny"));
  assert_eq!(unknown["code"], "approval_not_found");
  let counts = ended(&mut desktop, "KON-1");
  assert_eq!((counts.by_automation, counts.denied_by_automation), (0, 1));

  let first = run_until_asked(&mut desktop, &mut agent, ("KON-2", "Run `cargo test -p core` twice."));
  let once = ok(&mut agent, "decide_approval", &answer(&first.id, "approve_once"));
  assert_eq!(once["approval"]["decided"], "approve_once");
  let repo = desktop.repo().to_path_buf();
  desktop.wait_until("the agent asks again", |_| {
    waiting(&repo).iter().any(|request| request.id != first.id)
  });
  let second = waiting(&repo).remove(0);
  ok(&mut agent, "decide_approval", &answer(&second.id, "deny"));
  let counts = ended(&mut desktop, "KON-2");
  assert_eq!(
    (counts.by_automation, counts.denied_by_automation, counts.by_you),
    (1, 1, 0)
  );
}

/// Whether *Let automation answer approvals* is on in the application database of `desktop`.
fn allowed(desktop: &Desktop) -> bool {
  let database = desktop.repo().parent().expect("the test folder").join("orchester.db");
  AppDb::open(&database)
    .and_then(|db| db.automation_answers_approvals())
    .expect("read the setting")
}

/// Flips *Let automation answer approvals* in the MCP item's popover until it is `on`. The switch can
/// be flipped once the setting is read; until then, and while it saves, it is fixed.
pub(crate) fn let_automation_answer(desktop: &mut Desktop, on: bool) {
  if !has(desktop, "status-mcp-answer-approvals") {
    desktop.click("status-mcp");
  }
  desktop.wait_for("status-mcp-answer-approvals");
  desktop.wait_until("the setting is saved", |desktop| {
    if allowed(desktop) == on {
      return true;
    }
    desktop.click("status-mcp-answer-approvals");
    false
  });
}

#[test]
fn app_mcp_popover_lets_automation_answer_and_takes_it_back() {
  let mut desktop = Desktop::with_seeded_project_and_app_mcp(|_| {});
  assert!(!allowed(&desktop), "off until a person turns it on");
  let_automation_answer(&mut desktop, true);
  // The item keeps its `$hover` fill while its popover is open, the pointer elsewhere.
  desktop.hover("nav-tasks");
  let hover = desktop.color(PaletteColor::AccentMuted);
  assert_eq!(desktop.fill_at("status-mcp"), Some(hover));
  let_automation_answer(&mut desktop, false);
  desktop.click("status-mcp");
  desktop.wait_until("the popover closes", |desktop| !has(desktop, "status-mcp-popover"));
  desktop.hover("nav-tasks");
  assert_ne!(desktop.fill_at("status-mcp"), Some(hover));
}
