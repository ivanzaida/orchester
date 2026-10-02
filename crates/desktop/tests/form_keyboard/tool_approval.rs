//! Long values in the approval dialog, from requests seeded into the project database as the runner
//! records them: a value that does not fit beside its label moves under it whole, in the
//! transcript's command box, which scrolls instead of cutting it; Copy takes the value as recorded;
//! and Approve and Deny stay in view in the smallest window.

use std::{
  sync::Arc,
  time::{Duration, Instant},
};

use lurq::{
  app::events::ScrollPhase,
  clipboard::{copy_to_clipboard, read_from_clipboard},
};
use orchester_core::{
  command::SecretVariable,
  connection::{
    claude_approval::{ClaudeApprovalScope, ClaudeAsk},
    mcp::McpServerName,
  },
  permission::{AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess},
  run::{ARGUMENTS_CEILING, CommandShell, NativeCommandAsk, PendingToolApproval, recorded_arguments},
  secret::SecretValue,
};
use orchester_desktop::theme::{APPROVAL_VALUE_MAX_HEIGHT, STATUS_BAR_HEIGHT, TITLE_BAR_HEIGHT};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use crate::{
  github::shows,
  harness::{Desktop, expected},
  native_run::native_run,
  unreadable_run::at,
};

const DIALOG: &str = "tool-approval-dialog";
const ACTION: &str = "tool-approval-action";
const TARGET: &str = "tool-approval-target";
const ARGUMENTS: &str = "tool-approval-arguments";

/// The smallest window the app opens (`main`).
const MIN_WIDTH: u32 = 960;
const MIN_HEIGHT: u32 = 700;

const LONG_TARGET: &str = ".orchester/worktrees/01J8Z9B2/packages/web/src/components/settings/keyboard-shortcuts/\
                           switcher/__tests__/fixtures/switcher-keyboard-shortcuts-with-every-binding.snapshot.json";

const HEREDOC: &str = "cat <<'EOF' > docs/keyboard-shortcuts.md
# Keyboard shortcuts

| Action                     | Windows            | macOS              |
| -------------------------- | ------------------ | ------------------ |
| Open the project switcher  | Ctrl+K             | Cmd+K              |
| Next project               | Ctrl+Tab           | Ctrl+Tab           |
| Previous project           | Ctrl+Shift+Tab     | Ctrl+Shift+Tab     |
| Close the switcher         | Escape             | Escape             |
| Pin the selected project   | Ctrl+P             | Cmd+P              |

Shortcuts work while the switcher has the focus.
They do nothing while a text field has it.

EOF
pnpm prettier --write docs/keyboard-shortcuts.md && git add docs/keyboard-shortcuts.md";

/// An MCP call's arguments as the runner records them: JSON on one line, secret values removed.
const ARGUMENTS_RAW: &str = concat!(
  r#"{"filePath":"H:\\projects\\orchester\\data\\orchester.pen","#,
  r#""input":"lbl=I(\"x\",{type:\"text\",content:\"a, b: {c}\"})\nU(\"y\",{a:[1,2]})","#,
  r#""parents":["wvh5j","coKjY"],"options":{},"items":[],"#,
  r#""apiKey":"<redacted>","depth":3,"dryRun":false,"note":null}"#
);

/// [`ARGUMENTS_RAW`] one member a line, every token as recorded.
const ARGUMENTS_SHOWN: &str = concat!(
  "{\n",
  r#"  "filePath": "H:\\projects\\orchester\\data\\orchester.pen","#,
  "\n",
  r#"  "input": "lbl=I(\"x\",{type:\"text\",content:\"a, b: {c}\"})\nU(\"y\",{a:[1,2]})","#,
  "\n",
  r#"  "parents": ["#,
  "\n",
  r#"    "wvh5j","#,
  "\n",
  r#"    "coKjY""#,
  "\n  ],\n",
  r#"  "options": {},"#,
  "\n",
  r#"  "items": [],"#,
  "\n",
  r#"  "apiKey": "<redacted>","#,
  "\n",
  r#"  "depth": 3,"#,
  "\n",
  r#"  "dryRun": false,"#,
  "\n",
  r#"  "note": null"#,
  "\n}"
);

