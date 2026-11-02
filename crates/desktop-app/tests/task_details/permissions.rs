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
