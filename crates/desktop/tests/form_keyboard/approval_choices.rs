//! The approval choices of 2026-10-02 in the desktop, from requests and runs seeded into the project
//! database as the runner records them: the dialog's layout (wider answers above a footer of Decide later,
//! Deny and Approve once, which has the focus; what Deny tells the agent), the keyboard (Tab in the visual
//! order, wrapping; Esc is Decide later), one request at a time with "1 of 3", Decide later leaving the
//! requests in Needs you and the status bar with Review, Needs you counting while the dialog is open, a
//! similar rule's Revoke, and auto-approval's switch, indicator and Stop. This harness has no runner, so its status
//! bar shows that problem in place of the waiting requests (`tests/status_bar/approvals.rs` shows them).

use lurq::{
  app::events::MouseButton,
  core::ElementRect,
  layout::quad::{Quad, QuadContent},
};
use orchester_core::{
  id::{InvocationId, RunId, StepExecutionId},
  resource::ModelId,
  run::{Invocation, PendingToolApproval, RuleScope, Run},
  store::{ActivityStore, RunStore},
};
use orchester_desktop::tasks::approvals::agent_worktree;
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::{Desktop, expected},
  native_run::native_run,
  task_run::has,
  unreadable_run::{at, started},
};

const DIALOG: &str = "tool-approval-dialog";

/// `count` MCP requests of a seeded run, the oldest first; the run's id.
fn waiting(count: usize) -> (Desktop, RunId) {
  let mut seeded = None;
  let desktop = Desktop::with_seeded_project(|repo| {
    let (run, invocation) = native_run(repo);
    let mut db = ProjectDb::open(repo).expect("open the project");
    for number in 0..count {
      let approval = PendingToolApproval {
        id: format!("{}:call_{number}", invocation.id),
        run: run.id(),
        execution: invocation.execution,
        invocation: invocation.id,
        agent: "Designer".to_owned(),
        command: false,
        scope: None,
        server: "pencil".to_owned(),
        action: format!("batch_design_{number}"),
        target: None,
        requested_at: at(20 + i64::try_from(number).expect("small")),
        native: None,
      };
      db.request_tool_approval(&approval).expect("pending approval");
    }
    seeded = Some(run.id());
  });
  (desktop, seeded.expect("seeded"))
}

fn open(count: usize) -> (Desktop, RunId) {
  let (mut desktop, run) = waiting(count);
  desktop.wait_for(DIALOG);
  (desktop, run)
}

#[test]
fn approval_choices_dialog_shows_the_wider_answers_and_what_deny_tells() {
  let (mut desktop, _) = open(1);
  for id in [
    "tool-approval-position",
    "tool-approval-similar",
    "tool-approval-always",
    "tool-approval-approve-run",
    "tool-approval-decide-later",
    "tool-approval-deny",
    "tool-approval-approve-once",
  ] {
    assert!(has(&mut desktop, id), "{id} is shown");
  }
  assert!(shows(&mut desktop, "tool-approval-position", "1 of 1 waiting"));
  assert!(shows(
    &mut desktop,
    "tool-approval-similar",
    "pencil batch_design_0, every call until Run 1 ends"
  ));
  assert!(shows(
    &mut desktop,
    "tool-approval-deny-tells",
    "The person reviewing this run denied pencil batch_design_0. Continue without it."
  ));
  // The server is not one of the project's, so Always allow says why it cannot list the tool.
  assert!(shows(&mut desktop, "tool-approval-always", "can no longer be read"));
  assert_eq!(desktop.focused().as_deref(), Some("tool-approval-approve-once"));
  // The wider answers sit between the request and the footer; Decide later is the footer's leftmost.
  let wider = desktop.bounds("tool-approval-wider");
  let footer = desktop.bounds("tool-approval-actions");
  assert!(wider.y + wider.height <= footer.y + 0.5);
  let later = desktop.bounds("tool-approval-decide-later");
  let deny = desktop.bounds("tool-approval-deny");
  let once = desktop.bounds("tool-approval-approve-once");
  assert!(later.x < deny.x && deny.x < once.x);
}

