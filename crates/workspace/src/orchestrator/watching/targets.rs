//! A watched target as the person reads it: its name, its kind and where its link goes, and the order that applies
//! to a subscription of it.

use std::sync::Arc;

use orchester_core::{
  id::ModuleId,
  orchestrator::{Subscription, WatchTarget},
};

use super::WatchReads;
use crate::{
  links::{Link, TaskPage},
  messages::Message,
};

/// A target with its words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetView {
  pub target: WatchTarget,
  /// A module's name, a task's key, or what the target takes in.
  pub name: Message,
  /// *module*, *task*.
  pub kind: Message,
  /// The module's page or the task's Summary; the project's Overview for the whole project; none for the tasks
  /// without a module, which have no page.
  pub link: Option<String>,
}

/// The order that applies to a subscription.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WatchOrder {
  /// The person's own words.
  Own(Arc<str>),
  /// None of its own: the project's default order applies.
  ProjectDefault(Arc<str>),
  /// None, and the project has no default order: the person's switches decide.
  None,
}

impl WatchReads {
  /// The name of module `id`; `None` once it is gone from the project.
  pub(super) fn module_name(&self, id: ModuleId) -> Option<String> {
    self
      .tasks
      .modules
      .iter()
      .find(|module| module.id() == id)
      .map(|module| module.name().as_str().to_owned())
  }

  pub(super) fn target_view(&self, target: WatchTarget) -> TargetView {
    let (name, kind, link) = match target {
      WatchTarget::Module(id) => {
        let name = self.module_name(id).map_or_else(
          || Message::new("orchestrator.watch.target.gone"),
          |name| Message::new("orchestrator.watch.target.named").with("name", name),
        );
        let link = Link::Module(self.project, id).path();
        (name, "orchestrator.watch.kind.module", Some(link))
      }
      WatchTarget::Task(id) => match self.task(id) {
        Some(task) => (
          Message::new("orchestrator.watch.target.named").with("name", task.key().to_string()),
          "orchestrator.watch.kind.task",
          Some(Link::Task(self.project, task.key().clone(), TaskPage::Summary).path()),
        ),
        None => (
          Message::new("orchestrator.watch.target.gone"),
          "orchestrator.watch.kind.task",
          None,
        ),
      },
      WatchTarget::WithoutModule => (
        Message::new("orchestrator.watch.target.withoutModule"),
        "orchestrator.watch.kind.withoutModule",
        None,
      ),
      WatchTarget::Project => (
        Message::new("orchestrator.watch.target.project"),
        "orchestrator.watch.kind.project",
        Some(Link::Project(self.project).path()),
      ),
    };
    TargetView {
      target,
      name,
      kind: Message::new(kind),
      link,
    }
  }

  pub(super) fn order(&self, subscription: &Subscription) -> WatchOrder {
    match subscription.order() {
      Some(own) => WatchOrder::Own(own.into()),
      None if !self.default_order.is_empty() => WatchOrder::ProjectDefault(self.default_order.clone()),
      None => WatchOrder::None,
    }
  }
}
