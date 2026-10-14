//! Add project (`Project setup · step 1` `kkGWj` and `W5PIR`, `vJTcd`, the detection states `gh5wV`): a page with
//! the setup rail at its start ([`rail`]) and the form of its first step — the project's name, the folder of an
//! existing git repository chosen in the system's folder picker ([`fields`]), and the GitHub repository detected in
//! that folder as soon as it is chosen ([`github`]) — then *Create project & continue* in the action bar ([`bar`]).
//! Create makes the project in the folder's `.orchester` and opens its Overview; the account `gh` acted as when the
//! folder was detected becomes the one the project expects. The later setup steps are not built: the rail says so.
//! Leaving discards what was entered. The rules are `orchester_workspace::project_setup`'s.

mod bar;
mod fields;
mod github;
mod rail;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::{Column, Row, ScrollVertical},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{connection::GitHubLogin, name::Name, repository::Detection};
use orchester_workspace::{
  background,
  messages::Message,
  project_setup::{self, CreateFailure, Creation},
  repository::{DetectTarget, detect},
  workspace::OpenProject,
};
use parking_lot::Mutex;

use crate::{
  navigation::Navigation,
  routes::{OverviewTab, ProjectPage, Route, Screen},
  services::{
    Services,
    path_picker::{PathKind, PathPicker},
  },
  theme::ProjectsSpacing,
};

/// The form's column (`Form` `IZHtk`).
const FORM_WIDTH: f32 = 640.0;

pub fn screen(ctx: &mut Ctx, folder: Option<PathBuf>) -> Element {
  ctx.mount_keyed::<AddProject>("add-project", Props(folder))
}

/// The folder the page starts with, e.g. one *Open folder…* found no project in.
#[derive(Clone, PartialEq)]
struct Props(Option<PathBuf>);

impl DevtoolsInspectable for Props {}

/// A detection, as the page's futures hold it.
#[derive(Clone, Debug, PartialEq)]
struct Detected(Detection);

impl DevtoolsInspectable for Detected {}

/// A path a picker gave; `None` when it was closed without one.
#[derive(Clone, Debug, PartialEq)]
struct Picked(Option<PathBuf>);

impl DevtoolsInspectable for Picked {}

/// The account the project will expect, if it expects one yet.
#[derive(Clone, Debug, Default, PartialEq)]
struct Expected(Option<GitHubLogin>);

impl DevtoolsInspectable for Expected {}

type PickAction = FutureAction<(PathKind, Arc<str>), Picked, Message>;
type DetectAction = FutureAction<DetectTarget, Detected, Message>;
type CreateAction = FutureAction<Creation, OpenProject, CreateFailure>;

/// The page's futures, asked for in this order in every render.
#[derive(Clone)]
struct Actions {
  folder: PickAction,
  program: PickAction,
  detect: DetectAction,
  create: CreateAction,
}

impl Actions {
  fn of(ctx: &mut Ctx) -> Self {
    let services = Services::of(ctx);
    let picker = PathPicker::of(ctx);
    Self {
      folder: pick_action(ctx, &picker),
      program: pick_action(ctx, &picker),
      detect: ctx
        .future_action(|target: DetectTarget| async move { background::run(move || Detected(detect(&target))).await }),
      create: ctx.future_action(move |creation: Creation| {
        let (workspace, activity) = (services.workspace.clone(), services.activity.clone());
        async move { project_setup::create(&workspace, &activity, creation).await }
      }),
    }
  }

  /// Detects the repository of `folder` with the `gh` chosen for it, if one was.
  fn detect(&self, folder: PathBuf, program: Option<PathBuf>) {
    self.detect.run(DetectTarget {
      repo_root: folder,
      connections: Vec::new(),
      program,
    });
  }

  /// The finished detection; `None` while it runs and before it ran.
  fn detection(&self) -> Option<Detection> {
    let state = self.detect.state().get();
    let pending = state.is_pending();
    state.data.filter(|_| !pending).map(|detected| detected.0)
  }
}

fn pick_action(ctx: &mut Ctx, picker: &PathPicker) -> PickAction {
  let picker = picker.clone();
  ctx.future_action(move |(kind, title): (PathKind, Arc<str>)| {
    let picker = picker.clone();
    async move { Ok(Picked(picker.pick(kind, title).await)) }
  })
}

/// What the person entered and chose.
#[derive(Clone)]
struct Form {
  name: Signal<String>,
  folder: Signal<Picked>,
  /// A `gh` chosen with *Choose program…* while none was found.
  program: Signal<Picked>,
  expected: Signal<Expected>,
  /// Whether *Change GitHub account or access* is open in the GitHub block.
  access: Signal<bool>,
  /// Whether Create was pressed: a name the rules reject is said from then on.
  pressed: Signal<bool>,
}

