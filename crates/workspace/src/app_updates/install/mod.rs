//! Installing a ready update on Windows (*Restart to update*): with no run at work it installs at once; with runs at
//! work the person waits for them (the default) or stops them, each paused at its next checkpoint so it goes on after
//! the restart (design 03 `MGILm`). Then the runners stop, the databases are copied aside ([`backup`]), the installer
//! is started silently to wait for the app's exit and open Orchester again ([`handover`]), and the window closes. Every
//! failure before the installer took over leaves this version running, its runners started again, and the state
//! [`UpdateState::InstallFailed`] with the reason; the next start tells how the installer did ([`outcome`]).

mod backup;
mod handover;
mod outcome;

use std::{
  io,
  path::PathBuf,
  process::Child,
  time::{Duration, Instant},
};

use orchester_store::SessionEnd;

pub use self::{
  backup::{BACKUPS_FOLDER, Databases, KEEP, back_up, backups},
  handover::{Handover, InstallRecord, RECORD_FILE, RESULT_FILE},
  outcome::{AfterInstall, InstallFailure, JustUpdated, after_install},
};
use super::{AppUpdates, ReadyUpdate, UpdateState};
use crate::{
  background,
  close::{CloseKind, session_end},
  runner::{ActiveWork, QUIET_STOP_TIMEOUT, Runners},
};

/// How long the app watches the started installer before it exits: one that ends this soon did not wait for it.
const EARLY_EXIT: Duration = Duration::from_secs(2);
/// How often the wait for the runs at work looks again, besides when the runners report.
const RUNS_RECHECK: Duration = Duration::from_millis(500);
/// How long *Stop runs and install* waits for the runs at work to reach their next checkpoints: an agent in the
/// middle of a long turn is ended then, and its run goes on from its last checkpoint after the restart.
pub const STOP_RUNS_WAIT: Duration = Duration::from_secs(60);

/// What *Restart to update* does now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RestartPlan {
  /// No update is ready, or one is installing already.
  NothingReady,
  /// No run is at work: it installs at once, without a question.
  InstallNow(ReadyUpdate),
  /// Runs are at work: the person waits for them or stops them.
  Ask { update: ReadyUpdate, runs: Vec<ActiveWork> },
}

/// The person's answer about the runs at work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunsAtWork {
  /// *Install when they end*: it installs once no run is at work.
  WaitForThem,
  /// *Stop runs and install*: each run pauses at its next checkpoint, or is ended after [`STOP_RUNS_WAIT`], and goes
  /// on from its last checkpoint after the restart.
  StopThem,
}

/// Where an install the person started stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallStep {
  /// It waits for these runs at work to end, as the person knows them (`ORC-110`).
  WaitingForRuns(Vec<String>),
  /// The runners stop, the runs at work at their next checkpoints.
  StoppingRuns,
  /// The databases are copied aside.
  BackingUp,
  /// The installer starts; the window closes once it took over.
  HandingOver,
}

/// How the app starts the installer: from its own folder and process, or a stand-in's in a test.
#[derive(Clone, Debug)]
pub struct Installer {
  /// The folder the running copy was installed into.
  pub program_folder: PathBuf,
  /// The app's process.
  pub pid: u32,
  /// Starts the installer the way [`Handover::start`] does.
  pub start: fn(&Handover) -> io::Result<Child>,
}

impl Installer {
  /// This program's installer start.
  pub fn this_program() -> io::Result<Self> {
    let program = std::env::current_exe()?;
    let program_folder = program
      .parent()
      .map(PathBuf::from)
      .ok_or_else(|| io::Error::other("the program has no folder"))?;
    Ok(Self {
      program_folder,
      pid: std::process::id(),
      start: Handover::start,
    })
  }
}

/// The installer took over: the window closes now, and the session ends as `end`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandedOver {
  pub end: SessionEnd,
}

impl AppUpdates {
  /// What *Restart to update* does with the runs at work now.
  pub fn restart_plan(&self, runners: &Runners) -> RestartPlan {
    let Some(update) = self.state().ready().cloned() else {
      return RestartPlan::NothingReady;
    };
    let runs = runners.active_work();
    if runs.is_empty() {
      RestartPlan::InstallNow(update)
    } else {
      RestartPlan::Ask { update, runs }
    }
  }

  /// The update that installed at this start, for the status bar's *Updated to* and What's new.
  pub fn just_updated(&self) -> Option<&JustUpdated> {
    self.inner.just_updated.as_ref()
  }

  /// Installs the ready update: waits for the runs at work or stops them as `runs` says, stops the runners, copies
  /// the databases aside and hands over to the installer `installer` starts. `Some` once the installer took over and
  /// the window must close; `None` when nothing was ready or an install runs already, or when it failed, which leaves
  /// this version running with its runners and the state [`UpdateState::InstallFailed`].
  pub async fn install(&self, runners: &Runners, runs: RunsAtWork, installer: &Installer) -> Option<HandedOver> {
    // No check runs meanwhile: none replaces the installer this hands over to.
    let _no_checks = self.inner.checking.lock().await;
    let update = self.begin_install(runs, runners)?;
    if runs == RunsAtWork::WaitForThem {
      self.wait_for_runs(runners, &update).await;
    }
    self.step(&update, InstallStep::StoppingRuns);
    let (kind, deadline) = match runs {
      RunsAtWork::StopThem => (CloseKind::WaitForAgents, Some(STOP_RUNS_WAIT)),
      RunsAtWork::WaitForThem => (CloseKind::Quiet, Some(QUIET_STOP_TIMEOUT)),
    };
    let stopped = runners.shutdown(deadline).await;
    tracing::info!(version = %update.version, ?stopped, "the runners stopped for the update");
    match self.hand_over(runners, &update, installer).await {
      Ok(()) => Some(HandedOver {
        end: session_end(kind, stopped),
      }),
      Err(failure) => {
        self.install_failed(runners, &update, failure).await;
        None
      }
    }
  }

