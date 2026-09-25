//! GitHub in project setup (01.03 `W6uXiL`, states 01.C1–C10): the chosen folder's repository
//! is detected as soon as the folder is chosen and again on *Check again*; the account `gh` acts
//! as at the first detection becomes the one the project expects (*Use this account* replaces
//! it), and *Change GitHub account or access* opens 01.D. Nothing is stored until Create.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::{
  app::ctx::Ctx,
  components::{Column, Modal, ModalTarget},
  core::Signal,
  node::Element,
};
use orchester_core::{connection::GitHubLogin, id::ConnectionId, repository::Detection};

use crate::{
  repository::DetectTarget,
  screens::github::{
    account_panel::{AccountPanel, AccountPanelProps},
    actions::{ChosenPath, DetectAction, Expected, ProgramAction, detected_connection},
    detection::{DetectionActions, DetectionBlock, DetectionPlace},
  },
  ui::command_row::Copied,
};

type Handler = Arc<dyn Fn() + Send + Sync>;

/// GitHub state of the setup form.
#[derive(Clone)]
pub struct SetupGitHub {
  /// A `gh` chosen with *Choose program…* (gh not found).
  program: Signal<Option<ChosenPath>>,
  /// The account the new project will expect.
  expected: Signal<Expected>,
  copied: Signal<Copied>,
  panel: Signal<bool>,
  /// Stands for the connection Create would add, while the account panel shows it.
  panel_connection: ConnectionId,
}

impl SetupGitHub {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      program: ctx.signal(None),
      expected: ctx.signal(Expected::default()),
      copied: ctx.signal(Copied::default()),
      panel: ctx.signal(false),
      panel_connection: ConnectionId::generate(),
    }
  }

  pub fn expected(&self) -> Option<GitHubLogin> {
    self.expected.get().0
  }

  /// A new detection pins the account `gh` acts as, unless the project expects one already; a
  /// chosen program starts a new detection of `folder`.
  pub fn watch(
    &self,
    ctx: &mut Ctx,
    detect: &DetectAction,
    program: &ProgramAction,
    folder: Signal<Option<ChosenPath>>,
  ) {
    let expected = self.expected.clone();
    ctx.watch(&detect.state(), move |state| {
      let account = state
        .data
        .as_ref()
        .and_then(|detected| detected.0.github())
        .and_then(|found| found.account());
      if let (None, Some(account)) = (expected.get_untracked().0, account) {
        expected.set(Expected(Some(account.clone())));
      }
    });
    let (chosen, detect) = (self.program.clone(), detect.clone());
    ctx.watch(&program.state(), move |state| {
      let (Some(Some(path)), false) = (&state.data, state.is_pending()) else {
        return;
      };
      if chosen.get_untracked().as_ref() == Some(path) {
        return;
      }
      chosen.set(Some(path.clone()));
      if let Some(folder) = folder.get_untracked() {
        detect.run(setup_target(folder.0, Some(path.0.clone())));
      }
    });
  }

  /// Detects the repository of `folder` again.
  pub fn check(&self, detect: &DetectAction, folder: PathBuf) -> Handler {
    let (detect, program) = (detect.clone(), self.program.clone());
    Arc::new(move || {
      let program = program.get_untracked().map(|program| program.0);
      detect.run(setup_target(folder.clone(), program));
    })
  }

  /// The detected repository block, below the folder field.
  pub fn block(
    &self,
    ctx: &Ctx,
    detect: &DetectAction,
    program: &ProgramAction,
    folder: &Path,
    project: &str,
  ) -> Element {
    let state = detect.state().get();
    let detection = state
      .data
      .as_ref()
      .filter(|_| !state.is_pending())
      .map(|detected| &detected.0);
    let expected = self.expected.get().0;
    let idle = detection.is_some() && !program.state().get().is_pending();
    let handler = |make: &dyn Fn() -> Handler| idle.then(make);
    let actions = DetectionActions {
      check: handler(&|| self.check(detect, folder.to_path_buf())),
      change_account: handler(&|| {
        let panel = self.panel.clone();
        Arc::new(move || panel.set(true))
      }),
      choose_program: handler(&|| {
        let (program, title) = (program.clone(), ctx.t("github.dialog.chooseProgram"));
        Arc::new(move || program.run(title.clone()))
      }),
      use_account: handler(&|| {
        let expected = self.expected.clone();
        let active = detection
          .and_then(Detection::github)
          .and_then(|found| found.account())
          .cloned();
        Arc::new(move || expected.set(Expected(active.clone())))
      }),
      add_connection: None,
      link: None,
    };
    DetectionBlock {
      detection,
      expected: expected.as_ref(),
      project,
      folder,
      place: DetectionPlace::Setup,
      copied: &self.copied,
    }
    .render(ctx, &actions)
  }

  /// Change GitHub account or access (01.D) for the `gh` detection asked, while it is open.
  pub fn panel(&self, ctx: &mut Ctx, detect: &DetectAction, folder: Option<PathBuf>, project: &str) -> Element {
    let closed = || Modal::new(Column::new()).open_when(false).into();
    if !self.panel.get() {
      return closed();
    }
    let state = detect.state().get();
    let Some(connection) = state
      .data
      .as_ref()
      .and_then(|detected| detected_connection(ctx, &detected.0, self.panel_connection))
    else {
      return closed();
    };
    let check = folder
      .filter(|_| !state.is_pending())
      .map(|folder| self.check(detect, folder));
    let panel = self.panel.clone();
    let props = AccountPanelProps {
      connection,
      expected: self.expected.get().0,
      project: project.into(),
      check,
      close: Arc::new(move || panel.set(false)),
    };
    let content = ctx.mount_keyed::<AccountPanel>("setup-github-account", props);
    Modal::new(content)
      .target(ModalTarget::Root)
      .dismiss_on_escape(false)
      .into()
  }
}

fn setup_target(folder: PathBuf, program: Option<PathBuf>) -> DetectTarget {
  DetectTarget {
    repo_root: folder,
    connections: Vec::new(),
    program,
  }
}
