//! Auto mode in the desktop, from runs and requests seeded into the project database: the run's switch in
//! its Approvals panel, fixed with its explanation and the reason while the project has no reviewer; Project settings'
//! *New runs use auto mode* and reviewer (any agent connection, a model of it and its effort, Claude Code with Haiku
//! 4.5 until a person chooses); why auto mode left a request to the person, in the approval dialog, the reviewer's
//! reason quoted on one line apart from the app's words; the activity's marks, whole at 960, with the reason on a line
//! of its own; and the run's switch and its settings through Orchester's MCP server, changed only while automation may
//! answer approvals. `cargo test -p orchester-desktop --test form_keyboard auto_mode`.

use std::path::Path;

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
    stream::{AgentEvent, CommandStatus},
  },
  id::ConnectionId,
  name::Name,
  permission::category::CommandCategory,
  resource::ModelId,
  run::{
    DecidedBy, PendingToolApproval, ToolDecision,
    review::{AutoModeSettings, Review, ReviewHold, ReviewerChoice},
  },
  store::{ActivityStore, ConnectionStore},
  time::Timestamp,
};
use orchester_store::{AppDb, ProjectDb};
use serde_json::json;

use crate::{
  app_mcp::{agent, ok, refused},
  approval_choices::open_task,
  github::shows,
  harness::Desktop,
  narrow_answers::{details, inside, open_conversation},
  native_dialog::choose,
  native_run::native_run,
  task_run::{latest_run, seed_claude},
  unreadable_run::at,
};

pub(crate) const SWITCH: &str = "task-auto-mode";

/// A seeded run with a waiting MCP request that auto mode left to the person with `review`, in a project
/// with a Claude Code connection when `reviewer`; the run's id.
pub(crate) fn seeded(reviewer: bool, review: Option<&Review>) -> (Desktop, orchester_core::id::RunId) {
  seeded_with((reviewer, review), |_| {})
}

/// [`seeded`], with what `more` adds to the project.
fn seeded_with(
  (reviewer, review): (bool, Option<&Review>),
  more: impl FnOnce(&Path),
) -> (Desktop, orchester_core::id::RunId) {
  let mut seeded = None;
  let desktop = Desktop::with_seeded_project_and_app_mcp(|repo| {
    more(repo);
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
pub(crate) fn claude_connection(repo: &Path) {
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

pub(crate) fn run_auto_mode(repo: &Path, run: orchester_core::id::RunId) -> bool {
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
    "tool-approval-review-label",
    "Auto mode asked you:"
  ));
  assert!(shows(
    &mut desktop,
    "tool-approval-review-reason",
    "\u{201c}Redesigning the switcher is not part of the task.\u{201d}"
  ));
  let label = desktop.bounds("tool-approval-review-label");
  let reason = desktop.bounds("tool-approval-review-reason");
  assert!((label.y - reason.y).abs() < 4.0, "the reason is on the label's line");

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
    "Auto mode needs an agent connection"
  ));
  assert!(
    shows(&mut desktop, "task-auto-mode", "A separate reviewer model"),
    "a locked switch keeps its explanation"
  );
  let switch = desktop.bounds("task-auto-mode");
  assert!(
    inside(desktop.bounds("task-auto-mode-unavailable"), switch),
    "the reason is inside the switch's row"
  );
}

/// A signed-in Codex connection named `Codex · Review` listing `gpt-5.5-codex` with two efforts.
pub(crate) fn codex_connection(repo: &Path) -> ConnectionId {
  let listed = CatalogModel::new(
    ModelId::new("gpt-5.5-codex").expect("model"),
    vec!["low".to_owned(), "high".to_owned()],
  );
  codex_named(repo, "Codex · Review", vec![listed])
}

