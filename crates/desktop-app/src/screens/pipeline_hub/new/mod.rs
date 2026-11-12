//! New pipeline in the hub (`Pipelines · v2` P14 `Rszkt`, P14a `d4f9Vi`, P14b `sVNK4`, P14c `g09dL`): *New pipeline* in
//! the head or the empty state, and the ⋯ menu's *Duplicate*, open an unsaved pipeline in the panel at once, listed
//! first under *New* ([`row`]). The panel ([`panel`]) has the name, where v1 starts from ([`start`]: the starter, a
//! copy of a pipeline or nothing), the starter's flow, the description and the slug ([`details`]), and the bar with
//! *Create v1* ([`bar`]), the only thing that stores it ([`create`]): a pipeline exists only as a saved version (D-032).
//!
//! The routes that begin one (`ProjectPage::NewPipeline`, `NewEmptyPipeline`, `CopyPipeline`) are replaced by the
//! hub's view of it, which names it by an id of its own that is never stored ([`Making`]): its steps open in the panel
//! through the same routes as a stored pipeline's, and the hub's question before leaving guards it as it guards an
//! edit. A route that no longer shows it drops it.

mod bar;
mod create;
mod details;
mod head;
mod panel;
mod reason;
mod row;
mod start;
mod words;

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  core::{Ref, Signal},
};
use orchester_core::id::ResourceId;
use orchester_workspace::pipelines::create::Start;

pub(super) use self::{
  panel::{Props, panel},
  row::section,
};
use super::{flow::copy_name, model::Listing};
use crate::{
  navigation::Navigation,
  routes::{PipelinesFilter, PipelinesView, ProjectPage, Route},
};

/// The pipeline being made: the id the hub's routes show it by while it is not stored, and the one it got once *Create
/// v1* stored it, until the hub lists that one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Making {
  /// Made for this panel and never stored: the created pipeline gets an id of its own.
  pub id: ResourceId,
  /// What the name field said when it opened: nothing, or the copy's *… copy*. A name changed from it is something
  /// typed, which leaving asks about.
  pub first_name: String,
  pub created: Option<ResourceId>,
}
impl DevtoolsInspectable for Making {}

/// Where v1 starts from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Choice {
  #[default]
  Starter,
  Copy,
  Empty,
}
impl DevtoolsInspectable for Choice {}

/// What the screen keeps of the pipeline being made, which its row and the question before leaving read: the panel
/// writes it, and it outlives the panel while a step of the pipeline is open.
#[derive(Clone)]
pub(super) struct NewPipeline {
  pub making: Signal<Option<Making>>,
  pub name: Signal<String>,
  pub choice: Signal<Choice>,
  /// The pipeline *Copy a pipeline* copies, by its id; empty while none is chosen.
  pub copy: Signal<String>,
  /// How many steps it has now, which its row says.
  pub steps: Signal<usize>,
  /// The route that begins one was followed: it is replaced, and a draw before that must not begin another.
  begun: Ref<bool>,
}

/// The screen's own signals, the same for as long as it lives.
impl PartialEq for NewPipeline {
  fn eq(&self, _: &Self) -> bool {
    true
  }
}

impl NewPipeline {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      making: ctx.signal(None),
      name: ctx.signal(String::new()),
      choice: ctx.signal(Choice::default()),
      copy: ctx.signal(String::new()),
      steps: ctx.signal(0),
      begun: ctx.create_ref(false),
    }
  }

  /// Follows the route: one that begins a pipeline (`begin`) makes it and is replaced by the hub's view of it; a view
  /// that no longer shows it drops it; once the pipeline *Create v1* stored is listed, the route opens it. Gives the
  /// pipeline being made while the route shows it.
  pub(super) fn follow(
    &self,
    ctx: &Ctx,
    (begin, view): (Option<Start>, PipelinesView),
    listing: &Listing,
    (navigation, project): (&Navigation, orchester_core::id::ProjectId),
  ) -> Option<Making> {
    match begin {
      Some(start) if !self.begun.get() => {
        self.begun.set(true);
        let id = self.begin(ctx, start, listing);
        navigation.replace(shows(project, id));
      }
      Some(_) => {}
      None => self.begun.set(false),
    }
    let making = self.making.get()?;
    if let Some(created) = making
      .created
      .filter(|created| listing.holds(PipelinesFilter::All, *created))
    {
      self.forget();
      navigation.replace(shows(project, created));
      return None;
    }
    if begin.is_none() && view.item != Some(making.id) {
      self.forget();
      return None;
    }
    Some(making)
  }

  /// Makes a pipeline that starts from `start`: a copy is named after what it copies, *… copy*.
  fn begin(&self, ctx: &Ctx, start: Start, listing: &Listing) -> ResourceId {
    let id = ResourceId::generate();
    let (choice, copy, name) = match start {
      Start::Starter => (Choice::Starter, String::new(), String::new()),
      Start::Empty => (Choice::Empty, String::new(), String::new()),
      Start::Copy(source) => {
        let name = listing
          .entry(source)
          .filter(|entry| entry.body.is_some())
          // A name at the limit cannot be extended: the copy's field starts on the name itself, which the slug refuses.
          .map(|entry| copy_name(ctx, &entry.name).map_or_else(|| entry.name.to_string(), |name| name.as_str().to_owned()));
        match name {
          Some(name) => (Choice::Copy, source.to_string(), name),
          None => (Choice::Copy, String::new(), String::new()),
        }
      }
    };
    self.choice.set(choice);
    self.copy.set(copy);
    self.name.set(name.clone());
    self.steps.set(0);
    self.making.set(Some(Making {
      id,
      first_name: name,
      created: None,
    }));
    id
  }

  /// Drops the pipeline being made, and what was typed for it.
  fn forget(&self) {
    self.making.set(None);
    self.name.set(String::new());
    self.copy.set(String::new());
    self.choice.set(Choice::default());
  }

  /// The name the question before leaving says for `id` when it is the pipeline being made: what is typed, or *New
  /// pipeline* while nothing is.
  pub(super) fn name_of(&self, ctx: &Ctx, id: ResourceId) -> Option<String> {
    self.making.get_untracked().filter(|making| making.id == id)?;
    let typed = self.name.get_untracked();
    Some(if typed.trim().is_empty() {
      ctx.t("pipeline_new.untitled").to_string()
    } else {
      typed.trim().to_owned()
    })
  }

  /// *Create v1* stored the pipeline as `created`: the panel waits until the hub lists it, then opens it.
  pub(super) fn created(&self, created: ResourceId) {
    self.making.update(|making| {
      if let Some(making) = making {
        making.created = Some(created);
      }
    });
  }
}

/// The hub's view of pipeline `id`.
fn shows(project: orchester_core::id::ProjectId, id: ResourceId) -> Route {
  Route::Project(
    project,
    ProjectPage::pipelines(PipelinesView::new(PipelinesFilter::All, Some(id))),
  )
}
