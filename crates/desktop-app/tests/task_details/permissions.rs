//! The run's permissions chip in the Conversation's composer (ORC-319, 15 `x55hWH`, 15b `G1Vm1e`, 15c `idkP8`, 15d
//! `K8CGe`): it says how the run answers its requests and what the profile allows, in place of the header's mode chip
//! there; its menu changes that for the run; each change shows as an event line; widening from an MCP client's press is
//! refused, and the refusal shows as an event line with *Review…*, which opens the menu again.
//!
//! `cargo test -p orchester-app --test task_details permissions`

use lurq::app::events::MouseButton;
use orchester_app::routes::TaskTab;
use orchester_core::run::{Answerer, PermissionEventKind, Run};
use orchester_store::ProjectDb;

use super::{
  Page,
  run_mode::autopilot,
  support::link_press::{Presser, press_control},
};

const CHIP: &str = "conversation-permissions";
const LABEL: &str = "conversation-permissions-label";
const MENU: &str = "conversation-permissions-menu";

/// The run's permission events, as stored.
fn events(page: &Page, run: &Run) -> Vec<(Answerer, PermissionEventKind)> {
  ProjectDb::open(&page.repo)
    .expect("the project")
    .permission_events(run.id())
    .expect("events")
    .into_iter()
    .map(|event| (event.by, event.kind))
    .collect()
}

fn chosen(page: &Page, run: &Run) -> Option<bool> {
  ProjectDb::open(&page.repo)
    .expect("the project")
    .run_auto_approve(run.id())
    .expect("read")
}

/// Opens the chip's menu, then asks *Approve everything in this run* in place.
fn ask_everything(page: &mut Page) {
  page.window.press(CHIP);
  page.window.until("the menu", |window| window.texts(MENU).is_some());
  page.window.press("conversation-permissions-everything");
  page.window.until("the question in place", |window| {
    window.texts("conversation-permissions-confirm").is_some()
  });
  let title = page.text("conversation-permissions-confirm-title");
  assert!(
    title.starts_with("Let Run ") && title.ends_with(" answer its requests itself?"),
    "the approval card's question: {title}"
  );
}

impl Page {
  fn until_label(&mut self, label: &str) {
    let label = label.to_owned();
    self.window.until(&format!("the chip reads {label}"), |window| {
      window.texts(LABEL) == Some(vec![label.clone()])
    });
  }
}

#[test]
fn permissions_chip_names_the_mode_and_what_the_profile_allows() {
  let mut page = Page::seeded("task-states");
  let (key, _) = autopilot(&page);
  page.open(&key, TaskTab::Conversation, CHIP);
  assert_eq!(page.text(LABEL), "Autopilot");
  assert_eq!(page.text("conversation-permissions-profile"), "· profile: Autopilot");
  assert!(
    !page.has("task-run-mode-label"),
    "on the Conversation the chip says the mode, not the header"
  );
  page.window.press(CHIP);
  page.window.until("the menu", |window| window.texts(MENU).is_some());
  assert!(
    page
      .text("conversation-permissions-heading")
      .ends_with("’s requests are answered")
  );
  assert_eq!(page.text("conversation-permissions-manual-name"), "Manual for this run");
  assert_eq!(page.text("conversation-permissions-autopilot-name"), "Autopilot");
  assert_eq!(
    page.text("conversation-permissions-everything-name"),
    "Approve everything in this run"
  );
  assert_eq!(
    page.text("conversation-permissions-similar-empty"),
    "None yet: Approve similar on a request’s card adds one here."
  );
}

#[test]
fn permissions_a_person_approves_everything_then_stops_it_each_change_an_event_line() {
  let mut page = Page::seeded("task-states");
  let (key, run) = autopilot(&page);
  page.open(&key, TaskTab::Conversation, CHIP);
  ask_everything(&mut page);
  page.window.press("conversation-permissions-confirm-approve");
  page.until_label("Everything in this run");
  assert_eq!(chosen(&page, &run), Some(true));
  let line = "conversation-permission-permission-0";
  page
    .window
    .until("the event line", |window| window.texts(line).is_some());
  assert_eq!(page.text(&format!("{line}-text")), "Approving everything in this run");
  let detail = page.text(&format!("{line}-detail"));
  assert!(
    detail.starts_with("· by you · ") && detail.contains("was Autopilot"),
    "{detail}"
  );
  assert_eq!(page.text(&format!("{line}-action")), "Stop");
  page.window.press(&format!("{line}-action"));
  page.until_label("Autopilot");
  assert_eq!(chosen(&page, &run), None, "back to Autopilot");
  let stopped = "conversation-permission-permission-1";
  page
    .window
    .until("the second event line", |window| window.texts(stopped).is_some());
  assert_eq!(
    page.text(&format!("{stopped}-text")),
    "Stopped approving everything in this run"
  );
  assert!(!page.has(&format!("{line}-action")), "Stop goes once it stopped");
}

