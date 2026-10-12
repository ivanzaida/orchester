//! The start-up recovery dialog (02.E `o8DCd`) rendered headlessly from fixture runs: all or
//! nothing, with no action per run; uncommitted changes link to the run's Changes tab. The
//! fixtures are test data only.

extern crate lurq_legacy as lurq;

use std::{
  path::PathBuf,
  sync::{
    Arc, OnceLock,
    atomic::{AtomicUsize, Ordering},
  },
};

use lurq::{
  app::{App, Tree},
  core::ElementRect,
  node::element::ElementRef,
};
use orchester_core::{
  connection::AgentProvider,
  id::{ProjectId, RunId, TaskId},
  run::InterruptCause,
  time::Timestamp,
};
use orchester_desktop::{
  FieldStroke, i18n,
  recovery::{InterruptedRun, WorktreeState},
  recovery_dialog::{RecoveryActions, RecoveryDialog, RecoveryDialogProps},
  theme,
};
use orchester_ipc::SessionEnd;
use parking_lot::Mutex;

/// The project the dialog's window shows in these tests.
fn shown_project() -> ProjectId {
  static SHOWN: OnceLock<ProjectId> = OnceLock::new();
  *SHOWN.get_or_init(ProjectId::generate)
}

fn run(project: &str, key: &str, cause: InterruptCause, worktree: WorktreeState) -> InterruptedRun {
  InterruptedRun {
    project_id: shown_project(),
    project_root: PathBuf::from(project),
    project: project.into(),
    run: RunId::generate(),
    task: TaskId::generate(),
    task_key: key.parse().expect("a task key"),
    task_title: "Fix canvas zoom on trackpad".into(),
    uncommitted_file: None,
    number: 129,
    step: Some("Implement".into()),
    agent: Some("Builder".into()),
    provider: Some(AgentProvider::CodexCli),
    model: Some("gpt-5.5".into()),
    cause,
    checkpoint: Some(("4c1e2a9".into(), Timestamp::now())),
    worktree,
  }
}

/// Counts presses of each button: open log, don't continue, continue all; and records the runs
/// whose changes were opened.
#[derive(Default)]
struct Presses([AtomicUsize; 3], Mutex<Vec<RunId>>);

impl Presses {
  fn actions(self: &Arc<Self>) -> RecoveryActions {
    let press = |index: usize| {
      let presses = Arc::clone(self);
      Arc::new(move || {
        presses.0[index].fetch_add(1, Ordering::SeqCst);
      }) as Arc<dyn Fn() + Send + Sync>
    };
    let presses = Arc::clone(self);
    RecoveryActions {
      open_log: press(0),
      dont_continue: press(1),
      continue_all: press(2),
      view_changes: Arc::new(move |run: &InterruptedRun| presses.1.lock().push(run.run)),
    }
  }

  fn count(&self, index: usize) -> usize {
    self.0[index].load(Ordering::SeqCst)
  }
}

fn props(
  previous: SessionEnd,
  runs: Vec<InterruptedRun>,
  continuing: bool,
  presses: &Arc<Presses>,
) -> RecoveryDialogProps {
  RecoveryDialogProps {
    previous,
    runs: runs.into(),
    shown_project: Some(shown_project()),
    continuing,
    error: None,
    actions: presses.actions(),
  }
}

fn app() -> App {
  let mut app = App::new();
  theme::install(&mut app);
  i18n::install(&app).expect("bundled locales load");
  app
}

fn mount(previous: SessionEnd, runs: Vec<InterruptedRun>, continuing: bool, presses: &Arc<Presses>) -> Tree {
  let mut tree = Tree::new();
  tree.mount_root::<RecoveryDialog>(&mut app(), props(previous, runs, continuing, presses));
  tree
}

/// The dialog laid out in a window of `width` x `height`.
fn laid_out(runs: Vec<InterruptedRun>, width: u32, height: u32) -> Tree {
  let mut app = app();
  let mut tree = Tree::new();
  tree.resize(width, height);
  let presses = Arc::new(Presses::default());
  tree.mount_root::<RecoveryDialog>(&mut app, props(SessionEnd::QuitNow, runs, false, &presses));
  tree.pass_headless(&mut app);
  tree
}

fn bounds(tree: &mut Tree, id: &str) -> ElementRect {
  tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
}

fn contains_text(element: ElementRef<'_>, text: &str) -> bool {
  element.text_content() == Some(text) || element.children().into_iter().any(|child| contains_text(child, text))
}

fn has_text(tree: &mut Tree, text: &str) -> bool {
  tree.root().is_some_and(|root| contains_text(root, text))
}

