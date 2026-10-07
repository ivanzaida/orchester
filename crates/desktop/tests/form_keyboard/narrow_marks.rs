//! Transcript rows with long decision marks as the chat narrows, from activity seeded as the runner records
//! it: in Tools and in the step's open chip, at 960 with Details closed and open, a row keeps a command's
//! width of title, and its meta and mark each show whole or drop out whole, never cut in a word or past the
//! row.

use lurq::core::ElementRect;
use orchester_core::{
  connection::stream::{AgentEvent, CommandStatus},
  permission::category::CommandCategory,
  run::{DecidedBy, ToolDecision},
  store::ActivityStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::{
  conversation::open_group,
  harness::Desktop,
  narrow_answers::{details, inside, open_conversation},
  native_run::native_run,
  task_run::latest_run,
};

/// What a row's title keeps beside a long mark (`activity::header`'s `TITLE_MIN`).
const TITLE_MIN: f32 = 120.0;

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
      waited_ms: None,
      review: None,
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

/// The meta's and the mark's ids of row `sequence`.
fn parts(sequence: u64) -> [String; 2] {
  [
    format!("activity-entry-meta-{sequence}"),
    format!("activity-entry-mark-{sequence}"),
  ]
}

/// The natural widths of the meta and the mark of each of `rows`, where the line holds them all.
fn natural(desktop: &mut Desktop, rows: [u64; 3]) -> Vec<[f32; 2]> {
  rows
    .iter()
    .map(|sequence| parts(*sequence).map(|id| desktop.bounds(&id).width))
    .collect()
}

/// Each of `rows` keeps a command's width of title inside the chat, and its meta and mark show whole
/// (as wide as in `whole`) or drop out (no width), with Details closed and open; `view` names the view.
fn assert_rows(desktop: &mut Desktop, (rows, whole): ([u64; 3], &[[f32; 2]]), view: &str) {
  for open in [false, true] {
    details(desktop, open);
    desktop.pass();
    let chat = desktop.bounds("conversation-main");
    for (index, sequence) in rows.into_iter().enumerate() {
      let header = desktop.bounds(&format!("activity-entry-header-{sequence}"));
      let title = desktop.bounds(&format!("activity-entry-title-{sequence}"));
      assert!(
        title.width >= TITLE_MIN - 0.5,
        "{view}: row {sequence} keeps its title, Details open: {open} ({title:?})"
      );
      assert!(inside(header, chat), "{view}: row {sequence} stays in the chat");
      for (part, id) in parts(sequence).iter().enumerate() {
        let shown: ElementRect = desktop.bounds(id);
        let dropped = shown.width < 0.5;
        let complete = shown.width >= whole[index][part] - 0.5 && inside(shown, header);
        assert!(
          dropped || complete,
          "{view}: {id} is whole or gone, Details open: {open} ({shown:?}, whole {})",
          whole[index][part]
        );
        assert!(
          dropped || shown.x >= title.x + title.width,
          "{view}: {id} follows the title"
        );
      }
    }
  }
}

#[test]
fn narrow_marks_show_whole_or_drop_beside_a_kept_title_at_960() {
  let mut first = None;
  let mut desktop = Desktop::with_seeded_project(|repo| {
    native_run(repo);
    first = Some(decided_rows(repo));
  });
  let first = first.expect("seeded");
  // The rows of the command the profile ran, the tool automation approved and the denied push.
  let rows = [first, first + 3, first + 4];
  desktop.tree.resize(1440, 960);
  open_conversation(&mut desktop);
  desktop.wait_for("conversation-show-2");
  desktop.click("conversation-show-2");
  desktop.wait_for(&format!("activity-entry-header-{}", rows[2]));
  details(&mut desktop, false);
  let whole = natural(&mut desktop, rows);
  assert!(whole.iter().all(|[_, mark]| *mark > 0.0), "every mark shows at 1440");
  desktop.tree.resize(960, 960);
  assert_rows(&mut desktop, (rows, &whole), "Tools");
  // The same rows in the chip of the step's tool group, opened in place.
  details(&mut desktop, false);
  desktop.click("conversation-show-0");
  open_group(&mut desktop, "");
  desktop.wait_for(&format!("activity-entry-header-{}", rows[2]));
  assert_rows(&mut desktop, (rows, &whole), "chip");
}
