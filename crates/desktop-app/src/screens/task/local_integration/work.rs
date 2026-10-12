use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
};
use orchester_core::{
  blob::ContentHash,
  integration::local::{CheckedResult, LocalConflict, LocalError, LocalPath, Placement, Resolution},
};
use orchester_workspace::{
  background,
  tasks::local_integration::{IntegrationError, LocalIntegration, Preview},
};

use super::TaskContext;
use crate::services::Services;

#[derive(Clone)]
pub enum Action {
  Inspect,
  Refresh,
  ReviewCommit(Vec<LocalPath>),
  Commit {
    paths: Vec<LocalPath>,
    tree: String,
    message: String,
  },
  Preserve(String),
  Sync,
  Merge(Box<CheckedResult>),
  Save {
    file: LocalConflict,
    expected: Option<ContentHash>,
    resolution: Resolution,
    placement: Placement,
    restore: bool,
  },
  Continue,
  Restore,
  FinishRestore,
  Reconcile,
  Abort,
  Cleanup,
  Cancel,
}

#[derive(Clone)]
pub struct Request {
  pub task: TaskContext,
  pub action: Action,
  pub stop: Arc<AtomicBool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
  Changed,
  CommitReview { tree: String, preview: Preview },
}
impl DevtoolsInspectable for Outcome {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem(pub &'static str);
impl DevtoolsInspectable for Problem {}

pub type Work = FutureAction<Request, Outcome, Problem>;

pub fn action(ctx: &mut Ctx) -> Work {
  let services = Services::of(ctx);
  ctx.future_action(move |request: Request| {
    let services = services.clone();
    async move {
      let task = request.task.clone();
      let run = task
        .detail
        .run
        .as_ref()
        .ok_or(Problem("local_integration.error.run"))?
        .id();
      let jobs = services.jobs.of(task.project.open.project.id()).of(run);
      let open = task.project.open.clone();
      let feed = task.project.feed.clone();
      let vault = services.vault.clone();
      let busy =
        orchester_workspace::messages::Message::new("localIntegration.running").with("task", task.key.to_string());
      let result = background::run(move || {
        let _working = jobs
          .local_work(&open, busy)
          .ok_or(Problem("local_integration.error.foreign"))?;
        let mut session = LocalIntegration::open(&open.repo_root, run).map_err(problem)?;
        let outcome = execute(&mut session, request.action, vault.get()).map_err(problem)?;
        if request.stop.load(Ordering::Acquire) {
          session.pause().map_err(problem)?;
        }
        Ok(outcome)
      })
      .await
      .map_err(|_| Problem("local_integration.error.unknown"))?;
      feed.saved();
      result
    }
  })
}

fn execute(
  session: &mut LocalIntegration,
  action: Action,
  vault: &dyn orchester_core::secret::SecretVault,
) -> Result<Outcome, IntegrationError> {
  match action {
    Action::Inspect => {}
    Action::Refresh => session.refresh()?,
    Action::ReviewCommit(paths) => {
      let tree = session.selected_commit_tree(&paths)?;
      let preview = session.selected_commit_preview(&tree, vault)?;
      return Ok(Outcome::CommitReview { tree, preview });
    }
    Action::Commit { paths, tree, message } => session.commit(&paths, &tree, &message)?,
    Action::Preserve(name) => session.preserve(&name)?,
    Action::Sync => {
      session.prepare_if_clean()?;
      session.sync()?;
    }
    Action::Merge(exact) => session.approve_and_merge(&exact)?,
    Action::Save {
      file,
      expected,
      resolution,
      placement,
      restore,
    } => {
      if restore {
        session.save_restore_resolution(&file, expected.as_ref(), &resolution, placement)?;
      } else {
        session.save_task_resolution(&file, expected.as_ref(), &resolution)?;
      }
    }
    Action::Continue => session.continue_task()?,
    Action::Restore => session.restore()?,
    Action::FinishRestore => session.finish_restore()?,
    Action::Reconcile => session.reconcile()?,
    Action::Abort => session.abort_task()?,
    Action::Cleanup => session.remove_preservation()?,
    Action::Cancel => session.cancel_preservation()?,
  }
  Ok(Outcome::Changed)
}

pub fn problem(error: IntegrationError) -> Problem {
  use orchester_exec::local_git::LocalGitError;
  let key = match error {
    IntegrationError::State(state) | IntegrationError::Git(LocalGitError::State(state)) => match state {
      LocalError::Stale => "stale",
      LocalError::ForeignOperation => "foreign",
      LocalError::WrongPhase => "phase",
      LocalError::NotChecked => "unchecked",
      LocalError::ApprovalRequired => "approval",
      LocalError::NotVerified => "unverified",
      LocalError::Reconcile => "unknown",
      LocalError::Unsupported => "unsupported",
      LocalError::Collision => "collision",
      LocalError::Unresolved => "unresolved",
      LocalError::Blank => "blank",
    },
    IntegrationError::Git(LocalGitError::Refused) => "git",
    IntegrationError::Git(_) => "io",
    IntegrationError::Store(_) => "storage",
    IntegrationError::Run => "run",
    IntegrationError::NoCheck => "checkMissing",
    IntegrationError::CheckFailed => "checkFailed",
  };
  Problem(match key {
    "stale" => "local_integration.error.stale",
    "foreign" => "local_integration.error.foreign",
    "phase" => "local_integration.error.phase",
    "unchecked" => "local_integration.error.unchecked",
    "approval" => "local_integration.error.approval",
    "unverified" => "local_integration.error.unverified",
    "unknown" => "local_integration.error.unknown",
    "unsupported" => "local_integration.error.unsupported",
    "collision" => "local_integration.error.collision",
    "unresolved" => "local_integration.error.unresolved",
    "blank" => "local_integration.error.blank",
    "git" => "local_integration.error.git",
    "storage" => "local_integration.error.storage",
    "run" => "local_integration.error.run",
    "checkMissing" => "local_integration.error.checkMissing",
    "checkFailed" => "local_integration.error.checkFailed",
    _ => "local_integration.error.io",
  })
}