/// Orchester's own tools are no project server's: Approve similar covers the tool for the run, and Always allow
/// says that they have no list of tools that run without asking.
#[test]
fn approval_choices_orchester_tool_offers_similar_but_not_always() {
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let (run, invocation) = native_run(repo);
    let mut db = ProjectDb::open(repo).expect("open the project");
    let approval = PendingToolApproval {
      id: format!("{}:call_outside", invocation.id),
      run: run.id(),
      execution: invocation.execution,
      invocation: invocation.id,
      agent: "Builder".to_owned(),
      command: false,
      scope: Some(r#"{"path":"/deps/src/lib.rs"}"#.to_owned()),
      server: "Orchester".to_owned(),
      action: "read_outside_file".to_owned(),
      target: None,
      requested_at: at(20),
      native: None,
    };
    db.request_tool_approval(&approval).expect("pending approval");
  });
  desktop.wait_for(DIALOG);
  assert!(shows(
    &mut desktop,
    "tool-approval-similar",
    "Orchester read_outside_file, every call until Run 1 ends"
  ));
  assert!(shows(
    &mut desktop,
    "tool-approval-always",
    "Orchester’s own tools have no list of tools that run without asking yet"
  ));
}

#[test]
fn approval_choices_tab_follows_the_visual_order_and_wraps() {
  let (mut desktop, _) = open(2);
  desktop.tab_to("tool-approval-next", 12);
  assert_eq!(
    desktop.stops(6, false),
    expected(&[
      Some("tool-approval-similar"),
      Some("tool-approval-approve-run"),
      Some("tool-approval-decide-later"),
      Some("tool-approval-deny"),
      Some("tool-approval-approve-once"),
      Some("tool-approval-next"),
    ]),
    "‹ is unavailable on the first request, Always allow on a server the project lacks"
  );
}

#[test]
fn approval_choices_one_request_at_a_time_oldest_first() {
  let (mut desktop, _) = open(3);
  assert!(shows(&mut desktop, "tool-approval-position", "1 of 3 waiting"));
  assert!(shows(&mut desktop, DIALOG, "batch_design_0"));
  desktop.press("tool-approval-next");
  desktop.wait_until("the second request", |desktop| {
    shows(desktop, "tool-approval-position", "2 of 3 waiting")
  });
  assert!(shows(&mut desktop, DIALOG, "batch_design_1"));
  desktop.press("tool-approval-previous");
  desktop.wait_until("the first request again", |desktop| {
    shows(desktop, "tool-approval-position", "1 of 3 waiting")
  });
}

/// ‹ and › hold their chevron in the middle of their 26 square (`YLfbU`, `H4ip59`) whether they can be
/// pressed or not, under the pointer and with the focus, at 1x and at 1.5x. The chevron is a lucide glyph
/// whose ink is centred in its one-em box, so its text box centred in the button centres the ink.
#[test]
fn approval_choices_queue_chevrons_are_centred_in_their_buttons() {
  let (mut desktop, _) = open(3);
  // The design's 1440 × 960 window in physical pixels at each scale.
  for (scale, width, height) in [(1.0, 1440, 960), (1.5, 2160, 1440)] {
    desktop.tree.set_scale_factor(scale);
    desktop.tree.resize(width, height);
    desktop.pass();
    // The first request: ‹ cannot be pressed, › can.
    assert_chevrons_centred(&mut desktop, scale, "first request");
    pointer(&mut desktop, "tool-approval-next", scale, false);
    assert_chevrons_centred(&mut desktop, scale, "› under the pointer");
    desktop.tab_to("tool-approval-next", 12);
    assert_chevrons_centred(&mut desktop, scale, "› focused");
    pointer(&mut desktop, "tool-approval-next", scale, true);
    desktop.wait_until("the second request", |desktop| {
      shows(desktop, "tool-approval-position", "2 of 3 waiting")
    });
    // The middle request: both can be pressed.
    assert_chevrons_centred(&mut desktop, scale, "middle request");
    pointer(&mut desktop, "tool-approval-previous", scale, true);
    desktop.wait_until("the first request again", |desktop| {
      shows(desktop, "tool-approval-position", "1 of 3 waiting")
    });
  }
}

