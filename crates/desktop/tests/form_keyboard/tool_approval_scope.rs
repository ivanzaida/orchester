//! The scope of *Approve similar in this run* in the approval dialog: hovering or focusing the tile
//! shows it above the wider answers and moves nothing in the dialog, at 1440 and 960 wide and with a
//! command long enough to fill the details.

use lurq::core::ElementRect;
use orchester_core::run::PendingToolApproval;
use orchester_store::ProjectDb;

use crate::{harness::Desktop, native_run::native_run, task_run::has, unreadable_run::at};

const DIALOG: &str = "tool-approval-dialog";
const SIMILAR: &str = "tool-approval-similar";
const SCOPE: &str = "tool-approval-similar-scope";

/// What the dialog is made of, whose place hovering or focusing an answer must not change.
const LAID_OUT: [&str; 9] = [
  DIALOG,
  "tool-approval-details-scroll",
  "tool-approval-details",
  "tool-approval-wider",
  SIMILAR,
  "tool-approval-always",
  "tool-approval-approve-run",
  "tool-approval-deny",
  "tool-approval-approve-once",
];

/// A `cargo test` with every filter spelled out, which Approve similar covers.
const LONG_COMMAND: &str = "cargo test -p orchester-desktop --test form_keyboard tool_approval -- --nocapture \
                            --test-threads=1 --skip tool_approval_copy_takes_each_value_as_recorded --skip \
                            tool_approval_runner_offline_keeps_both_buttons_in_the_dialog --skip \
                            tool_approval_native_shell_script_and_long_folder_keep_the_buttons_in_view --skip \
                            tool_approval_large_arguments_read_one_member_a_line_and_scroll";

/// The dialog of one request of a seeded run: an MCP call, or the command `action`.
fn dialog(command: Option<&'static str>) -> Desktop {
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let (run, invocation) = native_run(repo);
    let approval = PendingToolApproval {
      id: format!("{}:call_0", invocation.id),
      run: run.id(),
      execution: invocation.execution,
      invocation: invocation.id,
      agent: "Builder".to_owned(),
      command: command.is_some(),
      scope: None,
      server: if command.is_some() { "command" } else { "pencil" }.to_owned(),
      action: command.unwrap_or("batch_design").to_owned(),
      target: None,
      requested_at: at(20),
      native: None,
    };
    ProjectDb::open(repo)
      .expect("open the project")
      .request_tool_approval(&approval)
      .expect("pending approval");
  });
  desktop.wait_for(DIALOG);
  desktop
}

fn layout(desktop: &mut Desktop) -> Vec<ElementRect> {
  LAID_OUT.iter().map(|id| desktop.bounds(id)).collect()
}

fn assert_unmoved(desktop: &mut Desktop, before: &[ElementRect], when: &str) {
  for (id, (now, was)) in LAID_OUT.iter().zip(layout(desktop).iter().zip(before)) {
    assert!(
      (now.x - was.x).abs() < 0.5
        && (now.y - was.y).abs() < 0.5
        && (now.width - was.width).abs() < 0.5
        && (now.height - was.height).abs() < 0.5,
      "{id} keeps its place {when}: {was:?} became {now:?}"
    );
  }
}

/// The scope is shown, inside the window and above the wider answers, so it covers none of them.
fn assert_scope_above_the_answers(desktop: &mut Desktop, when: &str) {
  assert!(has(desktop, SCOPE), "the scope is shown {when}");
  let scope = desktop.bounds(SCOPE);
  let wider = desktop.bounds("tool-approval-wider");
  let details = desktop.bounds("tool-approval-details");
  assert!(scope.y >= 0.0, "the scope starts in the window {when}: {scope:?}");
  assert!(
    scope.y + scope.height <= wider.y + 0.5,
    "the scope ends above the wider answers {when}: {scope:?} over {wider:?}"
  );
  assert!(
    (scope.x - details.x).abs() > 0.5 && scope.x + scope.width < details.x + details.width,
    "the scope is inset like the details' content {when}: {scope:?} in {details:?}"
  );
}

fn assert_hover_and_focus_move_nothing(mut desktop: Desktop, width: u32) {
  desktop.tree.resize(width, 960);
  desktop.pass();
  assert!(!has(&mut desktop, SCOPE), "the scope is closed at first");
  let before = layout(&mut desktop);
  let when = format!("at {width} under the pointer");
  desktop.hover(SIMILAR);
  desktop.pass();
  assert_scope_above_the_answers(&mut desktop, &when);
  assert_unmoved(&mut desktop, &before, &when);
  // The pointer stays put: the tile under it stays hovered, and its scope open.
  desktop.pass();
  assert!(has(&mut desktop, SCOPE), "the scope stays open {when}");
  assert_unmoved(&mut desktop, &before, &when);
  desktop.hover("tool-approval-deny");
  desktop.pass();
  assert!(!has(&mut desktop, SCOPE), "the scope closes when the pointer leaves");
  let when = format!("at {width} with the focus");
  desktop.tab_to(SIMILAR, 12);
  desktop.pass();
  assert_scope_above_the_answers(&mut desktop, &when);
  assert_unmoved(&mut desktop, &before, &when);
}

#[test]
fn tool_approval_scope_hover_and_focus_move_nothing() {
  for width in [1440, 960] {
    assert_hover_and_focus_move_nothing(dialog(None), width);
    assert_hover_and_focus_move_nothing(dialog(Some(LONG_COMMAND)), width);
  }
}
