//! Ending the claims of runs that ended (`docs/architecture/server.md`, "Claims", ORC-284). Each claim the person
//! holds in a shared project is held against the phase of its task's latest run on this computer, read as the task's
//! page reads it (a run whose work a person merged without Orchester is merged), by core's [`claim_end`]: a merged
//! run completes its claim through the run progress report (`PUT …/run`), a cancelled one releases it, and every other
//! phase keeps it. This runs on every run change and every sync, so claims that ended before it existed end on the
//! first sync too. The heartbeat names only the claims that are kept: one whose end the server did not take goes stale
//! instead of staying fresh, and the next sync tries again.

use std::path::Path;

use orchester_api::{ErrorCode, RunReport, client::ClientError};
use orchester_core::{
  id::{TaskId, UserId},
  integration::{RunMerge, RunWork},
  run::{Run, RunEvent, TaskPhase},
  sharing::{ClaimEnd, claim_end},
  store::{MergeStore, RunStore, TaskStore},
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};

use super::ProjectServer;
use crate::{
  background,
  sharing::{Servers, SharingError},
  tasks::{delivery, detail::run_store_error},
};

/// Why a claim whose run ended was not ended on the server; the next sync tries again.
#[derive(Debug, thiserror::Error)]
pub enum ClaimEndError {
  /// The server has no run progress report: `PUT …/run` answered `404` or `405`, as a server older than the report
  /// does.
  #[error("the server does not take run progress reports (not_supported)")]
  NotSupported,
  #[error(transparent)]
  Sharing(#[from] SharingError),
}

/// A claim the person held whose task's latest run here ended it, and whether the server took its end.
#[derive(Debug)]
pub struct EndedClaim {
  pub task: TaskId,
  /// [`ClaimEnd::Complete`] for a merged run, [`ClaimEnd::Release`] for a cancelled one.
  pub end: ClaimEnd,
  pub result: Result<(), ClaimEndError>,
}

/// The claims a person holds in a project, by what core's [`claim_end`] makes of their task's latest run here.
pub(super) struct HeldClaims {
  /// The heartbeat keeps these fresh: their run goes on or waits for review, or there is no run of theirs here.
  pub(super) kept: Vec<TaskId>,
  ending: Vec<Ending>,
}

enum Ending {
  /// A merged run's, with its report.
  Complete(TaskId, RunReport),
  /// A cancelled run's.
  Release(TaskId),
}

impl HeldClaims {
  /// The claims `me` holds in the project at `root`, as its cache has them. Git, through `programs`, tells whether
  /// the work of a run that passed every step was merged without Orchester. Blocking.
  pub(super) fn read(root: &Path, me: UserId, programs: &Programs) -> Result<Self, StoreError> {
    let db = ProjectDb::open(root)?;
    let mut held = Self {
      kept: Vec::new(),
      ending: Vec::new(),
    };
    let mut latest = Vec::new();
    for task in db.claims_held_by(me)? {
      match Latest::read(&db, task)? {
        Some(run) => latest.push(run),
        None => held.kept.push(task),
      }
    }
    settle(programs, root, &mut latest);
    for run in latest {
      match claim_end(run.phase) {
        ClaimEnd::Keep => held.kept.push(run.task),
        ClaimEnd::Release => held.ending.push(Ending::Release(run.task)),
        ClaimEnd::Complete => held.ending.push(Ending::Complete(run.task, run.report(&db)?)),
      }
    }
    Ok(held)
  }
}

/// The latest run here of a task whose claim the person holds, with its merge and the phase it gives the task.
struct Latest {
  task: TaskId,
  run: Run,
  merge: Option<RunMerge>,
  phase: TaskPhase,
}

impl Latest {
  /// `None` without a readable run of `task` here: a claim taken by hand or on another computer, or a latest run that
  /// cannot be read.
  fn read(db: &ProjectDb, task: TaskId) -> Result<Option<Self>, StoreError> {
    let Some(stored) = db.task(task)? else {
      return Ok(None);
    };
    let Some(Ok(run)) = db.runs_of_task(task).map_err(run_store_error)?.pop() else {
      return Ok(None);
    };
    let merge = db.run_merge(run.id())?;
    let phase = TaskPhase::of(&stored, Some(&run), merge.as_ref());
    Ok(Some(Self {
      task,
      run,
      merge,
      phase,
    }))
  }