#[test]
fn recovery_dialog_lists_every_interrupted_run_and_answers_all_or_nothing() {
  let presses = Arc::new(Presses::default());
  let runs = vec![
    run(
      "Kontur",
      "ORC-125",
      InterruptCause::QuitNow,
      WorktreeState::Uncommitted(3),
    ),
    run("Kontur", "ORC-127", InterruptCause::QuitNow, WorktreeState::Clean),
    run(
      "Website",
      "PW-14",
      InterruptCause::RunnerLost,
      WorktreeState::Uncommitted(1),
    ),
  ];
  let ids: Vec<String> = runs.iter().map(|run| format!("recovery-run-{}", run.run)).collect();
  let mut tree = mount(SessionEnd::QuitNow, runs, false, &presses);

  for text in [
    "The last session ended before agents could save",
    "Interrupted during",
    "Kontur · Run #129",
    "Website · Run #129",
    "Builder · Codex · gpt-5.5",
    "Stopped with the app",
    "The runner stopped unexpectedly",
    "4c1e2a9",
    "3 files not committed",
    "1 file not committed",
    "No uncommitted changes",
    "Continue all 3 from checkpoints",
    "Don't continue",
  ] {
    assert!(has_text(&mut tree, text), "{text}");
  }
  assert!(has_text(
    &mut tree,
    "Orchester was closed with Quit now and 3 runs were interrupted. Nothing has been restarted. Continue all \
     of them from their last checkpoints, or none; work after a checkpoint is not counted as done."
  ));
  // Rows have no action of their own: the only buttons are the dialog's three.
  for id in &ids {
    assert!(tree.get_element_by_id(id).is_some(), "{id}");
  }
  for (id, index) in [
    ("recovery-open-log", 0),
    ("recovery-dont-continue", 1),
    ("recovery-continue-all", 2),
  ] {
    tree.get_element_by_id_mut(id).expect(id).click();
    assert_eq!(presses.count(index), 1, "{id} runs its action");
  }
}

#[test]
fn recovery_dialog_after_a_crash_names_one_run_and_shows_continuing() {
  let presses = Arc::new(Presses::default());
  let runs = vec![run(
    "Kontur",
    "ORC-125",
    InterruptCause::Crashed,
    WorktreeState::Unknown,
  )];
  let mut tree = mount(SessionEnd::Crashed, runs, true, &presses);
  assert!(has_text(
    &mut tree,
    "Orchester stopped unexpectedly and 1 run was interrupted. Nothing has been restarted. Continue it from \
     its last checkpoint, or leave it interrupted; work after a checkpoint is not counted as done."
  ));
  assert!(has_text(&mut tree, "Continuing…"));
  // While continuing, neither answer can be given again.
  tree
    .get_element_by_id_mut("recovery-continue-all")
    .expect("continue all")
    .click();
  tree
    .get_element_by_id_mut("recovery-dont-continue")
    .expect("don't continue")
    .click();
  assert_eq!(presses.count(2), 0);
  assert_eq!(presses.count(1), 0);
}

/// *View changes* (`O5D84`) is under uncommitted changes only, and only for runs of the project
/// the window shows: another project's task page cannot be opened yet.
#[test]
fn recovery_dialog_links_uncommitted_changes_of_the_shown_project() {
  let presses = Arc::new(Presses::default());
  let uncommitted = run(
    "Kontur",
    "ORC-125",
    InterruptCause::QuitNow,
    WorktreeState::Uncommitted(3),
  );
  let clean = run("Kontur", "ORC-127", InterruptCause::QuitNow, WorktreeState::Clean);
  let elsewhere = InterruptedRun {
    project_id: ProjectId::generate(),
    ..run(
      "Website",
      "PW-14",
      InterruptCause::QuitNow,
      WorktreeState::Uncommitted(1),
    )
  };
  let link = |run: &InterruptedRun| format!("recovery-view-changes-{}", run.run);
  let (linked, unlinked) = (link(&uncommitted), [link(&clean), link(&elsewhere)]);
  let target = uncommitted.run;
  let mut tree = mount(
    SessionEnd::QuitNow,
    vec![uncommitted, clean, elsewhere],
    false,
    &presses,
  );

  let view = tree
    .get_element_by_id(&linked)
    .expect("View changes under 3 files not committed");
  assert!(contains_text(view, "View changes  →"));
  for id in &unlinked {
    assert!(tree.get_element_by_id(id).is_none(), "{id}");
  }
  tree.get_element_by_id_mut(&linked).expect("View changes").click();
  assert_eq!(*presses.1.lock(), [target]);
  // Opening the changes answers nothing itself.
  assert_eq!((presses.count(1), presses.count(2)), (0, 0));
}

