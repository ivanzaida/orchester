//! A team task's page (`docs/architecture/team-authority-plan.md` §6, P6): the task as its server read it, joined by
//! its id with what this computer knows of it, its runs, their conversation and delivery. A task this computer never
//! ran has none of them; nothing of the task itself is taken from this computer's copy.

use std::{collections::BTreeMap, path::Path};

use orchester_core::{store::TaskStore, task::Task};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};

use super::{
  detail::TaskDetail,
  requirements::{self, LocalRuns},
};

impl TaskDetail {
  /// `task` as the server read it, with this computer's runs of it in the project at `repo_root`, settled as
  /// [`Self::load_settled`] settles them; `missing_pipeline` is the slug of its pipeline this computer has none for.
  /// Blocks on SQLite.
  pub fn of_team(
    repo_root: &Path,
    (task, missing_pipeline): (Task, Option<String>),
    programs: &Programs,
  ) -> Result<Self, StoreError> {
    let known = ProjectDb::open(repo_root)?.task(task.id())?.is_some();
    let mut detail = if known {
      Self::load_settled(repo_root, task.id(), programs)?
    } else {
      let db = ProjectDb::open(repo_root)?;
      let none = LocalRuns {
        latest: None,
        unreadable: false,
      };
      let requirements = requirements::read(&db, &task, none, None)?;
      Self::without_runs(task.clone(), (Vec::new(), BTreeMap::new()), requirements)
    };
    detail.task = task;
    detail.missing_pipeline = missing_pipeline;
    Ok(detail)
  }
}