/// What an agent asks about; `native` for an agent Orchester runs itself.
struct Request {
  command: bool,
  action: &'static str,
  target: Option<&'static str>,
  scope: Option<String>,
  native: Option<NativeCommandAsk>,
}

/// The dialog of `request`, pending on a seeded run.
fn approval_dialog(request: Request) -> Desktop {
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let (run, invocation) = native_run(repo);
    let approval = PendingToolApproval {
      id: format!("{}:call_9", invocation.id),
      run: run.id(),
      execution: invocation.execution,
      invocation: invocation.id,
      agent: "Builder".to_owned(),
      command: request.command,
      scope: request.scope,
      server: "pencil".to_owned(),
      action: request.action.to_owned(),
      target: request.target.map(str::to_owned),
      requested_at: at(20),
      native: request.native,
    };
    ProjectDb::open(repo)
      .expect("open the project")
      .request_tool_approval(&approval)
      .expect("pending approval");
  });
  desktop.wait_for(DIALOG);
  desktop
}

/// The box `id` holds exactly `text`, whole, and nothing but the box's own controls.
fn assert_whole(desktop: &mut Desktop, id: &str, text: &str) {
  let lines = desktop
    .tree
    .get_element_by_id(&format!("{id}-text"))
    .unwrap_or_else(|| panic!("{id} shows its value under its label"));
  assert_eq!(lines.text_content(), Some(text), "{id} shows the value whole");
  assert!(desktop.tree.get_element_by_id(&format!("{id}-copy")).is_some());
}

/// One wheel move of `(dx, dy)` over the middle of `id`.
fn wheel(desktop: &mut Desktop, id: &str, (dx, dy): (f32, f32)) {
  let bounds = desktop.bounds(id);
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  desktop.tree.scroll(x, y, 0.0, 0.0, ScrollPhase::Start);
  desktop.tree.scroll(x, y, dx, dy, ScrollPhase::Scroll);
  desktop.tree.scroll(x, y, 0.0, 0.0, ScrollPhase::End);
  desktop.pass();
}

/// The lines of `id` are taller than its box, which is no taller than the cap, and the wheel scrolls
/// them up inside it.
fn assert_scrolls_down(desktop: &mut Desktop, id: &str) {
  let scroll = format!("{id}-scroll");
  let text = format!("{id}-text");
  let frame = desktop.bounds(id);
  let viewport = desktop.bounds(&scroll);
  let before = desktop.bounds(&text);
  assert!(
    frame.height <= APPROVAL_VALUE_MAX_HEIGHT + 0.5,
    "{id} is {} px tall",
    frame.height
  );
  assert!(
    viewport.y + viewport.height <= frame.y + frame.height + 0.5,
    "{id} keeps its lines inside"
  );
  assert!(before.height > viewport.height, "{id}'s lines run past its box");
  wheel(desktop, &scroll, (0.0, -120.0));
  let after = desktop.bounds(&text);
  assert!(after.y < before.y, "the wheel scrolls {id}'s lines");
}

/// A line of `id` is wider than its box, and the wheel scrolls it sideways inside the box.
fn assert_scrolls_sideways(desktop: &mut Desktop, id: &str) {
  let scroll = format!("{id}-scroll");
  let text = format!("{id}-text");
  let viewport = desktop.bounds(&scroll);
  let before = desktop.bounds(&text);
  assert!(before.width > viewport.width, "{id}'s line runs past its box");
  wheel(desktop, &scroll, (-120.0, 0.0));
  let after = desktop.bounds(&text);
  assert!(after.x < before.x, "the wheel scrolls {id}'s line sideways");
}

/// In the smallest window, the dialog stays between the title bar and the status bar, and Approve
/// and Deny inside it.
fn assert_buttons_in_view_at_minimum_size(desktop: &mut Desktop) {
  desktop.tree.resize(MIN_WIDTH, MIN_HEIGHT);
  desktop.pass();
  let bottom = MIN_HEIGHT as f32 - STATUS_BAR_HEIGHT;
  let dialog = desktop.bounds(DIALOG);
  assert!(dialog.y >= TITLE_BAR_HEIGHT - 0.5, "the dialog starts at {}", dialog.y);
  assert!(
    dialog.y + dialog.height <= bottom + 0.5,
    "the dialog ends at {}",
    dialog.y + dialog.height
  );
  for button in ["tool-approval-deny", "tool-approval-approve-once"] {
    let bounds = desktop.bounds(button);
    assert!(bounds.height > 0.0, "{button} has its height");
    assert!(
      bounds.y >= dialog.y && bounds.y + bounds.height <= dialog.y + dialog.height,
      "{button} is inside the dialog"
    );
  }
}

