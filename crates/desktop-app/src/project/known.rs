use std::{collections::HashMap, sync::Arc};

use lurq::{app::ctx::Ctx, core::Signal};
use orchester_core::{id::ProjectId, name::Name};
use orchester_workspace::{tasks::integration_jobs::ProjectJobs, workspace::OpenProject};
use parking_lot::Mutex;

use super::{ProjectFeed, ProjectState};

/// What the window knows of the projects its tabs show, and of those it still follows after their last tab
/// closed: each one's state once it has been resolved (`None` while it is being opened), and the feed of
/// each one that opened. Clones share them; reading a state in a render re-renders it when the states change.
#[derive(Clone)]
pub struct Projects {
  states: Signal<KnownStates>,
  /// How often opening each project was asked for again ([`Projects::retry`]).
  attempts: Signal<Attempts>,
  /// How often every project was to be opened again ([`Projects::forget_all`]).
  rounds: Signal<u32>,
  feeds: Arc<Mutex<HashMap<ProjectId, ProjectFeed>>>,
  /// The session's merge and removal operations, which each project's feed follows.
  jobs: ProjectJobs,
  /// The projects whose feeds are kept current ([`Projects::followed`]).
  followed: Signal<Followed>,
}

#[derive(Clone, Default, PartialEq)]
struct Attempts(Arc<HashMap<ProjectId, u32>>);

impl lurq::app::component::DevtoolsInspectable for Attempts {}

/// The resolved projects by id.
#[derive(Clone, Default, PartialEq)]
struct KnownStates(Arc<HashMap<ProjectId, ProjectState>>);

impl lurq::app::component::DevtoolsInspectable for KnownStates {}

/// The projects followed, in the order the host mounts their loaders.
#[derive(Clone, Default, PartialEq)]
struct Followed(Arc<Vec<ProjectId>>);

impl lurq::app::component::DevtoolsInspectable for Followed {}

impl PartialEq for Projects {
  fn eq(&self, other: &Self) -> bool {
    self.states.id() == other.states.id()
  }
}

impl lurq::app::component::DevtoolsInspectable for Projects {}

impl Projects {
  /// No project known yet but `landed`, the project the session opened on, as opening it found it.
  /// Each project's feed follows its operations in `jobs`, the registry the task pages and Orchester's
  /// MCP tools share (`Services::jobs`).
  pub fn new(landed: Option<(ProjectId, ProjectState)>, jobs: ProjectJobs) -> Self {
    Self {
      states: Signal::new(KnownStates(Arc::new(landed.into_iter().collect()))),
      attempts: Signal::new(Attempts::default()),
      rounds: Signal::new(0),
      feeds: Arc::default(),
      jobs,
      followed: Signal::new(Followed::default()),
    }
  }

  /// The projects whose feeds the window keeps current: those its tabs show, and those an open project's runner
  /// still works for after its last tab closed (`Runners::keep_open`). Reading it in a render re-renders it when they
  /// change.
  pub fn followed(&self) -> Arc<Vec<ProjectId>> {
    self.followed.get().0
  }

  /// Records the projects followed; their readers render only when they changed.
  pub(super) fn set_followed(&self, followed: Vec<ProjectId>) {
    if *self.followed.get_untracked().0 != followed {
      self.followed.set(Followed(Arc::new(followed)));
    }
  }

  /// Every project known to be open, whether a tab shows it or not.
  pub(super) fn open_all(&self) -> Vec<OpenProject> {
    self
      .states
      .with(|known| known.0.values().filter_map(|state| state.open().cloned()).collect())
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
      .or_insert_with(|| ProjectFeed::new(self.jobs.of(project)))
      .clone()
  }

  /// The feed of `project` if it was made, without making one, for the app's MCP tools.
  pub fn feed_now(&self, project: ProjectId) -> Option<ProjectFeed> {
    self.feeds.lock().get(&project).cloned()
  }

  /// How often opening `project` was asked for again, alone or with every project; reading it in a render
  /// re-renders it then.
  pub fn attempt(&self, project: ProjectId) -> u32 {
    let alone = self
      .attempts
      .with(|attempts| attempts.0.get(&project).copied().unwrap_or(0));
    alone.wrapping_add(self.rounds.get())
  }

  /// Opens `project` again, e.g. once its folder is back: what it stood for is forgotten, so its
  /// screen waits for the new answer.
  pub fn retry(&self, project: ProjectId) {
    self.states.update(|known| {
      Arc::make_mut(&mut known.0).remove(&project);
    });
    self.attempts.update(|attempts| {
      *Arc::make_mut(&mut attempts.0).entry(project).or_insert(0) += 1;
    });
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

  /// Forgets every project's state and feed: another account signed in, and its tabs open each of its projects
  /// again, as its own list says (`root`).
  pub fn forget_all(&self) {
    self.feeds.lock().clear();
    self.states.update(|known| Arc::make_mut(&mut known.0).clear());
    self.rounds.update(|rounds| *rounds = rounds.wrapping_add(1));
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
