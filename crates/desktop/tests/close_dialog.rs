//! The close dialog rendered headlessly from fixture work (02.C `Xz30C`, 02.D `a0tTV`). The
//! fixtures are test data only.

use std::sync::{
  Arc,
  atomic::{AtomicUsize, Ordering},
};

use lurq::{
  app::{App, Tree},
  node::element::ElementRef,
};
use orchester_core::{connection::AgentProvider, id::RunId};
use orchester_desktop::{
  close_dialog::{CloseActions, CloseDialog, CloseDialogProps, CloseStage},
  i18n,
  runner::{ActiveWork, ActiveWorkFields, SaveState},
  theme,
};

fn active(project: &str, task_key: &str, task_title: &str, step: &str, agent: &str) -> ActiveWork {
  ActiveWork::new(&ActiveWorkFields {
    project,
    run: RunId::generate(),
    task_key,
    task_title,
    step,
    agent,
    provider: AgentProvider::CodexCli,
    model: "gpt-5.5",
  })
}

fn work() -> Arc<[ActiveWork]> {
  vec![
    active(
      "Kontur",
      "ORC-125",
      "Fix canvas zoom on trackpad",
      "Implement",
      "Builder",
    ),
    active(
      "Kontur",
      "ORC-127",
      "Improve mobile inspector",
      "Run checks",
      "Reviewer",
    ),
    active(
      "Website",
      "PW-14",
      "Compress preview images",
      "Deploy preview",
      "Builder",
    ),
  ]
  .into()
}

/// Counts presses of each button: wait, quit, cancel.
#[derive(Default)]
struct Presses([AtomicUsize; 3]);

impl Presses {
  fn actions(self: &Arc<Self>) -> CloseActions {
    let press = |index: usize| {
      let presses = Arc::clone(self);
      Arc::new(move || {
        presses.0[index].fetch_add(1, Ordering::SeqCst);
      }) as Arc<dyn Fn() + Send + Sync>
    };
    CloseActions {
      wait_for_agents: press(0),
      quit_now: press(1),
      cancel: press(2),
    }
  }

  fn count(&self, index: usize) -> usize {
    self.0[index].load(Ordering::SeqCst)
  }
}

fn app() -> App {
  let mut app = App::new();
  theme::install(&mut app);
  i18n::install(&app).expect("bundled locales load");
  app
}

fn props(stage: CloseStage, saves: Vec<SaveState>, presses: &Arc<Presses>) -> CloseDialogProps {
  CloseDialogProps {
    stage,
    work: work(),
    saves: saves.into(),
    actions: presses.actions(),
  }
}

fn mount(stage: CloseStage, saves: Vec<SaveState>, presses: &Arc<Presses>) -> Tree {
  let mut tree = Tree::new();
  tree.mount_root::<CloseDialog>(&mut app(), props(stage, saves, presses));
  tree
}

/// Width and left edge of the confirm dialog laid out in a window `width` wide.
fn dialog_bounds(width: u32) -> (f32, f32) {
  let mut app = app();
  let mut tree = Tree::new();
  tree.resize(width, 700);
  let presses = Arc::new(Presses::default());
  tree.mount_root::<CloseDialog>(&mut app, props(CloseStage::Confirm, Vec::new(), &presses));
  tree.pass_headless(&mut app);
  let bounds = tree
    .get_element_by_id_mut("close-dialog")
    .and_then(|element| element.bounds())
    .expect("the close dialog is laid out");
  (bounds.x, bounds.width)
}

fn contains_text(element: ElementRef<'_>, text: &str) -> bool {
  element.text_content() == Some(text) || element.children().into_iter().any(|child| contains_text(child, text))
}

/// Walks the built tree; no layout pass is needed for text and ids.
fn has_text(tree: &mut Tree, text: &str) -> bool {
  tree.root().is_some_and(|root| contains_text(root, text))
}

#[test]
fn close_dialog_confirm_lists_the_work_of_every_project_and_offers_three_choices() {
  let presses = Arc::new(Presses::default());
  let mut tree = mount(CloseStage::Confirm, Vec::new(), &presses);

  assert!(has_text(&mut tree, "Close Orchester while agents are working?"));
  assert!(has_text(&mut tree, "In progress · Kontur"));
  assert!(has_text(&mut tree, "In progress · Website"));
  for text in [
    "ORC-125",
    "Fix canvas zoom on trackpad",
    "Implement · Builder",
    "Codex · gpt-5.5",
    "Improve mobile inspector",
    "Deploy preview · Builder",
  ] {
    assert!(has_text(&mut tree, text), "row text {text}");
  }

  for (id, index) in [("close-wait", 0), ("close-quit-now", 1), ("close-cancel", 2)] {
    tree.get_element_by_id_mut(id).expect(id).click();
    assert_eq!(presses.count(index), 1, "{id} runs its action");
  }
  assert!(tree.get_element_by_id("close-waiting").is_none());
}

#[test]
fn close_dialog_waiting_shows_each_run_saving_or_saved_and_keeps_only_quit_now() {
  let presses = Arc::new(Presses::default());
  let saved = SaveState::Saved {
    checkpoint: Some("9d07b3f0a1b2c3d4e5f60718293a4b5c6d7e8f90".into()),
  };
  let mut tree = mount(
    CloseStage::Waiting,
    vec![SaveState::Saving, saved, SaveState::Saved { checkpoint: None }],
    &presses,
  );

  assert!(has_text(&mut tree, "Saving before closing"));
  assert!(has_text(&mut tree, "1 of 2 runs saved · Kontur"));
  assert!(has_text(&mut tree, "1 of 1 run saved · Website"));
  assert!(has_text(&mut tree, "Saving at next checkpoint…"));
  assert!(has_text(&mut tree, "Step still running"));
  assert!(has_text(&mut tree, "Paused at 9d07b3f"));
  assert!(has_text(&mut tree, "Paused"));
  assert!(has_text(&mut tree, "Closes after 1 more run"));
  assert!(tree.get_element_by_id("close-waiting").is_some());
  assert!(tree.get_element_by_id("close-cancel").is_none());
  assert!(tree.get_element_by_id("close-wait").is_none());

  tree.get_element_by_id_mut("close-quit-now").expect("quit now").click();
  assert_eq!(presses.count(1), 1);
}

#[test]
fn close_dialog_shares_the_lifecycle_width_and_keeps_its_side_margins() {
  assert_eq!(
    dialog_bounds(1440),
    ((1440.0 - 880.0) / 2.0, theme::LIFECYCLE_DIALOG_WIDTH)
  );
  assert_eq!(dialog_bounds(960).1, 880.0);
  assert_eq!(dialog_bounds(800), (32.0, 800.0 - 2.0 * 32.0));
}