  /// Moves a ready update on to installing; `None` when none is ready or one installs already.
  fn begin_install(&self, runs: RunsAtWork, runners: &Runners) -> Option<ReadyUpdate> {
    let mut update = None;
    self.inner.state.send_if_modified(|state| {
      let Some(ready) = state.ready().cloned() else {
        return false;
      };
      let step = match runs {
        RunsAtWork::WaitForThem => InstallStep::WaitingForRuns(run_keys(&runners.active_work())),
        RunsAtWork::StopThem => InstallStep::StoppingRuns,
      };
      *state = UpdateState::Installing {
        update: ready.clone(),
        step,
      };
      update = Some(ready);
      true
    });
    update
  }

  /// Waits until no run is at work, telling which runs it waits for.
  async fn wait_for_runs(&self, runners: &Runners, update: &ReadyUpdate) {
    let mut work = runners.work_changes();
    loop {
      let runs = run_keys(&runners.active_work());
      if runs.is_empty() {
        return;
      }
      self.step(update, InstallStep::WaitingForRuns(runs));
      tokio::select! {
        _ = work.changed() => {}
        () = tokio::time::sleep(RUNS_RECHECK) => {}
      }
    }
  }

  /// Copies the databases aside, notes the update for the next start and starts the installer.
  async fn hand_over(
    &self,
    runners: &Runners,
    update: &ReadyUpdate,
    installer: &Installer,
  ) -> Result<(), InstallFailure> {
    self.step(update, InstallStep::BackingUp);
    let data_folder = self.inner.data_folder.clone();
    let running = self.inner.app.version.clone();
    let databases = Databases {
      app: self.inner.workspace.database_path(),
      projects: runners.project_roots(),
    };
    let record = InstallRecord::new(
      &data_folder,
      running.clone(),
      update.version.clone(),
      update.notes.clone(),
    );
    let kept = record.clone();
    let folder = data_folder.clone();
    let prepared = background::run(move || {
      back_up(&folder, &running, &databases).map_err(|error| format!("cannot copy the databases aside: {error}"))?;
      kept
        .keep(&folder)
        .map_err(|error| format!("cannot note the update for the next start: {error}"))
    })
    .await
    .unwrap_or_else(|_| Err("copying the databases aside did not finish".to_owned()));
    prepared.map_err(|reason| failure(reason, None, None))?;
    self.step(update, InstallStep::HandingOver);
    let handover = Handover {
      installer: update.installer.clone(),
      program_folder: installer.program_folder.clone(),
      pid: installer.pid,
      log: record.log.clone(),
      result: data_folder.join(RESULT_FILE),
    };
    let child = (installer.start)(&handover)
      .map_err(|error| failure(format!("cannot start the installer: {error}"), None, Some(&record.log)))?;
    let early = background::run(move || exited_early(child)).await.unwrap_or(Ok(None));
    match early {
      Ok(None) => {
        tracing::info!(version = %update.version, "the installer took over; the app closes");
        Ok(())
      }
      Ok(Some(code)) => Err(failure(
        format!("the installer ended before the app closed, with exit code {code}"),
        Some(code),
        Some(&record.log),
      )),
      Err(error) => Err(failure(
        format!("cannot watch the installer: {error}"),
        None,
        Some(&record.log),
      )),
    }
  }

  /// Leaves this version running after a failed install: the note goes, the runners start again, and the state says
  /// why.
  async fn install_failed(&self, runners: &Runners, update: &ReadyUpdate, failure: InstallFailure) {
    tracing::error!(version = %update.version, reason = %failure.reason, "the update did not install");
    let folder = self.inner.data_folder.clone();
    let _ = background::run(move || InstallRecord::take(&folder)).await;
    runners.reopen();
    self.set(UpdateState::InstallFailed {
      version: update.version.clone(),
      failure,
    });
  }

  fn step(&self, update: &ReadyUpdate, step: InstallStep) {
    self.set(UpdateState::Installing {
      update: update.clone(),
      step,
    });
  }
}

/// The runs at work as the person knows them, each once.
pub(super) fn run_keys(work: &[ActiveWork]) -> Vec<String> {
  let mut keys: Vec<String> = Vec::new();
  for run in work {
    if !keys.iter().any(|key| key == run.task_key()) {
      keys.push(run.task_key().to_owned());
    }
  }
  keys
}

/// The installer's exit code when it ended within [`EARLY_EXIT`]; `None` while it waits for the app. Blocking.
fn exited_early(mut child: Child) -> io::Result<Option<i32>> {
  let since = Instant::now();
  while since.elapsed() < EARLY_EXIT {
    if let Some(status) = child.try_wait()? {
      return Ok(Some(status.code().unwrap_or(-1)));
    }
    std::thread::sleep(Duration::from_millis(50));
  }
  Ok(None)
}

fn failure(reason: String, exit_code: Option<i32>, log: Option<&PathBuf>) -> InstallFailure {
  InstallFailure {
    reason,
    exit_code,
    log: log.filter(|log| log.is_file()).cloned(),
  }
}
