//! The panel beside a rule is the route's: Variables, Preview and the full prompt a person opens are entries of
//! the tab's history, Back and Forward step through them, and closing one leaves no entry behind.
use orchester_app::routes::{InstructionsPanel, ProjectPage, Route};
use orchester_core::resource::InstructionScope;

use super::{harness::Instructions, preview::seed_preview};

fn panel(screen: &Instructions) -> InstructionsPanel {
  match screen.window.navigation().route_now() {
    Route::Project(_, ProjectPage::Instructions(view)) => view.panel().expect("a rule's view"),
    other => panic!("instructions route: {other:?}"),
  }
}

fn back(screen: &mut Instructions) {
  screen.window.navigation().back();
  screen.window.pass();
  screen.window.pass();
}

fn forward(screen: &mut Instructions) {
  screen.window.navigation().forward();
  screen.window.pass();
  screen.window.pass();
}

#[test]
fn instructions_side_panels_are_history_entries_and_closing_leaves_none() {
  let mut screen = Instructions::new();
  seed_preview(&screen);
  screen.rule(
    "Standards",
    "Working on {{task.title}}",
    InstructionScope::Project,
    true,
  );
  screen.go();
  screen.until("variables", |s| s.exists("instruction-variable-detail"));
  assert_eq!(panel(&screen), InstructionsPanel::Variables);
  screen.click("instruction-panel-1");
  screen.until("preview", |s| s.exists("instruction-full-prompt"));
  assert_eq!(panel(&screen), InstructionsPanel::Preview);
  screen.click("instruction-full-prompt");
  screen.until("viewer", |s| s.exists("instruction-full-viewer"));
  assert_eq!(panel(&screen), InstructionsPanel::Full);

  back(&mut screen);
  screen.until("preview again", |s| s.exists("instruction-full-prompt"));
  assert_eq!(panel(&screen), InstructionsPanel::Preview);
  assert!(screen.exists("agents-hub-tabs"), "the page is back around the rule");
  back(&mut screen);
  screen.until("variables again", |s| s.exists("instruction-variable-detail"));
  assert_eq!(panel(&screen), InstructionsPanel::Variables);
  forward(&mut screen);
  screen.until("preview forward", |s| s.exists("instruction-full-prompt"));
  assert_eq!(panel(&screen), InstructionsPanel::Preview);

  // The viewer's Back closes it: Forward does not reopen the full prompt.
  screen.click("instruction-full-prompt");
  screen.until("viewer", |s| s.exists("instruction-full-viewer"));
  screen.click("instruction-viewer-back");
  screen.until("preview after the viewer", |s| s.exists("instruction-full-prompt"));
  forward(&mut screen);
  assert_eq!(panel(&screen), InstructionsPanel::Preview);

  screen.click("instruction-panel-close");
  assert_eq!(panel(&screen), InstructionsPanel::Hidden);
  assert!(!screen.exists("instruction-panel"));
  back(&mut screen);
  screen.until("variables before the closed preview", |s| {
    s.exists("instruction-variable-detail")
  });
  assert_eq!(panel(&screen), InstructionsPanel::Variables);
}
