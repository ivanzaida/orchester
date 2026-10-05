//! The close dialog rendered headlessly from fixture work (02.C `Xz30C`, 02.D `a0tTV`). The
//! fixtures are test data only.

use std::sync::{
  Arc,
  atomic::{AtomicUsize, Ordering},
};

use lurq::{
  app::{App, Tree},
  core::ElementRect,
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
  on_model(project, task_key, task_title, (step, agent), "gpt-5.5")
}

fn on_model(project: &str, task_key: &str, task_title: &str, (step, agent): (&str, &str), model: &str) -> ActiveWork {
  ActiveWork::new(&ActiveWorkFields {
    project,
    run: RunId::generate(),
    task_key,
    task_title,
    step,
    agent,
    provider: AgentProvider::CodexCli,
    model,
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

/// The fixture work with a first row whose task title is far longer than its cell (ORC-65 as seen
/// 2026-10-05), so that several rows share the columns.
fn long_work() -> Arc<[ActiveWork]> {
  let long = active(
    "Kontur",
    "ORC-65",
    "Close dialog: the run state badge runs past the dialog's right edge when the task title is this long",
    "Design",
    "Designer",
  );
  std::iter::once(long).chain(work().iter().cloned()).collect()
}

/// The dialog laid out in a window `width` wide.
fn laid_out(stage: CloseStage, work: Arc<[ActiveWork]>, saves: Vec<SaveState>, width: u32) -> Tree {
  let mut app = app();
  let mut tree = Tree::new();
  tree.resize(width, 960);
  let presses = Arc::new(Presses::default());
  let props = CloseDialogProps {
    work,
    ..props(stage, saves, &presses)
  };
  tree.mount_root::<CloseDialog>(&mut app, props);
  tree.pass_headless(&mut app);
  tree
}

fn bounds(tree: &mut Tree, id: &str) -> ElementRect {
  tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
}

fn inside(inner: ElementRect, outer: ElementRect) -> bool {
  inner.x >= outer.x - 0.5
    && inner.x + inner.width <= outer.x + outer.width + 0.5
    && inner.y >= outer.y - 0.5
    && inner.y + inner.height <= outer.y + outer.height + 0.5
}

/// Each row keeps its status badge and step label inside the dialog and as wide as with a short
/// title (`whole`, by run), while a long title ends before the step label.
fn assert_rows_fit(tree: &mut Tree, work: &[ActiveWork], whole: &[(f32, f32)], view: &str) {
  let dialog = bounds(tree, "close-dialog");
  for (work, (step_whole, status_whole)) in work.iter().zip(whole) {
    let run = work.run();
    let row = bounds(tree, &format!("close-row-{run}"));
    let title = bounds(tree, &format!("close-title-{run}"));
    let step = bounds(tree, &format!("close-step-{run}"));
    let status = bounds(tree, &format!("close-status-{run}"));
    let last = bounds(tree, &format!("close-last-{run}"));
    let key = work.task_key();
    assert!(
      inside(row, dialog),
      "{view}: {key}'s row is inside the dialog ({row:?}, {dialog:?})"
    );
    assert!(
      inside(status, dialog),
      "{view}: {key}'s badge is inside the dialog ({status:?}, {dialog:?})"
    );
    assert!(
      inside(last, row),
      "{view}: {key}'s last cell is inside its row ({last:?}, {row:?})"
    );
    assert!(
      title.x + title.width <= step.x + 0.5,
      "{view}: {key}'s title ends before its step"
    );
    assert!(
      step.x + step.width <= status.x + 0.5,
      "{view}: {key}'s step ends before its badge"
    );
    assert!(
      (step.width - step_whole).abs() <= 0.5,
      "{view}: {key}'s step label is whole"
    );
    assert!(
      (status.width - status_whole).abs() <= 0.5,
      "{view}: {key}'s badge is whole"
    );
  }
}

/// The step label's and the badge's widths of each row of `work` with short titles only.
fn whole_widths(stage: CloseStage, work: &[ActiveWork], saves: &[SaveState]) -> Vec<(f32, f32)> {
  let short: Arc<[ActiveWork]> = work
    .iter()
    .map(|work| active(work.project(), work.task_key(), "Short", work.step(), work.agent()))
    .collect();
  let mut tree = laid_out(stage, short.clone(), saves.to_vec(), 1440);
  short
    .iter()
    .map(|work| {
      let run = work.run();
      let step = bounds(&mut tree, &format!("close-step-{run}")).width;
      (step, bounds(&mut tree, &format!("close-status-{run}")).width)
    })
    .collect()
}

#[test]
fn close_dialog_keeps_the_badge_and_step_whole_beside_a_long_title() {
  let saving = SaveState::Saved { checkpoint: None };
  for (stage, saves) in [
    (CloseStage::Confirm, Vec::new()),
    (
      CloseStage::Waiting,
      vec![SaveState::Saving, saving.clone(), SaveState::Saving, saving],
    ),
  ] {
    let work = long_work();
    let whole = whole_widths(stage, &work, &saves);
    for width in [960, 1440, 2160] {
      let view = format!("{stage:?} at {width}");
      let mut tree = laid_out(stage, work.clone(), saves.clone(), width);
      assert_rows_fit(&mut tree, &work, &whole, &view);
      // The one row on its own fits the same way.
      let mut tree = laid_out(stage, work[..1].into(), saves.iter().take(1).cloned().collect(), width);
      assert_rows_fit(&mut tree, &work[..1], &whole[..1], &view);
    }
  }
}

/// Rows with titles and model ids of different lengths: one far longer than its cell, short ones
/// of different lengths, and a model id wider than its cell.
fn mixed_work() -> Arc<[ActiveWork]> {
  let long_model = on_model(
    "Website",
    "PW-15",
    "Fix",
    ("Implement", "Builder"),
    "claude-opus-5-5-with-a-long-model-id",
  );
  long_work().iter().cloned().chain(std::iter::once(long_model)).collect()
}

#[test]
fn close_dialog_lines_up_the_step_status_and_last_columns_of_every_row() {
  let saved = SaveState::Saved { checkpoint: None };
  for (stage, saves) in [
    (CloseStage::Confirm, Vec::new()),
    (
      CloseStage::Waiting,
      vec![
        SaveState::Saving,
        saved.clone(),
        SaveState::Saving,
        saved.clone(),
        saved,
      ],
    ),
  ] {
    for width in [960, 1440, 2160] {
      let work = mixed_work();
      let mut tree = laid_out(stage, work.clone(), saves.clone(), width);
      // Where each cell starts, and where the title ends; the badge's own width follows its word.
      let columns = |tree: &mut Tree, run| {
        let title = bounds(tree, &format!("close-title-{run}"));
        let [step, status, last] =
          ["close-step", "close-status", "close-last"].map(|cell| bounds(tree, &format!("{cell}-{run}")).x);
        [title.x, title.x + title.width, step, status, last]
      };
      let first = columns(&mut tree, work[0].run());
      for work in &work[1..] {
        let row = columns(&mut tree, work.run());
        assert!(
          row.iter().zip(first).all(|(x, first_x)| (x - first_x).abs() <= 0.5),
          "{stage:?} at {width}: {}'s columns line up with the first row's ({row:?}, {first:?})",
          work.task_key()
        );
      }
    }
  }
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
