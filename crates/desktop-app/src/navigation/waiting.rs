//! Where a thing waiting for the person leads, the same from the Overview's *Needs you* cards and from the
//! Needs you panel (`Needs you · where each item leads` `icGHG`): where `attention` says it is decided, as the
//! route of its task's page and the decision card of the Summary the page brings into view and rings, or as the
//! project's Memory page for its memory records.

use orchester_core::{id::ProjectId, project::TaskKey};
use orchester_workspace::{
  messages::Message,
  tasks::attention::{Decided, NeedItem, SummaryCard},
};

use super::{
  Navigation,
  pointed::{Pointed, PointedCard},
  press::OpenIn,
};
use crate::routes::{MemoryTab, ProjectPage, Route, TaskTab, TaskView};

/// A page to open for an item, and the card on it to point at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Destination {
  pub route: Route,
  pub pointed: Option<Pointed>,
}

impl Destination {
  /// Where `item` of `project` leads: a request or a question to its task's Conversation, a plan, a limit and a
  /// decision after retries to their card of the Summary, the rest to the Summary; the project's memory records to
  /// its Memory page's *Waiting for approval* tab (`CIThk`); why not, where it leads nowhere
  /// ([`Decided::Unavailable`]).
  pub fn of(project: ProjectId, item: &NeedItem) -> Result<Self, Message> {
    let (view, card) = match item.decided() {
      Decided::Unavailable(reason) => return Err(reason),
      Decided::Memory => {
        return Ok(Self {
          route: Route::Project(project, ProjectPage::Memory(Some(MemoryTab::Waiting))),
          pointed: None,
        });
      }
      Decided::MergeQueue => {
        return Ok(Self {
          route: Route::Project(project, ProjectPage::MergeQueue),
          pointed: None,
        });
      }
      Decided::Card(_) | Decided::Conversation => (TaskView::Conversation, None),
      Decided::Summary(card) => (TaskView::of(TaskTab::Summary), card),
    };
    let task: TaskKey = item
      .task()
      .expect("an item decided somewhere has a task")
      .key
      .parse()
      .expect("a task's key reads back as a task key");
    let pointed = card.map(|card| Pointed {
      project,
      task: task.clone(),
      card: match card {
        SummaryCard::Plan => PointedCard::Plan,
        SummaryCard::RaiseLimit => PointedCard::RaiseLimit,
        SummaryCard::Retries => PointedCard::Retries,
      },
    });
    Ok(Self {
      route: Route::Project(project, ProjectPage::Task(task, view)),
      pointed,
    })
  }

  /// Opens it where a press asks (`navigation::link`), pointing at its card when it opens in sight.
  pub fn open(&self, navigation: &Navigation, at: OpenIn) {
    match &self.pointed {
      Some(card) => navigation.open_to_card(&self.route, at, card.clone()),
      None => navigation.open(self.route.clone(), at),
    }
  }
}