/// Presses Copy of `id` and returns what the clipboard then holds; the clipboard's previous text is
/// put back.
fn copied_text(desktop: &mut Desktop, id: &str) -> String {
  let previous = read_from_clipboard().expect("a text clipboard whose previous contents can be restored");
  let copy = format!("{id}-copy");
  desktop.press(&copy);
  let copied = read_from_clipboard();
  let confirmed = shows(desktop, &copy, "Copied");
  assert!(copy_to_clipboard(&previous), "restore the previous clipboard text");
  assert!(confirmed, "Copy says it copied");
  copied.expect("Copy put text on the clipboard")
}

fn long_target() -> Desktop {
  approval_dialog(Request {
    command: false,
    action: "batch_get",
    target: Some(LONG_TARGET),
    scope: None,
    native: None,
  })
}

fn heredoc() -> Desktop {
  approval_dialog(Request {
    command: true,
    action: HEREDOC,
    target: Some(".orchester/worktrees/01J8Z9B2"),
    scope: Some("Write the shortcut table.".to_owned()),
    native: None,
  })
}

fn large_arguments() -> Desktop {
  approval_dialog(Request {
    command: false,
    action: "batch_design",
    target: Some("H:\\projects\\orchester\\data\\orchester.pen"),
    scope: Some(ARGUMENTS_RAW.to_owned()),
    native: None,
  })
}

#[test]
fn tool_approval_long_target_moves_under_its_label_whole_and_scrolls_sideways() {
  let mut desktop = long_target();
  assert_whole(&mut desktop, TARGET, LONG_TARGET);
  // The short action stays beside its label.
  assert!(desktop.tree.get_element_by_id(&format!("{ACTION}-text")).is_none());
  assert!(shows(&mut desktop, DIALOG, "batch_get"));
  assert_scrolls_sideways(&mut desktop, TARGET);
  assert_buttons_in_view_at_minimum_size(&mut desktop);
}

#[test]
fn tool_approval_heredoc_command_keeps_its_lines_and_scrolls_inside_its_box() {
  let mut desktop = heredoc();
  assert_whole(&mut desktop, ACTION, HEREDOC);
  // The short working directory stays beside its label.
  assert!(desktop.tree.get_element_by_id(&format!("{TARGET}-text")).is_none());
  assert!(shows(&mut desktop, DIALOG, ".orchester/worktrees/01J8Z9B2"));
  assert_scrolls_down(&mut desktop, ACTION);
  assert_buttons_in_view_at_minimum_size(&mut desktop);
}

#[test]
fn tool_approval_large_arguments_read_one_member_a_line_and_scroll() {
  let mut desktop = large_arguments();
  assert_whole(&mut desktop, ARGUMENTS, ARGUMENTS_SHOWN);
  assert!(shows(&mut desktop, DIALOG, "batch_design"));
  assert!(shows(
    &mut desktop,
    DIALOG,
    "H:\\projects\\orchester\\data\\orchester.pen"
  ));
  assert_scrolls_down(&mut desktop, ARGUMENTS);
  assert_buttons_in_view_at_minimum_size(&mut desktop);
}

#[test]
fn tool_approval_arguments_that_are_not_json_are_shown_as_recorded() {
  // Before arguments were recorded whole, they were cut with an ellipsis and no longer parse.
  let cut = r#"{"filePath":"H:\\projects\\orchester\\data\\orchester.pen","input":"lbl=I(\"x\"…"#;
  let mut desktop = approval_dialog(Request {
    command: false,
    action: "batch_design",
    target: None,
    scope: Some(cut.to_owned()),
    native: None,
  });
  assert_whole(&mut desktop, ARGUMENTS, cut);
}