#[test]
fn permissions_widening_from_automation_input_is_refused_with_an_event_line() {
  let mut page = Page::seeded("task-states");
  let (key, run) = autopilot(&page);
  page.open(&key, TaskTab::Conversation, CHIP);
  ask_everything(&mut page);
  press_control(
    &mut page.window,
    "conversation-permissions-confirm-approve",
    (Presser::Synthetic, MouseButton::Left),
    (false, false),
  );
  let line = "conversation-permission-permission-0";
  page
    .window
    .until("the refusal's line", |window| window.texts(line).is_some());
  assert_eq!(chosen(&page, &run), None, "nothing widened");
  assert_eq!(
    events(&page, &run),
    [(Answerer::Automation, PermissionEventKind::Refused)]
  );
  assert_eq!(page.text(LABEL), "Autopilot");
  assert_eq!(
    page.text(&format!("{line}-text")),
    "Not changed: automation can’t widen a run"
  );
  assert_eq!(page.text(&format!("{line}-action")), "Review…");
  page.window.press(&format!("{line}-action"));
  page
    .window
    .until("the menu again", |window| window.texts(MENU).is_some());
  assert_eq!(
    page.text("conversation-permissions-foot-note"),
    "Only a person widens a run: the Orchestrator and MCP clients may only narrow it."
  );
}

#[test]
fn permissions_manual_for_this_run_narrows_the_run_for_good() {
  let mut page = Page::seeded("task-states");
  let (key, run) = autopilot(&page);
  page.open(&key, TaskTab::Conversation, CHIP);
  page.window.press(CHIP);
  page.window.until("the menu", |window| window.texts(MENU).is_some());
  page.window.press("conversation-permissions-manual");
  page.until_label("Manual · this run");
  let db = ProjectDb::open(&page.repo).expect("the project");
  assert!(db.run_switched_to_manual(run.id()).expect("read"));
  page.window.until("the switch's row", |window| {
    window.texts("conversation-switched-to-manual").is_some()
  });
  page.window.press(CHIP);
  page.window.until("the menu", |window| window.texts(MENU).is_some());
  assert!(
    page
      .text("conversation-permissions-autopilot-reason")
      .starts_with("Not available: a run switched to Manual stays Manual until it ends."),
    "no way back to Autopilot"
  );
}

#[test]
fn permissions_chip_is_gone_once_the_run_ended() {
  let mut page = Page::seeded("task-states");
  let key = page.key("failed");
  page.open(&key, TaskTab::Conversation, "conversation");
  assert!(!page.has(CHIP));
}

#[test]
fn permissions_roles_follow_the_design() {
  use lurq::{app::App, layout::text_style::FontWeight};
  use orchester_app::theme::{self, Mode, PermissionsText};
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  let theme = app.theme();
  for (role, size, weight) in [
    (PermissionsText::Chip, 11.5, FontWeight::Normal),
    (PermissionsText::Heading, 11.0, FontWeight::Medium),
    (PermissionsText::Tag, 11.0, FontWeight::Normal),
    (PermissionsText::Revoke, 11.5, FontWeight::Medium),
    (PermissionsText::Action, 12.0, FontWeight::Medium),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.weight, style.line_height),
      (size, weight, 1.3),
      "{role:?}"
    );
  }
}

