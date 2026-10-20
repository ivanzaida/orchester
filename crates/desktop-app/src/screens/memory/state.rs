//! What a Memory screen keeps while it shows: its filters and search, which record is open in an editor or a
//! delete confirmation, which histories are unfolded, the record its inspector shows, and the change under way.
//! The task's Memory tab and the project's Memory page each provide one to the parts they mount.

use std::{collections::BTreeSet, path::PathBuf, sync::Arc};

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  core::Signal,
};
use orchester_core::{
  id::{MemoryId, ProjectId},
  project::TaskKey,
};
use orchester_workspace::{memory::listing::Filters, secrets::Vault};

use super::work::{self, Start, Write};
use crate::project::ProjectFeed;

/// What is open in place of a record or above the list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Opened {
  #[default]
  Nothing,
  /// *New record*, above the list.
  New,
  /// A proposal's *Review proposed record* editor, in its place.
  Review(MemoryId),
  /// A record's *Edit* editor, in its place.
  Edit(MemoryId),
  /// A record's *Delete this record?*, in its place.
  Delete(MemoryId),
}

impl DevtoolsInspectable for Opened {}

/// Records whose earlier versions are unfolded.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Unfolded(pub BTreeSet<MemoryId>);

impl DevtoolsInspectable for Unfolded {}

/// The record the project's inspector shows, if a press chose one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Chosen(pub Option<MemoryId>);

impl DevtoolsInspectable for Chosen {}

/// The record the last change asked for was of, once one was asked for (`Some(None)`: a new record).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Asked(pub Option<Option<MemoryId>>);

impl DevtoolsInspectable for Asked {}

/// What a screen read, shared by its renders without copying it: one reading equals only itself.
#[derive(Debug)]
pub struct Shared<T>(pub Arc<T>);

impl<T> Shared<T> {
  pub fn new(read: T) -> Self {
    Self(Arc::new(read))
  }
}

impl<T> Clone for Shared<T> {
  fn clone(&self) -> Self {
    Self(Arc::clone(&self.0))
  }
}

impl<T> PartialEq for Shared<T> {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl<T> DevtoolsInspectable for Shared<T> {}

/// The task whose Memory tab this is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfTask {
  pub id: orchester_core::id::TaskId,
  pub key: TaskKey,
}

#[derive(Clone)]
pub struct MemoryState {
  pub repo: PathBuf,
  /// The task of the task's tab; `None` on the project's page.
  pub task: Option<OfTask>,
  pub project_id: ProjectId,
  /// The project's name.
  pub project: Arc<str>,
  pub opened: Signal<Opened>,
  pub filters: Signal<Filters>,
  /// What the search field holds; the filters search it.
  pub search: Signal<String>,
  pub histories: Signal<Unfolded>,
  /// The task's handoffs before the latest are listed too.
  pub earlier_handoffs: Signal<bool>,
  pub selected: Signal<Chosen>,
  pub action: work::Action,
  pub asked: Signal<Asked>,
  pub vault: Vault,
}

/// The parts of a Memory screen's state made with its component.
pub struct Made {
  pub opened: Signal<Opened>,
  pub filters: Signal<Filters>,
  pub search: Signal<String>,
  pub histories: Signal<Unfolded>,
  pub earlier_handoffs: Signal<bool>,
  pub selected: Signal<Chosen>,
  pub action: work::Action,
  pub asked: Signal<Asked>,
}

impl Made {
  /// A screen's state at first, filtered by `filters`. A finished change closes what was open, and the project's
  /// screens read again (`feed`).
  pub fn new(ctx: &mut Ctx, filters: Filters, feed: &ProjectFeed) -> Self {
    let made = Self {
      opened: ctx.signal(Opened::Nothing),
      filters: ctx.signal(filters),
      search: ctx.signal(String::new()),
      histories: ctx.signal(Unfolded::default()),
      earlier_handoffs: ctx.signal(false),
      selected: ctx.signal(Chosen::default()),
      action: work::action(ctx),
      asked: ctx.signal(Asked::default()),
    };
    let (opened, feed) = (made.opened.clone(), feed.clone());
    ctx.watch(&made.action.state(), move |done| {
      if done.fulfilled_data().is_some() {
        opened.set(Opened::Nothing);
        feed.saved();
      }
    });
    let filters = made.filters.clone();
    ctx.watch(&made.search, move |search: &String| {
      if filters.get_untracked().search != *search {
        filters.update(|filters| filters.search.clone_from(search));
      }
    });
    made
  }

  /// The state the screen provides, for `project` and, on a task's tab, its task.
  pub fn state(&self, ctx: &mut Ctx, project: &crate::project::ShownProject, task: Option<OfTask>) -> MemoryState {
    MemoryState {
      repo: project.open.repo_root.clone(),
      task,
      project_id: project.open.project.id(),
      project: project.open.project.name().as_str().into(),
      opened: self.opened.clone(),
      filters: self.filters.clone(),
      search: self.search.clone(),
      histories: self.histories.clone(),
      earlier_handoffs: self.earlier_handoffs.clone(),
      selected: self.selected.clone(),
      action: self.action.clone(),
      asked: self.asked.clone(),
      vault: crate::services::Services::of(ctx).vault,
    }
  }
}

impl MemoryState {
  pub fn of(ctx: &mut Ctx) -> Self {
    ctx
      .use_context::<Self>()
      .expect("a Memory screen provides its state before mounting its parts")
  }

  /// Starts `write`, unless a change is under way.
  pub fn write(&self, write: Write) {
    if self.action.state().get_untracked().is_pending() {
      return;
    }
    self.asked.set(Asked(Some(write.record())));
    self.action.run(Start {
      repo: self.repo.clone(),
      vault: self.vault.clone(),
      write,
    });
  }

  /// Opens `opened` in place, closing whatever was open, and forgets a change that failed.
  pub fn open(&self, opened: Opened) {
    self.asked.set(Asked(None));
    self.opened.set(opened);
  }

  pub fn close(&self) {
    self.open(Opened::Nothing);
  }

  /// Unfolds or folds `record`'s earlier versions.
  pub fn toggle_history(&self, record: MemoryId) {
    self.histories.update(|open| {
      if !open.0.remove(&record) {
        open.0.insert(record);
      }
    });
  }

  /// Whether `record`'s earlier versions are unfolded.
  pub fn history_open(&self, record: MemoryId) -> bool {
    self.histories.with(|open| open.0.contains(&record))
  }

  /// The record the project's inspector shows, if a press chose one.
  pub fn chosen(&self) -> Option<MemoryId> {
    self.selected.get().0
  }

  /// Shows `record` in the project's inspector.
  pub fn choose(&self, record: MemoryId) {
    self.selected.set(Chosen(Some(record)));
  }

  /// The record the last change asked for was of, once one was asked for.
  pub fn asked(&self) -> Option<Option<MemoryId>> {
    self.asked.get().0
  }

  /// Why the last change of `record` failed (`None`: a new record); `None` while it did not.
  pub fn failure_for(&self, record: Option<MemoryId>) -> Option<orchester_workspace::messages::Message> {
    let state = self.action.state().get();
    let rejected = state.is_rejected();
    let failed = state.error.filter(|_| rejected)?;
    (self.asked() == Some(record)).then_some(failed)
  }

  /// Whether a change of `record` is under way.
  pub fn pending_for(&self, record: Option<MemoryId>) -> bool {
    self.action.state().get().is_pending() && self.asked() == Some(record)
  }

  /// Whether any change is under way.
  pub fn busy(&self) -> bool {
    self.action.state().get().is_pending()
  }
}
