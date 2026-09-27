//! What clicking a status bar item or popover row opens: a place in the open project, the
//! runner log, or the operations popover.

use std::path::PathBuf;

use lurq::{app::ctx::Ctx, core::Signal};

use super::ProjectBar;
use crate::{
  activity::{Activity, OperationId, Place, Target},
  navigation::route::{AgentSelection, InstructionsTab, Page, PipelineView, SettingsTab, TaskPageTab, ToolSelection},
  runner::Runners,
  screens::project::ProjectNav,
};

#[derive(Clone)]
pub struct Opener {
  activity: Activity,
  /// Present inside the project shell.
  nav: Option<ProjectNav>,
  runners: Option<Runners>,
  repo_root: Option<PathBuf>,
  popover: Signal<bool>,
}

impl Opener {
  pub fn new(ctx: &mut Ctx, activity: &Activity, project: Option<&ProjectBar>, popover: &Signal<bool>) -> Self {
    Self {
      activity: activity.clone(),
      nav: ctx.use_context::<ProjectNav>(),
      runners: project.and(ctx.use_context::<Runners>()),
      repo_root: project.map(|project| project.repo_root.clone()),
      popover: popover.clone(),
    }
  }

  /// Whether `target` can be opened from here: places of another project cannot.
  pub fn can_open(&self, target: &Target) -> bool {
    match target {
      Target::Nothing => false,
      Target::RunnerLog => self.runners.is_some(),
      Target::Project { repo_root, .. } => self.nav.is_some() && self.repo_root.as_ref() == Some(repo_root),
    }
  }

  /// Opens `target`; a failure (`failure`) counts as opened and leaves the slot.
  pub fn open(&self, target: &Target, failure: Option<OperationId>) {
    if !self.can_open(target) {
      return;
    }
    if let Some(failure) = failure {
      self.activity.resolve(failure);
    }
    self.popover.set(false);
    match target {
      Target::Nothing => {}
      Target::RunnerLog => self.open_log(),
      Target::Project { place, .. } => self.show(*place),
    }
  }

  fn show(&self, place: Place) {
    let Some(nav) = &self.nav else {
      return;
    };
    nav.show(match place {
      Place::Overview => Page::OVERVIEW,
      Place::Tasks => Page::TASKS,
      Place::PullRequests => Page::PullRequests,
      Place::Pipeline(id) => Page::Pipeline(id, PipelineView::Draft),
      // The instructions screen moves to the tab of the instruction's scope.
      Place::Instruction(id) => Page::Instructions(InstructionsTab::ProjectRules, Some(id)),
      Place::AgentProfile(id) => Page::Agents(AgentSelection::Profile(id)),
      Place::Task(id) => Page::task(id),
      Place::TaskConversation(id) => Page::Task(id, TaskPageTab::Conversation),
      Place::Tool(id) => Page::Tools(ToolSelection::Tool(id)),
      Place::Secrets => Page::SECRETS,
      Place::Connections => Page::CONNECTIONS,
      Place::Settings => Page::Settings(SettingsTab::General),
    });
  }

  /// Opens `page` of the project: the task list for Agents and Queued, Overview › Needs you for
  /// Needs you.
  pub fn page(&self, page: Page) -> Option<impl Fn() + Send + Sync + 'static> {
    let nav = self.nav.clone()?;
    Some(move || nav.show(page))
  }

  pub fn open_log(&self) {
    if let Some(runners) = &self.runners {
      runners.open_log();
    }
  }

  pub fn restart(&self) {
    if let Some(runners) = &self.runners {
      runners.restart();
    }
  }

  pub fn dismiss(&self, failure: OperationId) {
    self.activity.resolve(failure);
  }

  pub fn toggle_popover(&self) {
    self.popover.update(|open| *open = !*open);
  }

  pub fn popover_open(&self) -> bool {
    self.popover.get()
  }

  pub fn popover(&self) -> &Signal<bool> {
    &self.popover
  }
}
