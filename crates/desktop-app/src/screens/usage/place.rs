//! Where the Usage page is: a project's page (`/project/<id>/usage`, `CFRdj`) or every project's, the
//! *Usage* tab of All projects (`/usage`, `rHA8G`), which reads every listed project without starting a
//! runner. A query a person chooses (period, grouping, filter, breakdown) is a new entry of the tab's
//! history.

use std::{path::PathBuf, sync::Arc};

use lurq::app::ctx::Ctx;
use orchester_core::id::ProjectId;
use orchester_workspace::usage::UsageQuery;

use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route},
};

/// The page's place and what it reads again on.
#[derive(Clone, PartialEq)]
pub enum Place {
  /// A project's (boxed: a shown project is far larger than a count).
  Project(Box<ShownProject>),
  /// Every project's, read again whenever a listed project's runner reports (how often they reported).
  Everywhere(u64),
}

impl Place {
  /// The project's name; `None` for every project's page.
  pub fn name(&self) -> Option<Arc<str>> {
    match self {
      Self::Project(project) => Some(project.open.project.name().as_str().into()),
      Self::Everywhere(_) => None,
    }
  }

  /// The project's folder; `None` for every project's page.
  pub fn root(&self) -> Option<PathBuf> {
    match self {
      Self::Project(project) => Some(project.open.repo_root.clone()),
      Self::Everywhere(_) => None,
    }
  }

  /// What the usage is read again on besides a saved threshold and Retry: the project's runs, or every
  /// runner's work.
  pub fn changes(&self) -> (u64, u64) {
    match self {
      Self::Project(project) => (project.feed.changes.get(), project.feed.revision.get()),
      Self::Everywhere(work) => (*work, 0),
    }
  }

  pub fn requery(&self, ctx: &mut Ctx) -> Requery {
    Requery {
      navigation: Navigation::of(ctx),
      project: match self {
        Self::Project(project) => Some(project.open.project.id()),
        Self::Everywhere(_) => None,
      },
    }
  }
}

/// Goes to the page showing another query, as a new entry of the tab's history.
#[derive(Clone)]
pub struct Requery {
  navigation: Navigation,
  project: Option<ProjectId>,
}

impl Requery {
  /// The project whose page it is; `None` on every project's page.
  pub const fn project(&self) -> Option<ProjectId> {
    self.project
  }

  pub fn show(&self, query: UsageQuery) {
    let query = Box::new(query);
    self.navigation.go(match self.project {
      Some(project) => Route::Project(project, ProjectPage::Usage(query)),
      None => Route::Usage(query),
    });
  }
}
