//! The start-up recovery dialog (02.E `o8DCd`) rendered headlessly from fixture runs: all or
//! nothing, with no action per run. The fixtures are test data only.

use std::{
  path::PathBuf,
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
};

use lurq::{
  app::{App, Tree},
  node::element::ElementRef,
};
use orchester_core::{
  connection::AgentProvider,
  id::{RunId, TaskId},
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

fn run(project: &str, key: &str, cause: InterruptCause, worktree: WorktreeState) -> InterruptedRun {
  InterruptedRun {
    project_root: PathBuf::from(project),
    project: project.into(),
    run: RunId::generate(),
    task: TaskId::generate(),
    task_key: key.into(),
    task_title: "Fix canvas zoom on trackpad".into(),
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

/// Counts presses of each button: open log, don't continue, continue all.
#[derive(Default)]
struct Presses([AtomicUsize; 3]);

impl Presses {
  fn actions(self: &Arc<Self>) -> RecoveryActions {
    let press = |index: usize| {
      let presses = Arc::clone(self);
      Arc::new(move || {
        presses.0[index].fetch_add(1, Ordering::SeqCst);
      }) as Arc<dyn Fn() + Send + Sync>
    };
    RecoveryActions {
      open_log: press(0),
      dont_continue: press(1),
      continue_all: press(2),
    }
  }

  fn count(&self, index: usize) -> usize {
    self.0[index].load(Ordering::SeqCst)
  }
}

fn mount(previous: SessionEnd, runs: Vec<InterruptedRun>, continuing: bool, presses: &Arc<Presses>) -> Tree {
  let mut app = App::new();
  theme::install(&mut app);
  i18n::install(&app).expect("bundled locales load");
  let mut tree = Tree::new();
  tree.mount_root::<RecoveryDialog>(
    &mut app,
    RecoveryDialogProps {
      previous,
      runs: runs.into(),
      continuing,
      error: None,
      actions: presses.actions(),
    },
  );
  tree
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

#[test]
fn unit_input_shows_the_focused_field_stroke_while_focused() {
  assert_eq!(FieldStroke::of(false, false), FieldStroke::Default);
  assert_eq!(FieldStroke::of(false, true), FieldStroke::Focused);
  assert_eq!(FieldStroke::of(true, true), FieldStroke::Invalid);
  assert_eq!(FieldStroke::of(true, false), FieldStroke::Invalid);
}