struct AddProject {
  form: Form,
  /// Whether the futures' ends are followed and the starting folder detected; once per page.
  started: Mutex<bool>,
}

impl Component for AddProject {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let folder = ctx.props::<Props>().0.clone();
    Self {
      form: Form {
        name: ctx.signal(String::new()),
        folder: ctx.signal(Picked(folder)),
        program: ctx.signal(Picked(None)),
        expected: ctx.signal(Expected::default()),
        access: ctx.signal(false),
        pressed: ctx.signal(false),
      },
      started: Mutex::new(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // First in every render: lurq keeps a component's futures by the order it asks for them.
    let actions = Actions::of(ctx);
    self.start(ctx, &actions);
    let detection = actions.detection();
    let form = Column::new()
      .id("add-project-form")
      .width(Dimension::Px(FORM_WIDTH))
      .spacing(SpacingSize::Xl)
      .child(fields::head(ctx, &self.form))
      .child(fields::name(ctx, &self.form))
      .child(fields::folder(ctx, &self.form, &actions))
      .with_children(github::block(ctx, &self.form, &actions, detection.as_ref()))
      .child(fields::ownership(ctx, &self.form));
    let scrolled = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(SpacingSize::Section)
          .bottom(ProjectsSpacing::Wide.role())
          .left(SpacingSize::Xl)
          .right(SpacingSize::Xl),
      )
      .align_items(Alignment::Center)
      .child(form);
    let main = Column::new()
      .flex(1.0)
      .height(Dimension::Pct(100.0))
      .child(ScrollVertical::new(scrolled).id("add-project-scroll").flex(1.0))
      .child(bar::bar(ctx, &self.form, &actions, detection));
    Row::new()
      .id(format!("screen-{}", Screen::AddProject.name()))
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(rail::rail(ctx))
      .child(main)
  }
}

impl AddProject {
  /// Follows the futures' ends and detects the folder the page started with; once.
  fn start(&self, ctx: &mut Ctx, actions: &Actions) {
    let mut started = self.started.lock();
    if *started {
      return;
    }
    *started = true;
    follow_folder(ctx, &self.form, actions);
    follow_program(ctx, &self.form, actions);
    let expected = self.form.expected.clone();
    ctx.watch(&actions.detect.state(), move |state| {
      if state.is_pending() || !state.is_fulfilled() {
        return;
      }
      if let Some(Detected(detection)) = &state.data {
        let current = expected.get_untracked().0;
        let next = project_setup::expected_account(current.as_ref(), detection);
        if next != current {
          expected.set(Expected(next));
        }
      }
    });
    // Created: the project opens on its Overview in the page's place, so Back does not return to setup.
    let navigation = Navigation::of(ctx);
    ctx.watch(&actions.create.state(), move |state| {
      if let Some(project) = state
        .data
        .as_ref()
        .filter(|_| state.is_fulfilled() && !state.is_pending())
      {
        navigation.replace(Route::Project(
          project.project.id(),
          ProjectPage::Overview(OverviewTab::default()),
        ));
      }
    });
    if let Some(folder) = self.form.folder.get_untracked().0 {
      actions.detect(folder, None);
    }
  }
}

/// A newly chosen folder is detected at once; closing the picker keeps the folder chosen before.
fn follow_folder(ctx: &mut Ctx, form: &Form, actions: &Actions) {
  let (folder, program, detect) = (form.folder.clone(), form.program.clone(), actions.clone());
  ctx.watch(&actions.folder.state(), move |state| {
    let (Some(Picked(Some(chosen))), true) = (&state.data, state.is_fulfilled() && !state.is_pending()) else {
      return;
    };
    if folder.get_untracked().0.as_ref() != Some(chosen) {
      folder.set(Picked(Some(chosen.clone())));
      detect.detect(chosen.clone(), program.get_untracked().0);
    }
  });
}

/// A chosen `gh` detects the folder again with it.
fn follow_program(ctx: &mut Ctx, form: &Form, actions: &Actions) {
  let (folder, program, detect) = (form.folder.clone(), form.program.clone(), actions.clone());
  ctx.watch(&actions.program.state(), move |state| {
    let (Some(Picked(Some(chosen))), true) = (&state.data, state.is_fulfilled() && !state.is_pending()) else {
      return;
    };
    if program.get_untracked().0.as_ref() == Some(chosen) {
      return;
    }
    program.set(Picked(Some(chosen.clone())));
    if let Some(folder) = folder.get_untracked().0 {
      detect.detect(folder, Some(chosen.clone()));
    }
  });
}

impl Form {
  /// The name the breadcrumb, the notes and the ownership say: the entered one once the rules accept it.
  fn shown_name(&self) -> Option<Name> {
    Name::new(&self.name.get()).ok()
  }
}
