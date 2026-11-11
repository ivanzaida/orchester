//! The *Approve mode* in Project settings › Runs › Approvals (`docs/design/approval-modes.md`, `v5d5r`: `XBtjx`,
//! `CdyBy`, `i0Aqq`, `Cuj66`): Manual by default; Dangerous auto-approve asks first in its confirmation, where *Cancel*
//! has the focus, Enter does nothing and Esc and *Cancel* leave the mode as it was; only a person's press of the danger
//! button chooses it, an MCP client's is refused; chosen, the row says so and the rows under it are dimmed; moving down
//! is anyone's, without a question.
use lurq::app::events::with_synthetic_input;
use orchester_core::run::ApproveMode;

use super::harness::Settings;

const DANGEROUS: &str = "settings-approve-mode-dangerous";
const DIALOG: &str = "settings-dangerous-dialog";
const HINT: &str = "settings-auto-approve-mode-hint";

fn mode(page: &Settings) -> ApproveMode {
  page.db().approve_mode().expect("read")
}

fn focused(page: &Settings) -> Option<String> {
  page
    .window
    .tree
    .focused_element()
    .and_then(|element| element.id().map(str::to_owned))
}

fn key(page: &mut Settings, key: &str) {
  page
    .window
    .tree
    .key_down(key.to_owned(), key.to_owned(), false, false, false);
  page.window.pass();
}

/// Lets what a press set off settle, so a save that should not happen has had its chance.
fn settle(page: &mut Settings) {
  for _ in 0..10 {
    page.window.tree.tick_futures();
    page.window.pass();
  }
}

fn bounds(page: &mut Settings, id: &str) -> lurq::core::ElementRect {
  page
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
}

#[test]
fn approve_mode_is_manual_by_default_with_three_choices() {
  let mut page = Settings::new(true);
  assert_eq!(mode(&page), ApproveMode::Manual);
  assert_eq!(page.text("settings-auto-approve-mode-label"), "Approve mode");
  assert!(page.exists("settings-auto-approve-mode-info"));
  for (id, words) in [
    ("settings-approve-mode-manual", "Manual"),
    ("settings-approve-mode-autopilot", "Autopilot"),
    (DANGEROUS, "Dangerous auto-approve"),
  ] {
    assert_eq!(page.text(&format!("{id}-label")), words);
  }
  assert_eq!(
    page.text(HINT),
    "Every agent asks for what its allowed list does not cover, whatever its profile says"
  );
  assert!(
    page.exists("settings-auto-foot"),
    "the group's foot outside Dangerous auto-approve"
  );
  assert!(!page.exists("settings-auto-dangerous-note"));
  // The control sits on the label's line; the hint runs the row's width under both (`aHLLh`).
  let label = bounds(&mut page, "settings-auto-approve-mode-label");
  let control = bounds(&mut page, "settings-approve-mode");
  let hint = bounds(&mut page, HINT);
  assert!(
    (control.height - 26.0).abs() < 0.5,
    "the control is 26 high: {control:?}"
  );
  let (label_middle, control_middle) = (label.y + label.height / 2.0, control.y + control.height / 2.0);
  assert!((label_middle - control_middle).abs() < 1.0, "on one line");
  assert!(hint.y >= control.y + control.height, "the hint under the control");
}

#[test]
fn approve_mode_dangerous_asks_first_and_cancel_esc_and_enter_leave_it() {
  let mut page = Settings::new(true);
  page.click(DANGEROUS);
  page.until("the confirmation", |p| p.exists(DIALOG));
  assert_eq!(
    page.text("settings-dangerous-dialog-title"),
    "Turn on Dangerous auto-approve mode?"
  );
  assert!(
    page
      .text("settings-dangerous-dialog-text")
      .starts_with("Every tool request of every agent in Kontur"),
    "names the project"
  );
  for index in 0..4 {
    assert!(page.exists(&format!("settings-dangerous-dialog-risk-{index}")));
  }
  assert!(!page.exists("settings-dangerous-dialog-enter"), "no Enter hint");
  page.until("Cancel has the focus", |p| {
    focused(p).as_deref() == Some("settings-dangerous-dialog-cancel")
  });
  // Enter is bound to nothing of the dialog's own: it presses the focused Cancel, never the danger button.
  key(&mut page, "Enter");
  settle(&mut page);
  assert!(!page.exists(DIALOG), "Enter pressed the focused Cancel");
  assert_eq!(mode(&page), ApproveMode::Manual);
  page.click(DANGEROUS);
  page.until("asked again", |p| p.exists(DIALOG));
  key(&mut page, "Escape");
  page.until("Esc cancels", |p| !p.exists(DIALOG));
  page.click(DANGEROUS);
  page.until("asked again", |p| p.exists(DIALOG));
  page.click("settings-dangerous-dialog-cancel");
  page.until("Cancel cancels", |p| !p.exists(DIALOG));
  settle(&mut page);
  assert_eq!(
    mode(&page),
    ApproveMode::Manual,
    "the selector moves only after the danger button"
  );
}

#[test]
fn approve_mode_dangerous_is_chosen_by_a_persons_danger_button_alone_and_left_by_anyone() {
  let mut page = Settings::new(true);
  with_synthetic_input(|| page.click(DANGEROUS));
  page.until("the confirmation opens for any input", |p| p.exists(DIALOG));
  with_synthetic_input(|| page.click("settings-dangerous-dialog-confirm"));
  page.until("the refusal", |p| p.exists("settings-dangerous-person-only"));
  settle(&mut page);
  assert_eq!(mode(&page), ApproveMode::Manual, "an MCP client's press is not taken");
  page.click("settings-dangerous-dialog-confirm");
  page.until("a person's press chooses it", |p| {
    mode(p) == ApproveMode::DangerousAutoApprove
  });
  page.until("the dialog closes", |p| !p.exists(DIALOG));
  page.saved();
  page.until("the row says so", |p| {
    p.text(HINT)
      .starts_with("Every tool request of every agent in this project is approved at once")
  });
  assert_eq!(
    page.text("settings-auto-dangerous-note-text"),
    "The hold rules, the reviewer and the approved rules are not used while Dangerous auto-approve is chosen."
  );
  assert!(!page.exists("settings-auto-foot"));
  assert!(
    page.exists("settings-reviewer-connection"),
    "the rows under it stay usable"
  );
  // Down is anyone's, without a question.
  with_synthetic_input(|| page.click("settings-approve-mode-autopilot"));
  page.until("an MCP client moves it down", |p| mode(p) == ApproveMode::Autopilot);
  assert!(!page.exists(DIALOG));
  page.saved();
  with_synthetic_input(|| page.click("settings-approve-mode-manual"));
  page.until("down to Manual", |p| mode(p) == ApproveMode::Manual);
  page.saved();
  // Up from Manual to Autopilot is a person's: an MCP client's press is refused under the row.
  with_synthetic_input(|| page.click("settings-approve-mode-autopilot"));
  page.until("the refusal", |p| p.exists("settings-approve-mode-person-only"));
  settle(&mut page);
  assert_eq!(mode(&page), ApproveMode::Manual);
}
