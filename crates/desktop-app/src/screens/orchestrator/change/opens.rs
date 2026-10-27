//! The places a change's card opens: the draft or what was published, *What automation may do* after the chat
//! published itself, the run's changes while a merge waits, the server in Connections, the task otherwise, and the
//! queue for a refused release. A decision card shows them as ghosts; its event line, the first as its link.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  id::{ProjectId, ResourceId},
  orchestrator::change::{Change, Publishing},
};
use orchester_workspace::orchestrator::facts as read;

use super::{
  facts,
  words::{Phase, View},
};
use crate::{
  routes::{Route, TaskTab},
  ui::icon::Icon,
};

/// A place a card opens: its icon, its words and where it leads.
pub(super) type Opens = (Icon, Arc<str>, Route);

/// The places the card of `view` opens, in project `project`.
pub(super) fn opens(ctx: &Ctx, (project, _name): (ProjectId, &str), view: &View) -> Vec<Opens> {
  let task = view.action.task.as_deref();
  let task_open = |tab| {
    task.and_then(|key| {
      facts::task_route(project, key, tab).map(|route| {
        (
          Icon::ArrowUpRight,
          ctx.t_args("orchestrator.action.openTask", [("task", key)]),
          route,
        )
      })
    })
  };
  match view.action.change.as_ref().map(|change| &change.change) {
    Some(Change::Publish { of, resource, .. }) => publish_opens(ctx, (project, view), (*of, *resource)),
    Some(Change::Merge { .. }) if view.phase().waits() => task
      .and_then(|key| facts::task_route(project, key, TaskTab::Changes))
      .map(|route| {
        (
          Icon::ArrowUpRight,
          ctx.t("orchestrator.change.merge.openChanges"),
          route,
        )
      })
      .into_iter()
      .collect(),
    Some(Change::Cancel { .. }) if view.phase().waits() => Vec::new(),
    // A release names a task only for its group: the task opens on its Details.
    Some(Change::Release { .. }) if matches!(view.phase(), Phase::Refused(_)) => {
      let queue = (
        Icon::ArrowUpRight,
        ctx.t("orchestrator.action.openQueue"),
        super::super::action::queue_route(project),
      );
      task_open(TaskTab::default()).into_iter().chain([queue]).collect()
    }
    Some(Change::Release { .. }) => task_open(TaskTab::default()).into_iter().collect(),
    Some(Change::Mcp(mcp)) => super::mcp::opens(ctx, (view, project), mcp),
    // Every other change is to a run of the task, which its Summary shows.
    _ => task_open(TaskTab::Summary).into_iter().collect(),
  }
}

fn publish_opens(ctx: &Ctx, (project, view): (ProjectId, &View), publication: (Publishing, ResourceId)) -> Vec<Opens> {
  let route = facts::draft_route(project, publication);
  match view.phase() {
    Phase::Applied | Phase::Done => {
      let name = view.facts.map_or_else(
        || {
          view
            .action
            .change
            .as_ref()
            .map(|change| change.subject.clone())
            .unwrap_or_default()
        },
        |facts| match facts {
          read::Facts::Publication(found) => found.name.clone(),
          read::Facts::Run(_) => String::new(),
        },
      );
      let mut opens = vec![(
        Icon::ArrowUpRight,
        ctx.t_args("orchestrator.change.publish.open", [("name", name.as_str())]),
        route,
      )];
      if view.phase() == Phase::Done {
        opens.push((
          Icon::Settings,
          ctx.t("orchestrator.change.publish.automation"),
          Route::Settings,
        ));
      }
      opens
    }
    Phase::Waits | Phase::Applying | Phase::Dismissed | Phase::Refused(_) => {
      vec![(
        Icon::ArrowUpRight,
        ctx.t("orchestrator.change.publish.openDraft"),
        route,
      )]
    }
  }
}