/// A signed-in Codex connection named `name` listing `listed`.
fn codex_named(repo: &Path, name: &str, listed: Vec<CatalogModel>) -> ConnectionId {
  let now = Timestamp::now();
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new(name).expect("name"),
    AgentProvider::CodexCli,
    repo.join("orchester-fake-codex"),
    now,
  )
  .expect("absolute path");
  connection
    .record_check(ConnectionCheck {
      at: now,
      result: CheckResult::Available {
        version: "0.159.2".into(),
        sign_in: SignIn::SignedIn,
        catalog: ModelCatalog::new(CatalogSource::Provider, listed),
      },
    })
    .expect("an agent check fits a CLI connection");
  let id = connection.id();
  ProjectDb::open(repo)
    .expect("open")
    .create_connection(&connection.into_connection())
    .expect("connection");
  id
}

pub(crate) fn settings(repo: &Path) -> AutoModeSettings {
  ProjectDb::open(repo).expect("open").auto_mode_settings().expect("read")
}

#[test]
fn auto_mode_project_setting_turns_new_runs_on_and_chooses_the_reviewer() {
  let mut codex = None;
  let mut desktop = Desktop::with_seeded_project(|repo| {
    seed_claude(repo, &repo.join("orchester-fake-codex"));
    codex = Some(codex_connection(repo));
  });
  let codex = codex.expect("seeded");
  desktop.wait_for("nav-settings");
  desktop.click("nav-settings");
  desktop.wait_for("settings-auto-mode-new-runs");
  // Until a person chooses, the first Claude Code connection reviews with Haiku 4.5.
  assert!(shows(
    &mut desktop,
    "settings-auto-mode-reviewer",
    "Reviewed by claude-haiku-4-5-20251001 on Claude · Kontur."
  ));
  let repo = desktop.repo().to_path_buf();
  assert!(!settings(&repo).new_runs, "off until a person turns it on");
  // One click: the switch shows the change at once, and a second click would undo it.
  desktop.click("settings-auto-mode-new-runs");
  desktop.wait_until("new runs use auto mode", |_| settings(&repo).new_runs);

  choose(&mut desktop, "settings-auto-mode-connection", "Codex · Review");
  desktop.wait_until("Codex reviews", |_| settings(&repo).reviewer.is_some());
  let chosen = ReviewerChoice {
    connection: codex,
    model: ModelId::new("gpt-5.5-codex").expect("model"),
    effort: None,
  };
  assert_eq!(
    settings(&repo).reviewer,
    Some(chosen.clone()),
    "the connection's default model"
  );
  desktop.wait_for("settings-auto-mode-effort");
  choose(&mut desktop, "settings-auto-mode-effort", "high");
  desktop.wait_until("at high effort", |_| {
    settings(&repo).reviewer.and_then(|choice| choice.effort).is_some()
  });
  assert_eq!(
    settings(&repo).reviewer,
    Some(ReviewerChoice {
      effort: Some("high".to_owned()),
      ..chosen
    })
  );
  desktop.wait_until("the readback names the choice", |desktop| {
    shows(
      desktop,
      "settings-auto-mode-reviewer",
      "Reviewed by gpt-5.5-codex (high effort) on Codex · Review.",
    )
  });
  for label in ["Project connection", "Model", "Reasoning effort"] {
    assert!(shows(&mut desktop, "settings-auto-mode-section", label), "{label}");
  }
  choose(&mut desktop, "settings-auto-mode-connection", "Default");
  desktop.wait_until("back to the default reviewer", |_| settings(&repo).reviewer.is_none());
  assert!(settings(&repo).new_runs);

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

#[test]
fn auto_mode_settings_through_mcp_read_always_and_change_only_while_automation_may_answer() {
  let mut codex = None;
  let asked = Review::Asked {
    reason: "Not needed for the task.".to_owned(),
    model: "claude-haiku-4-5-20251001".to_owned(),
  };
  let (desktop, _) = seeded_with((true, Some(&asked)), |repo| codex = Some(codex_connection(repo)));
  let codex = codex.expect("seeded");
  let mut client = agent(&desktop);
  let project = json!({"project": "Kontur"});
  // The run records which model reviewed each request.
  let waiting = ok(&mut client, "list_approvals", &project);
  assert_eq!(
    waiting["approvals"][0]["auto_mode"],
    json!({"outcome": "asked", "reason": "Not needed for the task.", "model": "claude-haiku-4-5-20251001"})
  );
  let read = ok(&mut client, "get_auto_mode", &project);
  assert_eq!(
    read["auto_mode"],
    json!({"new_runs": false, "reviewer": {"connection": "Claude · Kontur", "model": "claude-haiku-4-5-20251001",
      "effort": null, "default": true}})
  );
  let change = json!({"project": "Kontur", "new_runs": true,
    "reviewer": {"connection": "Codex · Review", "effort": "high"}});
  assert_eq!(
    refused(&mut client, "set_auto_mode", &change)["code"],
    "automation_not_allowed"
  );
  assert_eq!(settings(desktop.repo()), AutoModeSettings::default());
  AppDb::open(&desktop.repo().parent().expect("the test folder").join("orchester.db"))
    .expect("open")
    .set_automation_answers_approvals(true)
    .expect("a person lets automation answer");
  let changed = ok(&mut client, "set_auto_mode", &change);
  assert_eq!(
    changed["auto_mode"],
    json!({"new_runs": true, "reviewer": {"connection": "Codex · Review", "model": "gpt-5.5-codex",
      "effort": "high", "default": false}})
  );
  assert_eq!(
    settings(desktop.repo()).reviewer,
    Some(ReviewerChoice {
      connection: codex,
      model: ModelId::new("gpt-5.5-codex").expect("model"),
      effort: Some("high".to_owned()),
    })
  );
  let odd = json!({"project": "Kontur", "reviewer": {"connection": "Codex · Review", "effort": "max"}});
  assert_eq!(refused(&mut client, "set_auto_mode", &odd)["code"], "invalid_arguments");
  let back = ok(
    &mut client,
    "set_auto_mode",
    &json!({"project": "Kontur", "reviewer": {}}),
  );
  assert_eq!(back["auto_mode"]["reviewer"]["default"], true);
}

/// A command the person approved after auto mode asked, and a pencil tool auto mode approved, with a long
/// reason, as the runner records them; the first entry's sequence.
fn reviewed_rows(repo: &Path) -> u64 {
  let run = latest_run(repo).expect("the seeded run");
  let mut db = ProjectDb::open(repo).expect("open the project");
  let invocation = db.invocations(run.id()).expect("invocations").remove(0);
  let model = "claude-haiku-4-5-20251001".to_owned();
  let decided = |item: &str, server: &str, action: &str, (decided_by, review)| AgentEvent::ToolDecided {
    item: item.into(),
    server: server.into(),
    action: action.into(),
    decision: ToolDecision {
      decided_by,
      category: (server == "command").then_some(CommandCategory::Worktree),
      secrets: false,
      told: None,
      waited_ms: None,
      review: Some(review),
    },
  };
  let asked = Review::Asked {
    reason: "Building is not needed to write the release notes.".to_owned(),
    model: model.clone(),
  };
  let approved = Review::Approved {
    reason: "Reading a screenshot of the switcher frame serves the task and changes nothing, so it is safe to \
             approve without the person; the frame belongs to the design the task names."
      .to_owned(),
    model,
  };
  let events = vec![
    AgentEvent::CommandStarted {
      item: "cmd_build".into(),
      command: "cargo build".into(),
    },
    decided("cmd_build", "command", "cargo build", (DecidedBy::Person, asked)),
    AgentEvent::CommandFinished {
      item: "cmd_build".into(),
      command: "cargo build".into(),
      output: String::new(),
      exit_code: Some(0),
      status: CommandStatus::Completed,
    },
    decided("mcp_shot", "pencil", "get_screenshot", (DecidedBy::Reviewer, approved)),
  ];
  let count = events.len() as u64;
  let events: Vec<_> = events
    .into_iter()
    .enumerate()
    .map(|(i, event)| (Timestamp::from_unix_millis(30_000 + i as i64 * 1_000), event))
    .collect();
  db.append_activity(&invocation, &events).expect("store the events") - count + 1
}

#[test]
fn auto_mode_activity_keeps_the_mark_and_quotes_the_reason_on_its_own_line() {
  let mut first = None;
  let mut desktop = Desktop::with_seeded_project(|repo| {
    native_run(repo);
    first = Some(reviewed_rows(repo));
  });
  let first = first.expect("seeded");
  desktop.tree.resize(960, 960);
  open_conversation(&mut desktop);
  desktop.wait_for("conversation-show-2");
  desktop.click("conversation-show-2");
  let (command, tool) = (first, first + 3);
  desktop.wait_for(&format!("activity-entry-header-{tool}"));
  for open in [false, true] {
    details(&mut desktop, open);
    desktop.pass();
    let chat = desktop.bounds("conversation-main");
    for (sequence, mark, label) in [
      (command, "approved by you", "Auto mode asked you:"),
      (tool, "approved by auto mode", "Auto mode:"),
    ] {
      // With Details open the line is as narrow as for any mark, which then drops whole.
      let mark_id = format!("activity-entry-mark-{sequence}");
      assert!(open || desktop.bounds(&mark_id).width > 0.5, "{mark_id} shows");
      assert!(shows(&mut desktop, &mark_id, mark));
      let review = format!("activity-entry-review-{sequence}");
      assert!(shows(&mut desktop, &format!("{review}-label"), label));
      assert!(inside(desktop.bounds(&review), chat), "{review} stays in the chat");
      let header = desktop.bounds(&format!("activity-entry-header-{sequence}"));
      assert!(
        desktop.bounds(&review).y >= header.y + header.height - 0.5,
        "{review} is under the row"
      );
    }
    let reason = desktop.bounds(&format!("activity-entry-review-{tool}-reason"));
    let label = desktop.bounds(&format!("activity-entry-review-{tool}-label"));
    assert!(
      (reason.y - label.y).abs() < 4.0,
      "the reason starts on the label's line"
    );
    assert!(
      shows(
        &mut desktop,
        &format!("activity-entry-review-{tool}-reason"),
        "the frame belongs to the design the task names."
      ),
      "the reason is shown whole"
    );
    assert!(reason.height > label.height * 1.5, "and wraps");
  }
}

#[test]
fn auto_mode_offers_a_connection_that_cannot_review_disabled_with_its_reason() {
  let mut desktop = Desktop::with_seeded_project(|repo| {
    seed_claude(repo, &repo.join("orchester-fake-codex"));
    codex_named(repo, "Codex · Unlisted", Vec::new());
  });
  desktop.wait_for("nav-settings");
  desktop.click("nav-settings");
  desktop.wait_for("settings-auto-mode-connection");
  let repo = desktop.repo().to_path_buf();
  {
    let select = desktop
      .tree
      .get_element_by_id_mut("settings-auto-mode-connection")
      .and_then(|element| element.as_select())
      .expect("the connection select");
    let labels = select.labels();
    assert!(
      labels.iter().any(|label| label.contains("Codex · Unlisted")),
      "{labels:?}"
    );
  }
  // It cannot be chosen: nothing is saved, and the select keeps the saved reviewer.
  choose(&mut desktop, "settings-auto-mode-connection", "Codex · Unlisted");
  for _ in 0..5 {
    desktop.pass();
  }
  assert_eq!(settings(&repo), AutoModeSettings::default());
  let selected = desktop
    .tree
    .get_element_by_id_mut("settings-auto-mode-connection")
    .and_then(|element| element.as_select())
    .expect("the connection select")
    .selected_labels();
  assert_eq!(selected.len(), 1);
  assert!(selected[0].starts_with("Default"), "{selected:?}");
  assert!(shows(
    &mut desktop,
    "settings-auto-mode-reviewer",
    "Reviewed by claude-haiku-4-5-20251001 on Claude · Kontur."
  ));
}
