//! What a Memory screen keeps while it shows: its filters and search, which record is open in an editor or a
//! delete confirmation, which histories are unfolded, the record whose details are open in place, and the change
//! under way. The task's Memory tab and the project's Memory page each provide one to the parts they mount; the
//! page keeps it across its tabs, so a search stays when another tab opens.

use std::{collections::BTreeSet, path::PathBuf, sync::Arc};

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx, events::is_synthetic_input},
  components::Row,
  core::Signal,
};
use orchester_core::{
  id::{MemoryId, ProjectId},
  project::TaskKey,
};
use orchester_workspace::{memory::listing::Filters, secrets::Vault};

use super::work::{self, Start, Write};
use crate::{
  project::ProjectFeed,
  ui::{field::field_error, message::message_text, tinted_note::person_only},
};

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

/// The record whose details are open in place on the project's page, if a press opened them.
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
  /// Synthetic input asked for the last change, which was not taken.
  pub refused: Signal<bool>,
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
  pub refused: Signal<bool>,
}

impl Made {
  /// A screen's state at first, filtered by `filters`. A finished change closes what was open, and the project's
  /// screens read again (`feed`); so does a change refused because the records shown were out of date, which stays
  /// open with why.
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
      refused: ctx.signal(false),
    };
    let (opened, selected, feed) = (made.opened.clone(), made.selected.clone(), feed.clone());
    ctx.watch(&made.action.state(), move |done| {
      if done.fulfilled_data().is_some() {
        // A record's page closes too: what it showed changed.
        opened.set(Opened::Nothing);
        selected.set(Chosen::default());
        feed.saved();
      } else if done.is_rejected() && done.error.as_ref().is_some_and(|failed| failed.stale) {
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
      refused: self.refused.clone(),
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

  /// Starts `write`, unless a change is under way. Called in a press's handler: only a person changes memory, as
  /// what records say reaches every later agent's prompt and a change is kept as the person's. lurq's synthetic input
  /// (an MCP client driving the window) is refused, and the screen says so where the change was asked for.
  pub fn write(&self, write: Write) {
    if self.action.state().get_untracked().is_pending() {
      return;
    }
    self.asked.set(Asked(Some(write.record())));
    let synthetic = is_synthetic_input();
    self.refused.set(synthetic);
    if synthetic {
      tracing::warn!("synthetic input asked for a change of memory records; not taken");
      return;
    }
    self.action.run(Start {
      repo: self.repo.clone(),
      vault: self.vault.clone(),
      write,
    });
  }

  /// Opens `opened` in place, closing whatever was open, and forgets a change that failed or was refused.
  pub fn open(&self, opened: Opened) {
    self.asked.set(Asked(None));
    self.refused.set(false);
    self.opened.set(opened);
  }

  pub fn close(&self) {
    self.open(Opened::Nothing);
  }

  /// Leaves a record's page for its tab's list (the crumb, *Cancel*).
  pub fn leave_page(&self) {
    self.close();
    self.selected.set(Chosen::default());
  }

  /// The record whose page the project's page shows: one opened for review, editing or deletion, else the one a
  /// press on its row chose.
  pub fn page_record(&self) -> Option<MemoryId> {
    match self.opened.get() {
      Opened::Review(id) | Opened::Edit(id) | Opened::Delete(id) => Some(id),
      Opened::Nothing | Opened::New => self.chosen(),
    }
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

  /// The record whose details are open in place, if a press opened them.
  pub fn chosen(&self) -> Option<MemoryId> {
    self.selected.get().0
  }

  /// Opens `record`'s details in place, closing another record's; closes them when they are open.
  pub fn choose(&self, record: MemoryId) {
    self.selected.update(|chosen| {
      chosen.0 = (chosen.0 != Some(record)).then_some(record);
    });
  }

  /// The record the last change asked for was of, once one was asked for.
  pub fn asked(&self) -> Option<Option<MemoryId>> {
    self.asked.get().0
  }

  /// Why the last change of `record` did nothing (`None`: a new record), under the controls whose ids begin with
  /// `id`: `{id}-person-only` once synthetic input asked for it, `{id}-error` once it failed; `None` while neither.
  pub fn failure_note(&self, ctx: &Ctx, id: &str, record: Option<MemoryId>) -> Option<Row> {
    if self.asked() != Some(record) {
      return None;
    }
    if self.refused.get() {
      return Some(person_only(format!("{id}-person-only"), &ctx.t("memory.personOnly")));
    }
    let state = self.action.state().get();
    let rejected = state.is_rejected();
    let failed = state.error.filter(|_| rejected)?;
    Some(field_error(&format!("{id}-error"), &message_text(ctx, &failed.message)))
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
