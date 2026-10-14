//! Where each item of the panel leads (`Needs you · where each item leads` `icGHG`), and how it is told from the
//! others while the panel shows.

use std::sync::Arc;

use lurq::core::Signal;
use orchester_core::id::ProjectId;
use orchester_workspace::tasks::needs_you::{WaitingItem, WaitingKind};

use super::Shown;
use crate::{
  navigation::{
    Navigation,
    pointed::{Pointed, PointedCard},
  },
  routes::{ProjectPage, Route, TaskView},
};

/// How an item is told from the others while the panel shows, and its element id's end: its card's key when
/// it is answered in place; else its task's, what it is and its step, as several steps of one run may wait at
/// once; `memory-<project>` for `project`'s memory records.
pub(super) fn key_of(project: ProjectId, item: &WaitingItem) -> Arc<str> {
  if let Some(key) = item.kind.card_key() {
    return key;
  }
  let kind = match item.kind {
    WaitingKind::Memory { .. } => return Arc::from(format!("memory-{project}")),
    WaitingKind::Question(_) => "question",
    WaitingKind::Plan(_) => "plan",
    WaitingKind::Decision(_) => "decision",
    WaitingKind::Interrupted { .. } => "interrupted",
    WaitingKind::StoppedAtLimit { .. } => "limit",
    WaitingKind::ReadyToMerge => "merge",
    WaitingKind::Approval(_) | WaitingKind::Choice(_) => "card",
  };
  let task = item.task.as_ref().map_or_else(String::new, |task| task.id.to_string());
  let step = item.step.as_ref().map_or(0, |step| step.number);
  Arc::from(format!("task-{task}-{kind}-{step}"))
}

/// Where an item that is not answered in place leads: an agent's question to the task's Conversation, where
/// the composer answers it; the others to the task's Summary.
pub(super) fn route_of(shown: &Shown) -> Option<Route> {
  let task = shown.item.task.as_ref()?;
  let view = match shown.item.kind {
    WaitingKind::Question(_) => TaskView::Conversation,
    _ => TaskView::of(crate::routes::TaskTab::Summary),
  };
  task_route(shown.project, &task.key, view)
}

pub(super) fn task_route(project: ProjectId, key: &str, view: TaskView) -> Option<Route> {
  let key = key.parse().ok()?;
  Some(Route::Project(project, ProjectPage::Task(key, view)))
}

/// The decision card of the task's Summary an item points at, which the page brings into view and rings
/// (`icGHG`): the plan under review, raising the limit, the decision after retries.
pub(super) fn pointed_of(shown: &Shown) -> Option<Pointed> {
  let card = match shown.item.kind {
    WaitingKind::Plan(_) => PointedCard::Plan,
    WaitingKind::StoppedAtLimit { .. } => PointedCard::RaiseLimit,
    WaitingKind::Decision(_) => PointedCard::Retries,
    _ => return None,
  };
  let task = shown.item.task.as_ref()?.key.parse().ok()?;
  Some(Pointed {
    project: shown.project,
    task,
    card,
  })
}

/// Goes to `route` and closes the panel, which the page opening takes the place of.
pub(super) fn go(navigation: &Navigation, open: &Signal<bool>, route: Option<Route>) -> Arc<dyn Fn() + Send + Sync> {
  let (navigation, open) = (navigation.clone(), open.clone());
  Arc::new(move || {
    if let Some(route) = route.clone() {
      navigation.go(route);
    }
    open.set(false);
  })
}

/// Goes to `route`, pointing at `card` on it, and closes the panel.
pub(super) fn open_at(
  navigation: &Navigation,
  open: &Signal<bool>,
  route: Route,
  card: Option<Pointed>,
) -> Arc<dyn Fn() + Send + Sync> {
  let Some(card) = card else {
    return go(navigation, open, Some(route));
  };
  let (navigation, open) = (navigation.clone(), open.clone());
  Arc::new(move || {
    navigation.go_to_card(&route, card.clone());
    open.set(false);
  })
}