#[test]
#[ignore = "writes to the clipboard and restores its previous text; run separately"]
fn tool_approval_copy_takes_each_value_as_recorded() {
  let mut desktop = long_target();
  assert_eq!(copied_text(&mut desktop, TARGET), LONG_TARGET);
  let mut desktop = heredoc();
  assert_eq!(copied_text(&mut desktop, ACTION), HEREDOC);
  let mut desktop = large_arguments();
  assert_eq!(
    copied_text(&mut desktop, ARGUMENTS),
    ARGUMENTS_RAW,
    "Copy takes the arguments as recorded, not as laid out"
  );
}

#[test]
fn tool_approval_native_shell_script_and_long_folder_keep_the_buttons_in_view() {
  // A native shell command's dialog is the tallest: its notice and time limit come on top.
  let mut desktop = approval_dialog(Request {
    command: true,
    action: HEREDOC,
    target: Some(LONG_TARGET),
    scope: None,
    native: Some(NativeCommandAsk {
      shell: Some(CommandShell::Pwsh),
      time_limit_seconds: 120,
    }),
  });
  assert!(shows(&mut desktop, "tool-approval-shell-command", HEREDOC));
  assert_whole(&mut desktop, TARGET, LONG_TARGET);
  assert_buttons_in_view_at_minimum_size(&mut desktop);
  // The details scroll between the header and the buttons.
  let details = desktop.bounds("tool-approval-details");
  let viewport = desktop.bounds("tool-approval-details-scroll");
  assert!(
    details.height > viewport.height,
    "the details run past the dialog's middle"
  );
}

/// The arguments Claude Code's permission prompt records for a `pencil` call with `input`, with
/// `secret` removed, as the runner stores them.
fn recorded_for(input: &Value, secret: &str) -> String {
  let mut permissions = AgentPermissions::new(RepositoryAccess::Write, CommandPermission::AskBefore, NetworkAccess::On);
  permissions.set_interactive_tool_approvals(true);
  let server = McpServerName::new("pencil").expect("server name");
  let secret = SecretVariable::new(
    "PENCIL_TOKEN",
    Arc::new(SecretValue::new(secret.to_owned()).expect("secret")),
  );
  let scope = ClaudeApprovalScope::of(&permissions, &[server])
    .expect("the step asks before MCP actions")
    .with_secrets(vec![secret]);
  let call = json!({"tool_name": "mcp__pencil__batch_design", "input": input, "tool_use_id": "toolu_1"});
  let ClaudeAsk::Person(request) = scope.classify(&call) else {
    panic!("the call goes to a person");
  };
  request.scope().expect("arguments").to_owned()
}

fn mcp_arguments(scope: String) -> Desktop {
  approval_dialog(Request {
    command: false,
    action: "batch_design",
    target: None,
    scope: Some(scope),
    native: None,
  })
}

#[test]
fn tool_approval_five_kilobytes_of_arguments_arrive_whole_laid_out_and_without_the_secret() {
  let token = "pencil-live-7f3a9c2e5b1d";
  let operations: Vec<String> = (0..100)
    .map(|line| format!("op{line:03}=I(\"frame\",{{name:\"Row {line}\",width:\"fill_container\"}})"))
    .collect();
  let input = json!({
    "filePath": r"H:\projects\orchester\data\orchester.pen",
    "operations": operations,
    "headers": {"Authorization": format!("Bearer {token}")},
  });
  let recorded = recorded_for(&input, token);
  assert!(recorded.len() > 5 * 1024, "{} bytes", recorded.len());
  assert!(!recorded.contains(token), "the secret is removed before it is recorded");
  let mut desktop = mcp_arguments(recorded.clone());
  // The dialog lays out exactly what was recorded: the same JSON, pretty-printed.
  let value: Value = serde_json::from_str(&recorded).expect("recorded arguments are JSON");
  let laid_out = serde_json::to_string_pretty(&value).expect("pretty JSON");
  assert_whole(&mut desktop, ARGUMENTS, &laid_out);
  assert!(laid_out.contains("op099=I"), "the last operation is shown");
  assert!(laid_out.contains("\"Authorization\": \"Bearer <redacted>\""));
  assert!(!shows(&mut desktop, DIALOG, token), "the secret is not in the dialog");
  assert_scrolls_down(&mut desktop, ARGUMENTS);
  assert_buttons_in_view_at_minimum_size(&mut desktop);
}

