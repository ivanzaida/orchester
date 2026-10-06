//! The scope of *Approve similar in this run* in the approval dialog: hovering or focusing the tile
//! opens it under the request, with the header and the scope in view, and moves none of the answers,
//! at 1440 and 960 wide, in a window tall enough for the dialog to grow and with a command long
//! enough to fill the details.

use lurq::core::ElementRect;
use orchester_core::run::PendingToolApproval;
use orchester_store::ProjectDb;

use crate::{harness::Desktop, native_run::native_run, task_run::has, unreadable_run::at};

const DIALOG: &str = "tool-approval-dialog";
const DETAILS: &str = "tool-approval-details-scroll";
const SIMILAR: &str = "tool-approval-similar";
const SCOPE: &str = "tool-approval-similar-scope";

/// The answers, whose place hovering or focusing one of them must not change.
const ANSWERS: [&str; 7] = [
  "tool-approval-wider",
  SIMILAR,
  "tool-approval-always",
  "tool-approval-approve-run",
  "tool-approval-decide-later",
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

fn same(now: &ElementRect, was: &ElementRect) -> bool {
  (now.x - was.x).abs() < 0.5
    && (now.y - was.y).abs() < 0.5
    && (now.width - was.width).abs() < 0.5
    && (now.height - was.height).abs() < 0.5
}

fn answers(desktop: &mut Desktop) -> Vec<ElementRect> {
  ANSWERS.iter().map(|id| desktop.bounds(id)).collect()
}

fn assert_answers_unmoved(desktop: &mut Desktop, before: &[ElementRect], when: &str) {
  for (id, (now, was)) in ANSWERS.iter().zip(answers(desktop).iter().zip(before)) {
    assert!(same(now, was), "{id} keeps its place {when}: {was:?} became {now:?}");
  }
}

/// The scope is shown whole in the details, under the header and above the answers, with the dialog
/// inside the window.
fn assert_scope_in_view(desktop: &mut Desktop, when: &str) {
  assert!(has(desktop, SCOPE), "the scope is shown {when}");
  let scope = desktop.bounds(SCOPE);
  let details = desktop.bounds(DETAILS);
  let dialog = desktop.bounds(DIALOG);
  assert!(dialog.y >= 0.0, "the dialog starts in the window {when}: {dialog:?}");
  assert!(
    scope.y >= details.y - 0.5 && scope.y + scope.height <= details.y + details.height + 0.5,
    "the scope is in view in the details, under the header {when}: {scope:?} in {details:?}"
  );
  assert!(
    scope.x > details.x + 0.5 && scope.x + scope.width < details.x + details.width,
    "the scope is inset like the details' content {when}: {scope:?} in {details:?}"
  );
}

fn assert_hover_and_focus_move_no_answer(mut desktop: Desktop, (width, height): (u32, u32)) {
  desktop.tree.resize(width, height);
  desktop.pass();
  assert!(!has(&mut desktop, SCOPE), "the scope is closed at first");
  let before = answers(&mut desktop);
  let closed = desktop.bounds(DIALOG);
  let when = format!("at {width}x{height} under the pointer");
  desktop.hover(SIMILAR);
  desktop.pass();
  assert_scope_in_view(&mut desktop, &when);
  assert_answers_unmoved(&mut desktop, &before, &when);
  // The pointer stays put: the tile under it stays hovered, and its scope open.
  desktop.pass();
  assert!(has(&mut desktop, SCOPE), "the scope stays open {when}");
  assert_answers_unmoved(&mut desktop, &before, &when);
  desktop.hover("tool-approval-deny");
  desktop.pass();
  assert!(!has(&mut desktop, SCOPE), "the scope closes when the pointer leaves");
  let now = desktop.bounds(DIALOG);
  assert!(
    same(&now, &closed),
    "the dialog is as before {when}: {closed:?} became {now:?}"
  );
  let when = format!("at {width}x{height} with the focus");
  desktop.tab_to(SIMILAR, 12);
  desktop.pass();
  assert_scope_in_view(&mut desktop, &when);
  assert_answers_unmoved(&mut desktop, &before, &when);
}

#[test]
fn tool_approval_scope_hover_and_focus_move_no_answer() {
  for size in [(1440, 960), (960, 960), (1440, 1400)] {
    assert_hover_and_focus_move_no_answer(dialog(None), size);
    assert_hover_and_focus_move_no_answer(dialog(Some(LONG_COMMAND)), size);
  }
}
