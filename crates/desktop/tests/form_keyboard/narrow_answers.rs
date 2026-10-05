//! Answers at the supported minimum width, 960, with the Conversation tab's Details closed and open: an
//! inline approval card keeps every answer inside it (its *Always allow…* gives way first, then the
//! buttons wrap), and a Tools row with a long decision mark keeps its title at least a command's width,
//! its mark ending in an ellipsis or dropping out, never past the chat. From requests and activity seeded
//! as the runner records them.

use lurq::core::ElementRect;
use orchester_core::{
  connection::stream::{AgentEvent, CommandStatus},
  permission::category::CommandCategory,
  run::{DecidedBy, PendingToolApproval, ToolDecision},
  store::ActivityStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::{
  conversation::open_group,
  harness::Desktop,
  native_run::native_run,
  task_run::{has, latest_run},
  unreadable_run::at,
};

const DIALOG: &str = "tool-approval-dialog";
/// What a Tools row's title keeps beside a long mark (`activity::header`'s `TITLE_MIN`).
const TITLE_MIN: f32 = 120.0;

fn open_conversation(desktop: &mut Desktop) {
  desktop.wait_for("nav-tasks");
  desktop.press("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  desktop.click("task-row-KON-1");
  desktop.wait_for("task-page");
  desktop.click("task-tab-conversation");
}

/// Whether `inner` lies within `outer`, to half a pixel.
fn inside(inner: ElementRect, outer: ElementRect) -> bool {
  inner.x >= outer.x - 0.5
    && inner.y >= outer.y - 0.5
    && inner.x + inner.width <= outer.x + outer.width + 0.5
    && inner.y + inner.height <= outer.y + outer.height + 0.5
}

/// Opens or closes the Conversation tab's Details until `open`.
fn details(desktop: &mut Desktop, open: bool) {
  if has(desktop, "conversation-details-head") != open {
    desktop.click("conversation-details");
  }
  desktop.wait_until("Details", |desktop| has(desktop, "conversation-details-head") == open);
}

#[test]
fn narrow_answers_card_keeps_approve_once_inside_at_960() {
  let mut seeded = None;
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let (run, invocation) = native_run(repo);
    let request = PendingToolApproval {
      id: format!("{}:call_commit", invocation.id),
      run: run.id(),
      execution: invocation.execution,
      invocation: invocation.id,
      agent: "Builder".to_owned(),
      command: false,
      scope: Some(r#"{"message":"Add release notes for 0.4.0"}"#.to_owned()),
      server: "Orchester".to_owned(),
      action: "git_commit".to_owned(),
      target: None,
      requested_at: at(20),
      native: None,
    };
    ProjectDb::open(repo)
      .expect("open the project")
      .request_tool_approval(&request)
      .expect("pending approval");
    seeded = Some(request.id);
  });
  let card = format!("conversation-approval-{}", seeded.expect("seeded"));
  desktop.tree.resize(960, 960);
  desktop.wait_for(DIALOG);
  desktop.press("tool-approval-decide-later");
  desktop.wait_until("the dialog closes", |desktop| !has(desktop, DIALOG));
  open_conversation(&mut desktop);
  desktop.wait_for("conversation-approval-approve");
  for open in [false, true] {
    details(&mut desktop, open);
    desktop.pass();
    let frame = desktop.bounds(&card);
    for answer in [
      "conversation-approval-always",
      "conversation-approval-deny",
      "conversation-approval-similar",
      "conversation-approval-approve",
    ] {
      let bounds = desktop.bounds(answer);
      assert!(
        inside(bounds, frame),
        "{answer} lies inside the card with Details open: {open} ({bounds:?} in {frame:?})"
      );
    }
  }
}

/// A command the profile answered, an Orchester tool automation approved, and a push automation
/// denied, as the runner marks them; the first entry's sequence.
fn decided_rows(repo: &std::path::Path) -> u64 {
  let run = latest_run(repo).expect("the seeded run");
  let mut db = ProjectDb::open(repo).expect("open the project");
  let invocation = db.invocations(run.id()).expect("invocations").remove(0);
  let decided = |item: &str, server: &str, action: &str, decided_by| AgentEvent::ToolDecided {
    item: item.into(),
    server: server.into(),
    action: action.into(),
    decision: ToolDecision {
      decided_by,
      category: (server == "command").then_some(CommandCategory::Worktree),
      secrets: false,
      told: (decided_by == DecidedBy::AutomationDeny)
        .then(|| "Denied by automation through Orchester's MCP server. Continue without it.".to_owned()),
    },
  };
  let push = "git push origin orc/release-notes";
  let events = vec![
    AgentEvent::CommandStarted {
      item: "cmd_fmt".into(),
      command: "cargo fmt --all".into(),
    },
    decided("cmd_fmt", "command", "cargo fmt --all", DecidedBy::Permissions),
    AgentEvent::CommandFinished {
      item: "cmd_fmt".into(),
      command: "cargo fmt --all".into(),
      output: String::new(),
      exit_code: Some(0),
      status: CommandStatus::Completed,
    },
    decided("mcp_add", "Orchester", "git_add", DecidedBy::Automation),
    AgentEvent::CommandStarted {
      item: "cmd_push".into(),
      command: push.into(),
    },
    decided("cmd_push", "command", push, DecidedBy::AutomationDeny),
  ];
  let count = events.len() as u64;
  let events: Vec<_> = events
    .into_iter()
    .enumerate()
    .map(|(i, event)| (Timestamp::from_unix_millis(30_000 + i as i64 * 1_000), event))
    .collect();
  db.append_activity(&invocation, &events).expect("store the events") - count + 1
}

/// Each of `rows` keeps a command's width of title and its mark inside the chat, with Details closed and
/// open; `view` names the view for the failure.
fn assert_rows(desktop: &mut Desktop, rows: [u64; 3], view: &str) {
  for open in [false, true] {
    details(desktop, open);
    desktop.pass();
    let chat = desktop.bounds("conversation-main");
    for sequence in rows {
      let header = desktop.bounds(&format!("activity-entry-header-{sequence}"));
      let title = desktop.bounds(&format!("activity-entry-title-{sequence}"));
      let meta = desktop.bounds(&format!("activity-entry-meta-{sequence}"));
      assert!(
        title.width >= TITLE_MIN - 0.5,
        "{view}: row {sequence} keeps its title with Details open: {open} ({title:?})"
      );
      assert!(
        inside(header, chat),
        "{view}: row {sequence} stays in the chat ({header:?} in {chat:?})"
      );
      assert!(
        title.x + title.width <= meta.x + 0.5,
        "{view}: the mark follows the title"
      );
      assert!(
        meta.x + meta.width <= header.x + header.width + 0.5,
        "{view}: the mark ends in its row"
      );
    }
  }
}

#[test]
fn narrow_answers_tools_rows_keep_their_title_beside_long_marks_at_960() {
  let mut first = None;
  let mut desktop = Desktop::with_seeded_project(|repo| {
    native_run(repo);
    first = Some(decided_rows(repo));
  });
  let first = first.expect("seeded");
  // The rows of the command the profile ran, the tool automation approved and the denied push.
  let rows = [first, first + 3, first + 4];
  desktop.tree.resize(960, 960);
  open_conversation(&mut desktop);
  desktop.wait_for("conversation-show-2");
  desktop.click("conversation-show-2");
  desktop.wait_for(&format!("activity-entry-header-{}", rows[2]));
  assert_rows(&mut desktop, rows, "Tools");
  // The same rows in the chip of the step's tool group, opened in place.
  details(&mut desktop, false);
  desktop.click("conversation-show-0");
  open_group(&mut desktop, "");
  desktop.wait_for(&format!("activity-entry-header-{}", rows[2]));
  assert_rows(&mut desktop, rows, "chip");
}
