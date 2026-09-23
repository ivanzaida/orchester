//! The close dialog rendered headlessly from fixture work. Runs do not exist yet, so the
//! running app cannot reach this dialog; these fixtures are test data only.

use std::sync::{
  Arc,
  atomic::{AtomicUsize, Ordering},
};

use lurq::{
  app::{App, Tree},
  node::element::ElementRef,
};
use orchester_desktop::{
  close_dialog::{CloseActions, CloseDialog, CloseDialogProps, CloseStage},
  i18n,
  runner::{ActiveWork, ActiveWorkFields},
  theme,
};

fn work() -> Arc<[ActiveWork]> {
  vec![
    ActiveWork::new(&ActiveWorkFields {
      project: "Kontur",
      task_key: "ORC-125",
      task_title: "Fix canvas zoom on trackpad",
      step: "Implement",
      agent: "Builder",
      model: "Codex · Sol",
    }),
    ActiveWork::new(&ActiveWorkFields {
      project: "Kontur",
      task_key: "ORC-127",
      task_title: "Improve mobile inspector",
      step: "Run checks",
      agent: "Reviewer",
      model: "Claude · Opus",
    }),
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

fn mount(stage: CloseStage, presses: &Arc<Presses>) -> Tree {
  let mut app = App::new();
  theme::install(&mut app);
  i18n::install(&app).expect("bundled locales load");
  let mut tree = Tree::new();
  tree.mount_root::<CloseDialog>(
    &mut app,
    CloseDialogProps {
      stage,
      work: work(),
      actions: presses.actions(),
    },
  );
  tree
}

fn contains_text(element: ElementRef<'_>, text: &str) -> bool {
  element.text_content() == Some(text) || element.children().into_iter().any(|child| contains_text(child, text))
}

/// Walks the built tree; no layout pass is needed for text and ids.
fn has_text(tree: &mut Tree, text: &str) -> bool {
  tree.root().is_some_and(|root| contains_text(root, text))
}

#[test]
fn close_dialog_confirm_lists_the_work_and_offers_three_choices() {
  let presses = Arc::new(Presses::default());
  let mut tree = mount(CloseStage::Confirm, &presses);

  assert!(has_text(&mut tree, "Close Orchester while agents are working?"));
  assert!(has_text(&mut tree, "In progress · Kontur"));
  for text in [
    "ORC-125",
    "Fix canvas zoom on trackpad",
    "Implement · Builder",
    "Codex · Sol",
  ] {
    assert!(has_text(&mut tree, text), "row text {text}");
  }
  assert!(has_text(&mut tree, "Improve mobile inspector"));

  for (id, index) in [("close-wait", 0), ("close-quit-now", 1), ("close-cancel", 2)] {
    tree.get_element_by_id_mut(id).expect(id).click();
    assert_eq!(presses.count(index), 1, "{id} runs its action");
  }
  assert!(tree.get_element_by_id("close-waiting").is_none());
}

#[test]
fn close_dialog_waiting_keeps_only_quit_now() {
  let presses = Arc::new(Presses::default());
  let mut tree = mount(CloseStage::Waiting, &presses);

  assert!(has_text(&mut tree, "Saving before closing"));
  assert!(has_text(&mut tree, "Saving 2 runs · Kontur"));
  assert!(has_text(&mut tree, "Closes when every run is saved"));
  assert!(tree.get_element_by_id("close-waiting").is_some());
  assert!(tree.get_element_by_id("close-cancel").is_none());
  assert!(tree.get_element_by_id("close-wait").is_none());

  tree.get_element_by_id_mut("close-quit-now").expect("quit now").click();
  assert_eq!(presses.count(1), 1);
}