/// The pointer over the middle of `id`, and a press there if `press`. lurq takes the pointer in the
/// window's physical pixels, which `Desktop::hover` and `Desktop::press` assume are logical ones.
fn pointer(desktop: &mut Desktop, id: &str, scale: f32, press: bool) {
  let bounds = desktop.bounds(id);
  let (x, y) = (
    (bounds.x + bounds.width / 2.0) * scale,
    (bounds.y + bounds.height / 2.0) * scale,
  );
  desktop.tree.mouse_move(x, y);
  if press {
    desktop.tree.mouse_down(x, y, MouseButton::Left);
    desktop.tree.mouse_up(x, y, MouseButton::Left);
  }
  desktop.pass();
}

fn assert_chevrons_centred(desktop: &mut Desktop, scale: f32, state: &str) {
  for id in ["tool-approval-previous", "tool-approval-next"] {
    let button = desktop.bounds(id);
    let glyph = painted_text_in(desktop, &button).unwrap_or_else(|| panic!("{id} paints its chevron"));
    let what = format!("{id} at {scale}x, {state}: chevron {glyph:?} in {button:?}");
    assert_eq!(glyph.width, glyph.height, "{what}: the chevron's box is its square em");
    let centre = |rect: &ElementRect| (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
    let ((glyph_x, glyph_y), (button_x, button_y)) = (centre(&glyph), centre(&button));
    // Within a quarter of a physical pixel, so a rounding of the layout passes and an offset does not.
    let tolerance = 0.25 / scale;
    assert!((glyph_x - button_x).abs() <= tolerance, "{what}: centred across");
    assert!((glyph_y - button_y).abs() <= tolerance, "{what}: centred down");
  }
}

/// The box of the one text painted inside `bounds` in the last pass.
fn painted_text_in(desktop: &Desktop, bounds: &ElementRect) -> Option<ElementRect> {
  let inside = |x: f32, y: f32, width: f32, height: f32| {
    x >= bounds.x && y >= bounds.y && x + width <= bounds.x + bounds.width && y + height <= bounds.y + bounds.height
  };
  desktop
    .tree
    .painted_quads()
    .into_iter()
    .filter(|quad| matches!(quad.content, QuadContent::Text { .. }))
    .find(|quad| inside(quad.x, quad.y, quad.width, quad.height))
    .map(|quad| ElementRect {
      x: quad.x,
      y: quad.y,
      width: quad.width,
      height: quad.height,
      ..ElementRect::default()
    })
}

/// A focused ‹ or › shows its whole focus ring (`xURvJ`): the header row ends at › and is taller
/// than the buttons by less than the ring reaches, so it must not clip them.
#[test]
fn approval_choices_queue_buttons_show_their_whole_focus_ring() {
  let (mut desktop, _) = open(3);
  desktop.press("tool-approval-next");
  desktop.wait_until("the middle request", |desktop| {
    shows(desktop, "tool-approval-position", "2 of 3 waiting")
  });
  for id in ["tool-approval-previous", "tool-approval-next"] {
    desktop.tab_to(id, 12);
    let button = desktop.bounds(id);
    let rings: Vec<_> = desktop
      .tree
      .painted_quads()
      .into_iter()
      .filter_map(|quad| match &quad.content {
        QuadContent::BoxShadow(shadow) if !shadow.inset && same_box(&quad, &button) => {
          Some((shadow.outset(), quad.clip))
        }
        _ => None,
      })
      .collect();
    assert!(!rings.is_empty(), "{id} paints its focus ring");
    for (outset, clip) in rings {
      let (left, top) = (button.x - outset, button.y - outset);
      let (right, bottom) = (button.x + button.width + outset, button.y + button.height + outset);
      let whole = !clip.active
        || (clip.x <= left && clip.y <= top && clip.x + clip.width >= right && clip.y + clip.height >= bottom);
      let clipped = (clip.x, clip.y, clip.width, clip.height);
      assert!(
        whole,
        "{id}'s ring reaching {outset} past {button:?} is clipped to {clipped:?}"
      );
    }
  }
}

/// Whether `quad` is painted on `bounds`, to within a layout rounding.
fn same_box(quad: &Quad, bounds: &ElementRect) -> bool {
  let near = |a: f32, b: f32| (a - b).abs() <= 0.5;
  near(quad.x, bounds.x) && near(quad.y, bounds.y) && near(quad.width, bounds.width) && near(quad.height, bounds.height)
}

#[test]
fn approval_choices_needs_you_counts_requests_while_the_dialog_is_open() {
  let (mut desktop, _) = open(3);
  assert!(shows(&mut desktop, "status-needs-you", "3"));
}

#[test]
fn approval_choices_decide_later_leaves_the_requests_waiting_with_review() {
  let (mut desktop, _) = open(3);
  desktop.press("tool-approval-decide-later");
  desktop.wait_until("the dialog closes", |desktop| !has(desktop, DIALOG));
  assert!(shows(&mut desktop, "status-needs-you", "3"));
  // Needs you on the Overview lists each request, oldest first.
  assert!(shows(
    &mut desktop,
    "overview-attention",
    "Designer asks to use pencil batch_design_0"
  ));
  // Nothing reopens it by itself.
  desktop.pass();
  assert!(!has(&mut desktop, DIALOG));
  // This harness has no runner, so the status bar shows that first; Needs you's row opens the request.
  let review = first_review(&desktop).expect("a request row with Review");
  desktop.click(&review);
  desktop.wait_for(DIALOG);
  assert!(shows(&mut desktop, "tool-approval-position", "1 of 3 waiting"));
  // Esc is Decide later, never Deny.
  desktop.key("Escape", "Escape");
  desktop.wait_until("Esc closes the dialog", |desktop| !has(desktop, DIALOG));
  let db = ProjectDb::open(desktop.repo()).expect("open");
  assert_eq!(db.pending_tool_approvals_all().expect("requests").len(), 3);
}

#[test]
fn approval_choices_revoke_ends_a_similar_rule() {
  let (mut desktop, run) = waiting(0);
  let rule = ProjectDb::open(desktop.repo())
    .expect("open")
    .add_approval_rule(
      run,
      &RuleScope::SimilarMcpTool {
        server: "pencil".to_owned(),
        tool: "batch_design".to_owned(),
        folder: None,
        check: None,
      },
      at(30),
    )
    .expect("rule");
  open_task(&mut desktop);
  let revoke = format!("task-revoke-rule-{}", rule.id.get());
  desktop.wait_for(&revoke);
  assert!(shows(&mut desktop, "task-run-approvals", "pencil · batch_design"));
  // The panel scrolls; a click on the element itself, wherever it is scrolled to.
  desktop.click(&revoke);
  desktop.wait_until("the rule is revoked", |desktop| {
    ProjectDb::open(desktop.repo())
      .expect("open")
      .approval_rules(run)
      .expect("rules")[0]
      .revoked_at
      .is_some()
  });
  desktop.wait_until("Revoke goes away", |desktop| !has(desktop, &revoke));
}

#[test]
fn approval_choices_auto_approval_switch_indicator_and_stop() {
  let (mut desktop, run) = waiting(0);
  open_task(&mut desktop);
  desktop.wait_for("task-auto-approve");
  assert!(!has(&mut desktop, "task-auto-approving"));
  desktop.click("task-auto-approve");
  let chosen = |desktop: &mut Desktop| {
    ProjectDb::open(desktop.repo())
      .expect("open")
      .run_auto_approve(run)
      .expect("read")
  };
  desktop.wait_until("the run auto-approves", |desktop| chosen(desktop) == Some(true));
  desktop.wait_for("task-auto-approving");
  desktop.wait_for("status-auto-approving");
  assert!(shows(&mut desktop, "status-auto-approving", "Auto-approving"));
  assert!(shows(&mut desktop, "status-auto-approving-run", "· Run 1"));
  desktop.press("status-auto-approving-stop");
  desktop.wait_until("the run stops auto-approving", |desktop| chosen(desktop) == Some(false));
  desktop.wait_until("the indicators go away", |desktop| {
    !has(desktop, "status-auto-approving") && !has(desktop, "task-auto-approving")
  });
}

/// The id of the first request's Review on the Overview's Needs you.
fn first_review(desktop: &Desktop) -> Option<String> {
  let db = ProjectDb::open(desktop.repo()).expect("open");
  let oldest = db.pending_tool_approvals_all().expect("requests").into_iter().next()?;
  Some(format!("overview-request-review-{}", oldest.id))
}

pub(crate) fn open_task(desktop: &mut Desktop) {
  desktop.wait_for("nav-tasks");
  desktop.press("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  desktop.click("task-row-KON-1");
  desktop.wait_for("task-page");
}

/// An MCP request of `run`'s `invocation`, the `number`th.
fn mcp_request(run: &Run, invocation: &Invocation, number: i64) -> PendingToolApproval {
  PendingToolApproval {
    id: format!("{}:call_{number}", invocation.id),
    run: run.id(),
    execution: invocation.execution,
    invocation: invocation.id,
    agent: "Designer".to_owned(),
    command: false,
    scope: None,
    server: "pencil".to_owned(),
    action: format!("batch_design_{number}"),
    target: None,
    requested_at: at(20 + number),
    native: None,
  }
}

/// KON-1's run #1 waiting on one request, and run #2, which the task page shows, whose working step's
/// agent waits on one request of its own.
fn two_runs_waiting() -> Desktop {
  Desktop::with_seeded_project(|repo| {
    let (first, invocation) = native_run(repo);
    let mut db = ProjectDb::open(repo).expect("open the project");
    db.request_tool_approval(&mcp_request(&first, &invocation, 0))
      .expect("pending approval");
    let mut second = started(&mut db, (first.task(), first.snapshot()), 2, 30);
    let execution = second.attempts()[0].executions()[0].id();
    // A tool request leaves its step working, as the runner records it.
    let events = second.start_step(execution, at(31)).expect("start the step");
    db.append_events(&second, &events).expect("append");
    let invocation = db
      .start_invocation(
        second.id(),
        execution,
        &ModelId::new("deepseek-v4-pro").expect("model"),
        at(32),
      )
      .expect("invocation");
    db.request_tool_approval(&mcp_request(&second, &invocation, 9))
      .expect("pending approval");
  })
}

#[test]
fn approval_choices_subject_line_names_the_server_policy_and_wait() {
  let (mut desktop, _) = open(1);
  assert!(shows(&mut desktop, "tool-approval-category", "pencil"));
  assert!(shows(&mut desktop, "tool-approval-policy", "Asks before MCP actions"));
  assert!(shows(&mut desktop, "tool-approval-waiting", "waiting since"));
}

#[test]
fn approval_choices_approve_everything_shows_on_while_the_run_auto_approves() {
  let (mut desktop, run) = waiting(1);
  ProjectDb::open(desktop.repo())
    .expect("open")
    .set_run_auto_approve(run, Some(true))
    .expect("auto-approves");
  desktop.wait_for(DIALOG);
  // Auto-approval would answer an MCP call, so this one came before it was on.
  desktop.wait_until("the tile says the run auto-approves", |desktop| {
    shows(desktop, "tool-approval-approve-run", "came before it was on")
  });
}

#[test]
fn approval_choices_waiting_step_and_requests_in_other_runs() {
  let mut desktop = two_runs_waiting();
  desktop.wait_for(DIALOG);
  desktop.press("tool-approval-decide-later");
  desktop.wait_until("the dialog closes", |desktop| !has(desktop, DIALOG));
  open_task(&mut desktop);
  desktop.wait_for("task-approval-card");
  assert!(shows(
    &mut desktop,
    "task-approval-card",
    "1 more request waits in another run"
  ));
  assert!(shows(
    &mut desktop,
    "task-progress-steps",
    "Waiting for you · pencil batch_design_9 · since"
  ));
}

#[test]
fn approval_choices_agent_worktree_is_the_step_executions_own_or_the_runs() {
  let repo = tempfile::tempdir().expect("repository");
  let (run, execution) = (RunId::generate(), StepExecutionId::generate());
  let approval = PendingToolApproval {
    id: "invocation:1".to_owned(),
    run,
    execution,
    invocation: InvocationId::generate(),
    agent: "Builder".to_owned(),
    command: true,
    scope: None,
    server: "command".to_owned(),
    action: "cargo test".to_owned(),
    target: None,
    requested_at: at(1),
    native: None,
  };
  let worktrees = repo.path().join(".orchester").join("worktrees");
  assert_eq!(
    agent_worktree(repo.path(), None, &approval),
    worktrees.join(run.to_string())
  );
  std::fs::create_dir_all(worktrees.join(execution.to_string())).expect("the step's own worktree");
  assert_eq!(
    agent_worktree(repo.path(), None, &approval),
    worktrees.join(execution.to_string())
  );
}

/// KON-1's run waiting on `cargo test -p core`, which Builder's Allowed commands can take; the
/// request's id.
fn waiting_command() -> (Desktop, String) {
  let mut seeded = None;
  let desktop = Desktop::with_seeded_project(|repo| {
    let (run, invocation) = native_run(repo);
    let approval = PendingToolApproval {
      command: true,
      server: "command".to_owned(),
      action: "cargo test -p core".to_owned(),
      ..mcp_request(&run, &invocation, 0)
    };
    ProjectDb::open(repo)
      .expect("open the project")
      .request_tool_approval(&approval)
      .expect("pending approval");
    seeded = Some(approval.id);
  });
  (desktop, seeded.expect("seeded"))
}

/// The task's Conversation tab, after Decide later on the dialog that opened by itself.
fn conversation_of_waiting(desktop: &mut Desktop) {
  desktop.wait_for(DIALOG);
  desktop.press("tool-approval-decide-later");
  desktop.wait_until("the dialog closes", |desktop| !has(desktop, DIALOG));
  open_task(desktop);
  desktop.click("task-tab-conversation");
  desktop.wait_for("conversation-approval-always");
}

#[test]
fn approval_choices_card_shows_the_request_as_the_dialog_does() {
  let (mut desktop, request) = waiting_command();
  conversation_of_waiting(&mut desktop);
  assert!(shows(&mut desktop, "conversation-approval-category", "Worktree"));
  assert!(shows(
    &mut desktop,
    "conversation-approval-reason",
    "Changes the worktree:"
  ));
  let command = format!("conversation-approval-command-{request}");
  assert!(shows(&mut desktop, &command, "cargo test -p core"));
  // Nothing reopens the dialog after Decide later, on this tab or another page.
  assert!(!has(&mut desktop, DIALOG));
}

#[test]
fn approval_choices_card_always_allow_opens_the_step_and_back_returns_to_the_request() {
  let (mut desktop, _) = waiting_command();
  conversation_of_waiting(&mut desktop);
  desktop.click("conversation-approval-always");
  desktop.wait_for("tool-approval-always-title");
  desktop.press("tool-approval-back");
  desktop.wait_until("the request step of the same request", |desktop| {
    has(desktop, DIALOG) && !has(desktop, "tool-approval-always-title") && shows(desktop, DIALOG, "cargo test -p core")
  });
}