/// While the project is in Dangerous auto-approve the chip says so in `danger`, without the profile's word, and its
/// menu offers no way of answering: one line that the project approves every request and a link to Project settings ›
/// Runs.
#[test]
fn permissions_chip_says_dangerous_auto_approve_and_offers_no_way() {
  use orchester_core::run::ApproveMode;
  let mut page = Page::seeded("task-states");
  let (key, _) = autopilot(&page);
  ProjectDb::open(&page.repo)
    .expect("the project")
    .save_approve_mode(ApproveMode::DangerousAutoApprove)
    .expect("a person chooses it");
  page.open(&key, TaskTab::Conversation, CHIP);
  page.until_label("Dangerous auto-approve");
  assert!(!page.has("conversation-permissions-profile"), "no profile's word");
  page.window.press(CHIP);
  page.window.until("the menu", |window| window.texts(MENU).is_some());
  assert!(
    page
      .text("conversation-permissions-dangerous-note")
      .contains("is in Dangerous auto-approve: every tool request of every agent is approved at once")
  );
  for way in [
    "conversation-permissions-manual",
    "conversation-permissions-autopilot",
    "conversation-permissions-everything",
  ] {
    assert!(!page.has(way), "{way} is not offered");
  }
  assert_eq!(
    page.text("conversation-permissions-settings-link"),
    "Approve mode in Project settings"
  );
}

impl Page {
  /// Moves the project's Approve mode to `to` as `by`, and tells the open page so, as Project settings' save and the
  /// MCP tools do: the project's auto-mode change.
  fn move_approve_mode(&mut self, to: orchester_core::run::ApproveMode, by: Answerer) {
    orchester_workspace::project_settings::save_approve_mode_in(&self.repo, to, by).expect("moved");
    let services = self.window.launch.services().expect("the session started").clone();
    services
      .changes
      .record(self.project, orchester_workspace::tasks::outside::Outside::AutoMode);
  }
}

/// With the task's Conversation open and its menu showing, a move of the Approve mode elsewhere reaches the chip and
/// the open menu without leaving the page: down by automation, then up by a person.
#[test]
fn permissions_chip_and_open_menu_follow_the_approve_mode_on_an_open_page() {
  use orchester_core::run::ApproveMode;
  let mut page = Page::seeded("task-states");
  let (key, _) = autopilot(&page);
  page.move_approve_mode(ApproveMode::DangerousAutoApprove, Answerer::Person);
  page.open(&key, TaskTab::Conversation, CHIP);
  page.until_label("Dangerous auto-approve");
  page.window.press(CHIP);
  page.window.until("the menu", |window| window.texts(MENU).is_some());
  assert!(page.has("conversation-permissions-dangerous-note"));
  // Down by automation: the chip and the open menu say Autopilot and offer its ways again.
  page.move_approve_mode(ApproveMode::Autopilot, Answerer::Automation);
  page.until_label("Autopilot");
  page.window.until("the menu's ways", |window| {
    window.texts("conversation-permissions-manual").is_some()
  });
  assert!(!page.has("conversation-permissions-dangerous-note"));
  // Up by a person: the open page says Dangerous auto-approve again.
  page.move_approve_mode(ApproveMode::DangerousAutoApprove, Answerer::Person);
  page.until_label("Dangerous auto-approve");
  page.window.until("the menu's line", |window| {
    window.texts("conversation-permissions-dangerous-note").is_some()
  });
}

/// The task header's mode chip follows the Approve mode on an open Summary too, and in Dangerous auto-approve reads as
/// the composer and the status bar do: a `triangle-alert` badge in `danger`.
#[test]
fn permissions_header_chip_follows_the_approve_mode_on_an_open_page() {
  use orchester_core::run::ApproveMode;
  let mut page = Page::seeded("task-states");
  let (key, _) = autopilot(&page);
  page.open(&key, TaskTab::Summary, "task-run-mode-label");
  assert_eq!(page.text("task-run-mode-label"), "Autopilot");
  page.move_approve_mode(ApproveMode::DangerousAutoApprove, Answerer::Person);
  page.window.until("the header chip", |window| {
    window.texts("task-run-mode-label") == Some(vec!["Dangerous auto-approve".to_owned()])
  });
  let glyph = orchester_app::ui::icon::Icon::TriangleAlert.glyph().to_string();
  assert!(
    page
      .window
      .tree
      .get_element_by_id("task-run-mode-glyph")
      .and_then(|element| element.text_content().map(str::to_owned))
      .is_some_and(|shown| shown == glyph),
    "the badge's triangle"
  );
  assert!(!page.has("task-run-mode-switch"), "nothing to switch");
  page.move_approve_mode(ApproveMode::Manual, Answerer::Automation);
  page.window.until("the header chip back", |window| {
    window.texts("task-run-mode-label") == Some(vec!["Manual · project".to_owned()])
  });
}