#[test]
fn recovery_dialog_is_as_wide_as_the_close_dialogs_in_the_design_window() {
  let mut long = run("Kontur", "ORC-131", InterruptCause::QuitNow, WorktreeState::Clean);
  long.task_title = "Rework the canvas zoom and pan handling for trackpads, mice and touch screens".into();
  let row = format!("recovery-run-{}", long.run);
  let mut tree = laid_out(vec![long], 1440, 960);

  let dialog = bounds(&mut tree, "recovery-dialog");
  assert_eq!(dialog.width, theme::LIFECYCLE_DIALOG_WIDTH);
  assert_eq!(dialog.width, 880.0);
  assert_eq!(dialog.x, (1440.0 - 880.0) / 2.0, "centred");
  // A long task title ends in an ellipsis instead of widening the table past the body padding.
  let row = bounds(&mut tree, &row);
  assert_eq!(row.x, dialog.x + 28.0);
  assert_eq!(row.width, 880.0 - 2.0 * 28.0);
}

fn inside(inner: ElementRect, outer: ElementRect) -> bool {
  inner.x >= outer.x - 0.5 && inner.x + inner.width <= outer.x + outer.width + 0.5
}

/// Beside a long task title, every row's last column, the worktree state, stays inside the row and the
/// dialog, with one row and with several, at the narrowest window and at a wide one.
#[test]
fn recovery_dialog_keeps_the_worktree_column_inside_beside_a_long_title() {
  let mut long = run(
    "Kontur",
    "ORC-65",
    InterruptCause::QuitNow,
    WorktreeState::Uncommitted(3),
  );
  long.task_title = "Close dialog: the run state badge runs past the dialog's right edge when the task title is this \
                     long"
    .into();
  let others = [
    run("Kontur", "ORC-127", InterruptCause::QuitNow, WorktreeState::Clean),
    run("Website", "PW-14", InterruptCause::RunnerLost, WorktreeState::Unknown),
  ];
  let several: Vec<InterruptedRun> = std::iter::once(long.clone()).chain(others).collect();
  for runs in [vec![long], several] {
    for width in [960, 1440, 2160] {
      let mut tree = laid_out(runs.clone(), width, 960);
      let dialog = bounds(&mut tree, "recovery-dialog");
      for run in &runs {
        let row = bounds(&mut tree, &format!("recovery-run-{}", run.run));
        let state = bounds(&mut tree, &format!("recovery-worktree-label-{}", run.run));
        let key = &run.task_key;
        assert!(inside(row, dialog), "{key} at {width}: the row is inside the dialog");
        assert!(
          inside(state, row),
          "{key} at {width}: the worktree state is inside its row ({state:?}, {row:?})"
        );
      }
    }
  }
}

#[test]
fn recovery_dialog_keeps_its_side_margins_in_a_narrow_window() {
  let runs = vec![run(
    "Kontur",
    "ORC-125",
    InterruptCause::QuitNow,
    WorktreeState::Uncommitted(3),
  )];
  // At the smallest window the app allows, 960, the dialog keeps its full width.
  let mut tree = laid_out(runs.clone(), 960, 700);
  assert_eq!(bounds(&mut tree, "recovery-dialog").width, 880.0);
  // Narrower than 880 plus two 32px margins, it shrinks and keeps 32 from either edge.
  let mut tree = laid_out(runs, 800, 700);
  let dialog = bounds(&mut tree, "recovery-dialog");
  assert_eq!(dialog.x, 32.0);
  assert_eq!(dialog.width, 800.0 - 2.0 * 32.0);
}

/// A worktree state is top-aligned (`Py65X`): its 12px icon sits centred in a 16px slot that
/// lines up with the first line of the label (Inter 11, 16.5px lines), whether the label takes
/// one line or wraps to two.
#[test]
fn recovery_dialog_keeps_the_worktree_icon_with_the_first_line() {
  let unreadable = run("Website", "PW-14", InterruptCause::QuitNow, WorktreeState::Unknown);
  let clean = run("Kontur", "ORC-127", InterruptCause::QuitNow, WorktreeState::Clean);
  let ids = [unreadable.run, clean.run];
  let mut tree = laid_out(vec![unreadable, clean], 1440, 960);
  let mut lines = Vec::new();
  for id in ids {
    let slot = bounds(&mut tree, &format!("recovery-worktree-icon-{id}"));
    let label = bounds(&mut tree, &format!("recovery-worktree-label-{id}"));
    assert_eq!(slot.height, 16.0);
    assert_eq!(slot.y, label.y, "the state row is top-aligned");
    let first_line_middle = label.y + 16.5 / 2.0;
    assert!((slot.y + slot.height / 2.0 - first_line_middle).abs() <= 0.5);
    lines.push((label.height / 16.5).round());
  }
  assert_eq!(lines, [2.0, 1.0], "“Could not read the worktree” wraps beside its icon");
}

#[test]
fn unit_input_shows_the_focused_field_stroke_while_focused() {
  assert_eq!(FieldStroke::of(false, false), FieldStroke::Default);
  assert_eq!(FieldStroke::of(false, true), FieldStroke::Focused);
  assert_eq!(FieldStroke::of(true, true), FieldStroke::Invalid);
  assert_eq!(FieldStroke::of(true, false), FieldStroke::Invalid);
}
