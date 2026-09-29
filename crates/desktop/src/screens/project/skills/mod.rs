//! Skills (ORC-15): the library (`qO1Az`) with its empty state (`YRaLK`), and one skill in its
//! editor (`ecJtg`). Each skill follows Draft saved → Publish vN → Published vN; drafts are saved as
//! they are edited. Import from repository opens over the screen
//! ([`super::repository_import`]).

mod copy;
mod editor;
mod library;
mod new_dialog;
mod row;

use lurq::{
  app::{component::Component, ctx::Ctx},
  components::{Column, Modal, ModalTarget},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ResourceId;

use self::{
  editor::SkillEditor,
  library::{Library, LibraryProps},
  new_dialog::{NewSkillDialog, NewSkillProps},
};
use super::ProjectNav;
use crate::{
  background,
  messages::{self, Message},
  navigation::route::{Page, Route},
  skills::data::{ProjectSkills, SkillEntry},
  ui::backdrop::backdrop,
  workspace::OpenProject,
};

/// Provided to the editor when it is mounted: what it edits and what it chooses from.
#[derive(Clone)]
pub struct EditorContext {
  pub project: OpenProject,
  pub data: ProjectSkills,
  pub entry: SkillEntry,
  pub nav: ProjectNav,
  /// Bumped after a publish, to read the skills again.
  pub revision: Signal<u64>,
  /// Whether the New skill dialog is open.
  pub creating: Signal<bool>,
}

/// The skills, and the revision they were read at.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Loaded {
  revision: u64,
  data: ProjectSkills,
}

impl lurq::app::component::DevtoolsInspectable for Loaded {}

pub struct SkillsScreen {
  revision: Signal<u64>,
  creating: Signal<bool>,
  /// Counts dialog openings, so every opening starts with an empty form.
  opened: Signal<u64>,
  nav: ProjectNav,
}

impl Component for SkillsScreen {
  type Props = OpenProject;

  fn create(ctx: &mut Ctx) -> Self {
    let creating = ctx.signal(false);
    let opened = ctx.signal(0_u64);
    let count = opened.clone();
    ctx.watch(&creating, move |open| {
      if *open {
        count.update(|opened| *opened += 1);
      }
    });
    Self {
      revision: ctx.signal(0),
      creating,
      opened,
      nav: ctx
        .use_context::<ProjectNav>()
        .expect("the project shell provides ProjectNav"),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let open = ctx.props::<OpenProject>().clone();
    let deps = (open.repo_root.clone(), self.revision.get());
    let load = ctx.future(deps, |(repo_root, revision)| async move {
      background::run(move || ProjectSkills::load_in(&repo_root))
        .await?
        .map(|data| Loaded { revision, data })
        .map_err(|error| {
          tracing::error!(%error, "cannot read the project's skills");
          messages::store_error(&error)
        })
    });
    let selected = match Route::current_page(ctx) {
      Some(Page::Skills(selected)) => selected,
      _ => None,
    };
    let state = load.state().get();
    let content = match (state.data, state.error) {
      (Some(loaded), None) => self.loaded(ctx, &open, loaded, selected),
      (_, error) => self.library(ctx, &open, None, error),
    };
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(content)
      .child(self.dialog(ctx, &open))
  }
}

impl SkillsScreen {
  fn loaded(&self, ctx: &mut Ctx, open: &OpenProject, loaded: Loaded, selected: Option<ResourceId>) -> Element {
    let Some(entry) = selected.and_then(|id| loaded.data.entry(id)).cloned() else {
      return self.library(ctx, open, Some(loaded.data), None);
    };
    let key = format!("skill-editor-{}-{}", entry.id, loaded.revision);
    ctx.provide(EditorContext {
      project: open.clone(),
      data: loaded.data,
      entry,
      nav: self.nav.clone(),
      revision: self.revision.clone(),
      creating: self.creating.clone(),
    });
    ctx.mount_keyed::<SkillEditor>(&key, ())
  }

  fn library(&self, ctx: &mut Ctx, open: &OpenProject, data: Option<ProjectSkills>, error: Option<Message>) -> Element {
    ctx.mount_keyed::<Library>(
      "skills-library",
      LibraryProps {
        project: open.clone(),
        data,
        error,
        creating: self.creating.clone(),
      },
    )
  }

  fn dialog(&self, ctx: &mut Ctx, open: &OpenProject) -> Element {
    if !self.creating.get() {
      return Column::new().into();
    }
    let props = NewSkillProps {
      project: open.clone(),
      open: self.creating.clone(),
      revision: self.revision.clone(),
    };
    let key = format!("new-skill-{}", self.opened.get());
    let dialog = ctx.mount_keyed::<NewSkillDialog>(&key, props);
    Modal::new(backdrop(ctx, dialog))
      .target(ModalTarget::Root)
      .open(self.creating.clone())
      .into()
  }
}
