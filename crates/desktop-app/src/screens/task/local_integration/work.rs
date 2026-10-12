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
  Refused,
  Inspect,
  Refresh,
  ReviewCommit(Vec<LocalPath>),
  Commit {
    paths: Vec<LocalPath>,
    tree: String,
    message: String,
  },
  Preserve {
    name: String,
    paths: Vec<LocalPath>,
  },
  Sync,
  Merge {
    exact: Box<CheckedResult>,
    generation: u64,
  },
  Save {
    file: LocalConflict,
    expected: Option<ContentHash>,
    resolution: Resolution,
    candidate: orchester_core::integration::local::EditorCandidate,
    placement: Placement,
    restore: bool,
  },
  SaveDraft {
    file: LocalConflict,
    expected: Option<ContentHash>,
    candidate: orchester_core::integration::local::EditorCandidate,
    generation: u64,
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
  DraftSaved(orchester_core::integration::local::EditorCandidate),
  Resolved(orchester_core::integration::local::EditorCandidate),
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
      if matches!(request.action, Action::Refused) {
        return Err(Problem("local_integration.error.approval"));
      }
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
        if matches!(request.action, Action::Inspect) {
          orchester_workspace::tasks::local_integration::read(&open.repo_root, run, None, vault.get())
            .map_err(problem)?;
          return Ok(Outcome::Changed);
        }
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
    Action::Inspect | Action::Refresh => session.refresh()?,
    Action::Refused => return Err(LocalError::ApprovalRequired.into()),
    Action::ReviewCommit(paths) => {
      let tree = session.selected_commit_tree(&paths)?;
      let preview = session.selected_commit_preview(&tree, vault)?;
      return Ok(Outcome::CommitReview { tree, preview });
    }
    Action::Commit { paths, tree, message } => session.commit(&paths, &tree, &message)?,
    Action::Preserve { name, paths } => session.preserve_selected(&name, &paths)?,
    Action::Sync => {
      session.prepare_if_clean()?;
      session.sync()?;
    }
    Action::Merge { exact, generation } => session.approve_and_merge_at(&exact, generation)?,
    Action::Save {
      file,
      expected,
      resolution,
      candidate,
      placement,
      restore,
    } => {
      if restore {
        session.save_restore_resolution(&file, expected.as_ref(), &resolution, placement)?;
      } else {
        session.save_task_resolution(&file, expected.as_ref(), &resolution)?;
      }
      return Ok(Outcome::Resolved(candidate));
    }
    Action::SaveDraft {
      file,
      expected,
      candidate,
      generation,
    } => {
      session.save_editor_draft(&file, expected.as_ref(), &candidate, generation)?;
      return Ok(Outcome::DraftSaved(candidate));
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
      LocalError::ConditionalRemovalUnsupported => "conditionalRemoval",
      LocalError::ProcessContainmentUnsupported => "containment",
      LocalError::StoreOwnershipUnsupported => "storageOwnership",
      LocalError::ProcessUncertain => "processUncertain",
      LocalError::Collision => "collision",
      LocalError::Unresolved => "unresolved",
      LocalError::Blank => "blank",
    },
    IntegrationError::Git(LocalGitError::Refused) => "git",
    IntegrationError::Git(_) => "io",
    IntegrationError::Store(_) => "storage",
    IntegrationError::Run => "run",
    IntegrationError::SensitiveMetadata => "metadata",
    IntegrationError::AlreadyIntegrated => "external",
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
    "conditionalRemoval" => "local_integration.error.conditionalRemoval",
    "containment" => "local_integration.error.containment",
    "processUncertain" => "local_integration.error.processUncertain",
    "collision" => "local_integration.error.collision",
    "unresolved" => "local_integration.error.unresolved",
    "blank" => "local_integration.error.blank",
    "git" => "local_integration.error.git",
    "storage" => "local_integration.error.storage",
    "run" => "local_integration.error.run",
    "metadata" => "local_integration.error.metadata",
    "external" => "local_integration.externalBody",
    "checkMissing" => "local_integration.error.checkMissing",
    "checkFailed" => "local_integration.error.checkFailed",
    _ => "local_integration.error.io",
  })
}