#[test]
fn tool_approval_arguments_cut_at_the_ceiling_say_so_under_what_was_kept() {
  let recorded = recorded_arguments(format!("{{\"text\":\"{}\"}}", "x".repeat(ARGUMENTS_CEILING)));
  let mut desktop = mcp_arguments(recorded);
  let kept = format!("{{\n  \"text\": \"{}", "x".repeat(ARGUMENTS_CEILING - 9));
  assert_whole(&mut desktop, ARGUMENTS, &format!("{kept}\n… cut at 256 KB"));
  assert_buttons_in_view_at_minimum_size(&mut desktop);
}

#[test]
fn tool_approval_deeply_nested_arguments_are_shown_as_recorded_and_quickly() {
  // 120 levels deep, and 20 000 elements at the bottom: laid out one a line, every element would
  // carry 240 spaces of indent, about 5 MB.
  let raw = format!("{}{}{}", "[".repeat(120), vec!["1"; 20_000].join(","), "]".repeat(120));
  serde_json::from_str::<Value>(&raw).expect("valid JSON");
  let started = Instant::now();
  let mut desktop = mcp_arguments(raw.clone());
  let elapsed = started.elapsed();
  assert_whole(&mut desktop, ARGUMENTS, &raw);
  assert!(shows(&mut desktop, "tool-approval-arguments-note", "Shown as recorded"));
  assert!(elapsed < Duration::from_secs(3), "the dialog appears in {elapsed:?}");
  // Small arguments nested as deep are still laid out.
  let small = format!("{}1{}", "[".repeat(100), "]".repeat(100));
  let mut desktop = mcp_arguments(small);
  let lines = desktop
    .tree
    .get_element_by_id(&format!("{ARGUMENTS}-text"))
    .and_then(|element| element.text_content().map(str::to_owned))
    .expect("the arguments are shown");
  assert_eq!(lines.lines().count(), 201, "one bracket a line, and the number");
  assert!(desktop.tree.get_element_by_id("tool-approval-arguments-note").is_none());
}

#[test]
fn tool_approval_values_of_60_columns_stay_beside_their_label() {
  let inline = |target: &'static str| {
    let mut desktop = approval_dialog(Request {
      command: false,
      action: "batch_get",
      target: Some(target),
      scope: None,
      native: None,
    });
    let beside = desktop.tree.get_element_by_id(&format!("{TARGET}-text")).is_none();
    if !beside {
      assert_whole(&mut desktop, TARGET, target);
    }
    beside
  };
  assert!(inline(&*"a".repeat(60).leak()), "60 columns fit");
  assert!(!inline(&*"a".repeat(61).leak()), "61 do not");
  // A character outside ASCII counts as two columns.
  assert!(inline(&*"é".repeat(30).leak()), "30 wide characters fit");
  assert!(!inline(&*format!("{}a", "é".repeat(30)).leak()), "61 columns do not");
  // A tab or another control character never stays beside the label, however short.
  assert!(!inline("docs/\tnotes.md"));
  assert!(!inline("docs/\u{1b}[31mnotes.md"));
}

#[test]
fn tool_approval_tab_goes_through_each_copy_then_deny_and_approve() {
  let mut desktop = approval_dialog(Request {
    command: true,
    action: HEREDOC,
    target: Some(LONG_TARGET),
    scope: None,
    native: None,
  });
  let action_copy = format!("{ACTION}-copy");
  let target_copy = format!("{TARGET}-copy");
  desktop.tab_to(&action_copy, 8);
  assert_eq!(
    desktop.stops(4, false),
    expected(&[
      Some(&target_copy),
      Some("tool-approval-deny"),
      Some("tool-approval-approve-once"),
      Some(&action_copy),
    ]),
    "Tab goes on from the action's Copy and wraps in the dialog"
  );
  assert_eq!(
    desktop.stops(1, true),
    expected(&[Some("tool-approval-approve-once")]),
    "Shift+Tab goes back"
  );
}