  /// The report of a merged run: merged since Orchester merged it, or, for work a person merged without Orchester,
  /// since the run last recorded something, as git does not say when.
  fn report(&self, db: &ProjectDb) -> Result<RunReport, StoreError> {
    let since = match &self.merge {
      Some(merge) => merge.merged_at,
      None => db
        .events(self.run.id())
        .map_err(run_store_error)?
        .last()
        .map_or_else(|| self.run.created_at(), RunEvent::at),
    };
    Ok(RunReport {
      number: self.run.number().get(),
      phase: self.phase,
      step: None,
      started_at: self.run.created_at().unix_millis(),
      phase_since: since.unix_millis(),
    })
  }
}

/// Each run of `latest` that passed every step reads as merged when git finds its work merged without Orchester, as
/// on the task's page; one reading of the branches for all of them.
fn settle(programs: &Programs, root: &Path, latest: &mut [Latest]) {
  let mut succeeded: Vec<&mut Latest> = latest
    .iter_mut()
    .filter(|latest| latest.phase == TaskPhase::Succeeded)
    .collect();
  let works: Vec<RunWork> = succeeded.iter().map(|latest| RunWork::of(&latest.run)).collect();
  for (latest, delivery) in succeeded.iter_mut().zip(delivery::deliveries(programs, root, &works)) {
    latest.phase = delivery.phase(latest.phase);
  }
}

impl Servers {
  /// Ends the claims of runs that ended in every shared project of the workspace, e.g. right after a run changed.
  pub async fn end_claims(&self, programs: &Programs) {
    let servers = self.clone();
    let Ok(projects) = background::run(move || super::work::shared_projects(&servers)).await else {
      // The blocking pool is shutting down with the window.
      return;
    };
    for project in projects {
      end_claims_of(&project, programs).await;
    }
  }
}

/// [`ProjectServer::end_claims`], quietly while the server cannot take a change; every failure is logged.
pub(super) async fn end_claims_of(project: &ProjectServer, programs: &Programs) {
  match project.end_claims(programs).await {
    Ok(_) | Err(SharingError::Offline | SharingError::SignedOut | SharingError::NotMember) => {}
    Err(error) => tracing::warn!(%error, project = %project.link().project, "cannot end the claims of runs that ended"),
  }
}

impl ProjectServer {
  /// Ends each claim the person holds here whose task's latest run here ended it, and says what became of each. One
  /// the server did not take is logged and stays held here, so the next sync tries it again. Fails at once while
  /// the server cannot take a change.
  pub async fn end_claims(&self, programs: &Programs) -> Result<Vec<EndedClaim>, SharingError> {
    self.writable()?;
    let me = self.me().ok_or(SharingError::SignedOut)?;
    let (root, programs) = (self.repo_root.clone(), programs.clone());
    let held = background::run(move || HeldClaims::read(&root, me, &programs)).await??;
    let mut ended = Vec::with_capacity(held.ending.len());
    for ending in held.ending {
      let (task, end) = match &ending {
        Ending::Complete(task, _) => (*task, ClaimEnd::Complete),
        Ending::Release(task) => (*task, ClaimEnd::Release),
      };
      let result = self.end_claim(ending).await;
      if let Err(error) = &result {
        tracing::warn!(%error, %task, project = %self.link.project, ?end,
          "the claim of a run that ended was not ended; the next sync tries again");
      }
      ended.push(EndedClaim { task, end, result });
    }
    Ok(ended)
  }

  async fn end_claim(&self, ending: Ending) -> Result<(), ClaimEndError> {
    let (task, report) = match ending {
      Ending::Release(task) => return Ok(self.release_claim(task).await?),
      Ending::Complete(task, report) => (task, report),
    };
    let answer = self
      .call(|client, token, project| async move { client.report_run(&token, project, task, &report).await })
      .await;
    let completed = match answer {
      Err(SharingError::Client(error)) if not_supported(&error) => return Err(ClaimEndError::NotSupported),
      answer => answer?,
    };
    Ok(self.cache_answer(completed).await?)
  }
}

/// Whether `error` says the server has no `PUT …/run`: a server answers a route it does not know with `404
/// not_found`, or `405` where only other methods are routed; something in front of it may answer without the
/// contract's error.
fn not_supported(error: &ClientError) -> bool {
  matches!(error.code(), Some(ErrorCode::NotFound | ErrorCode::MethodNotAllowed))
    || matches!(error, ClientError::NotOrchester { status: 404 | 405 })
}
