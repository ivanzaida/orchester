use std::{collections::HashMap, sync::Arc};

use lurq::{app::ctx::Ctx, core::Signal};
use orchester_core::{id::ProjectId, name::Name};
use parking_lot::Mutex;

use super::{ProjectFeed, ProjectState};

/// What the window knows of the projects its tabs show: each one's state once it has been resolved
/// (`None` while it is being opened), and the feed of each one that opened. Clones share them; reading a
/// state in a render re-renders it when the states change.
#[derive(Clone)]
pub struct Projects {
  states: Signal<KnownStates>,
  feeds: Arc<Mutex<HashMap<ProjectId, ProjectFeed>>>,
}

/// The resolved projects by id.
#[derive(Clone, Default, PartialEq)]
struct KnownStates(Arc<HashMap<ProjectId, ProjectState>>);

impl lurq::app::component::DevtoolsInspectable for KnownStates {}

impl PartialEq for Projects {
  fn eq(&self, other: &Self) -> bool {
    self.states.id() == other.states.id()
  }
}

impl lurq::app::component::DevtoolsInspectable for Projects {}

impl Projects {
  /// No project known yet but `landed`, the project the session opened on, as opening it found it.
  pub fn new(landed: Option<(ProjectId, ProjectState)>) -> Self {
    Self {
      states: Signal::new(KnownStates(Arc::new(landed.into_iter().collect()))),
      feeds: Arc::default(),
    }
  }

  /// The projects the window provides to everything it mounts.
  pub fn of(ctx: &mut Ctx) -> Self {
    ctx
      .use_context::<Self>()
      .expect("the window provides the projects before mounting the chrome and the screens")
  }

  /// What `project` stands for; `None` while it is being opened.
  pub fn state(&self, project: ProjectId) -> Option<ProjectState> {
    self.states.with(|known| known.0.get(&project).cloned())
  }

  /// [`state`](Self::state) without subscribing a render to it, for the app's MCP tools.
  pub fn state_now(&self, project: ProjectId) -> Option<ProjectState> {
    self.states.with_untracked(|known| known.0.get(&project).cloned())
  }

  /// The name of `project`, once it is known to be in the list.
  pub fn name(&self, project: ProjectId) -> Option<Name> {
    self
      .states
      .with(|known| known.0.get(&project).and_then(ProjectState::name).cloned())
  }

  /// The feed of `project`, made empty the first time it is asked for.
  pub fn feed(&self, project: ProjectId) -> ProjectFeed {
    self
      .feeds
      .lock()
      .entry(project)
      .or_insert_with(ProjectFeed::new)
      .clone()
  }

  /// The feed of `project` if it was made, without making one, for the app's MCP tools.
  pub fn feed_now(&self, project: ProjectId) -> Option<ProjectFeed> {
    self.feeds.lock().get(&project).cloned()
  }

  /// Records what `project` stands for; its readers render only when that changed.
  pub(super) fn set(&self, project: ProjectId, state: ProjectState) {
    if self.state_now(project).as_ref() == Some(&state) {
      return;
    }
    self.states.update(|known| {
      Arc::make_mut(&mut known.0).insert(project, state);
    });
  }

  /// Forgets what projects no tab shows any more (not among `shown`) could not be opened as, so a tab
  /// that shows one again opens it again. Open projects stay known.
  pub(super) fn forget_unavailable(&self, shown: &[ProjectId]) {
    let stale = self.states.with_untracked(|known| {
      known
        .0
        .iter()
        .any(|(id, state)| state.open().is_none() && !shown.contains(id))
    });
    if !stale {
      return;
    }
    self.states.update(|known| {
      Arc::make_mut(&mut known.0).retain(|id, state| state.open().is_some() || shown.contains(id));
    });
  }
}
